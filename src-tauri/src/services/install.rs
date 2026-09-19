use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::core::cancel_token::CancelToken;
use crate::core::installer::{
    check_managed_skill_update, install_git_skill, install_git_skill_from_selection,
    install_local_skill_from_selection, list_git_skills, list_local_skills,
    update_managed_skill_from_source, validate_skill_name as validate_core_skill_name,
    GitSkillCandidate, LocalSkillCandidate, SkillAlreadyExistsError,
};
use crate::core::network_proxy::get_github_proxy_url;
use crate::core::skills_search::{search_skills_online, OnlineSkillResult};

use super::error::{ErrorCode, ServiceError};
use super::operation_lock::OperationKind;
use super::skills_hub::SkillsHubService;
use super::types::{SkillSelector, SkillSource, SkillTarget};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum InstallSource {
    Local(PathBuf),
    Git(String),
}

impl InstallSource {
    pub fn parse(input: &str) -> Result<Self, ServiceError> {
        let input = input.trim();
        if input.is_empty() {
            return Err(invalid_source());
        }
        if let Some(path) = input.strip_prefix("local:") {
            return (!path.trim().is_empty())
                .then(|| Self::Local(PathBuf::from(path.trim())))
                .ok_or_else(invalid_source);
        }
        if looks_like_explicit_local_path(input) {
            return Ok(Self::Local(PathBuf::from(input)));
        }
        if looks_like_git_source(input) || looks_like_marketplace_shorthand(input) {
            return Ok(Self::Git(input.to_string()));
        }
        Err(invalid_source())
    }

    pub const fn is_local(&self) -> bool {
        matches!(self, Self::Local(_))
    }

    pub const fn is_git(&self) -> bool {
        matches!(self, Self::Git(_))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct InstallRequest {
    pub source: InstallSource,
    pub subpath: Option<String>,
    pub name: Option<String>,
}

impl InstallRequest {
    pub fn parse(input: &str) -> Result<Self, ServiceError> {
        Ok(Self {
            source: InstallSource::parse(input)?,
            subpath: None,
            name: None,
        })
    }

    pub fn local(path: impl AsRef<Path>) -> Self {
        Self {
            source: InstallSource::Local(path.as_ref().to_path_buf()),
            subpath: None,
            name: None,
        }
    }

    pub fn git(reference: impl Into<String>) -> Self {
        Self {
            source: InstallSource::Git(reference.into()),
            subpath: None,
            name: None,
        }
    }

    pub fn with_subpath(mut self, subpath: impl Into<String>) -> Self {
        self.subpath = Some(subpath.into());
        self
    }

    pub fn with_name(mut self, name: Option<String>) -> Self {
        self.name = name;
        self
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct InstallOutcome {
    pub id: String,
    pub name: String,
    pub central_path: String,
    pub content_hash: Option<String>,
    pub source: SkillSource,
    pub targets: Vec<SkillTarget>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct UpdateCheck {
    pub id: String,
    pub name: String,
    pub update_available: bool,
    pub held_back: bool,
    pub removal_count: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct UpdateOutcome {
    pub id: String,
    pub name: String,
    pub content_hash: Option<String>,
    pub source_revision: Option<String>,
    pub updated_targets: Vec<String>,
    pub pending_targets: Vec<String>,
    pub changed: bool,
}

impl SkillsHubService {
    pub fn install(&self, request: InstallRequest) -> Result<InstallOutcome, ServiceError> {
        self.install_with_cancel(request, None)
    }

    pub(crate) fn install_with_cancel(
        &self,
        request: InstallRequest,
        cancel: Option<&CancelToken>,
    ) -> Result<InstallOutcome, ServiceError> {
        let _operation_lock = self.begin_write(OperationKind::Install)?;
        if let Some(subpath) = request.subpath.as_deref() {
            validate_subpath(subpath)?;
        }
        if let Some(name) = request.name.as_deref() {
            validate_skill_name(name)?;
        }
        let installed = match request.source {
            InstallSource::Local(source) => {
                let source = resolve_local_source(self.paths(), &source)?;
                install_local_request(self, &source, request.subpath.as_deref(), request.name)
            }
            InstallSource::Git(reference) => {
                validate_git_reference(&reference)?;
                install_git_request(
                    self,
                    &reference,
                    request.subpath.as_deref(),
                    request.name,
                    cancel,
                )
            }
        }?;
        let skill = self.show_skill(SkillSelector::Id(installed.skill_id.clone()))?;
        Ok(InstallOutcome {
            id: skill.id,
            name: installed.name,
            central_path: installed.central_path.to_string_lossy().into_owned(),
            content_hash: installed.content_hash,
            source: skill.source,
            targets: skill.targets,
        })
    }

    pub fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<OnlineSkillResult>, ServiceError> {
        self.ensure_database_compatible()?;
        if query.trim().is_empty() {
            return Err(ServiceError::new(
                ErrorCode::InvalidArgument,
                "search query must not be empty",
                json!({ "argument": "query" }),
            ));
        }
        let proxy_url = get_github_proxy_url(self.store())
            .map_err(|_| ServiceError::internal("failed to load network settings"))?;
        search_skills_online(query.trim(), limit, &proxy_url).map_err(|_| {
            ServiceError::new(
                ErrorCode::NetworkError,
                "skill search request failed",
                json!({ "legacy_category": "github_network" }),
            )
        })
    }

    pub(crate) fn local_install_candidates(
        &self,
        source: impl AsRef<Path>,
    ) -> Result<Vec<LocalSkillCandidate>, ServiceError> {
        self.ensure_database_compatible()?;
        let source = resolve_local_source(self.paths(), source.as_ref())?;
        list_local_skills(&source).map_err(map_install_error)
    }

    pub(crate) fn git_install_candidates_with_cancel(
        &self,
        reference: &str,
        cancel: Option<&CancelToken>,
    ) -> Result<Vec<GitSkillCandidate>, ServiceError> {
        self.ensure_database_compatible()?;
        validate_git_reference(reference)?;
        list_git_skills(self.paths(), self.store(), reference, cancel).map_err(map_install_error)
    }

    pub fn check_updates(&self, selector: SkillSelector) -> Result<UpdateCheck, ServiceError> {
        let skill = self.show_skill(selector)?;
        let checked = check_managed_skill_update(self.paths(), self.store(), &skill.id)
            .map_err(map_update_error)?;
        Ok(UpdateCheck {
            id: skill.id,
            name: skill.name,
            update_available: checked.changed,
            held_back: checked.changed && checked.removal_count > 0,
            removal_count: checked.removal_count,
        })
    }

    pub fn update(&self, selector: SkillSelector) -> Result<UpdateOutcome, ServiceError> {
        let _operation_lock = self.begin_write(OperationKind::Update)?;
        let skill = self.show_skill(selector)?;
        let updated = update_managed_skill_from_source(self.paths(), self.store(), &skill.id)
            .map_err(map_update_error)?;
        Ok(UpdateOutcome {
            id: updated.skill_id,
            name: updated.name,
            content_hash: updated.content_hash,
            source_revision: updated.source_revision,
            updated_targets: updated.updated_targets,
            pending_targets: updated.pending_targets,
            changed: updated.changed,
        })
    }
}

fn install_local_request(
    service: &SkillsHubService,
    source: &Path,
    subpath: Option<&str>,
    name: Option<String>,
) -> Result<crate::core::installer::InstallResult, ServiceError> {
    if let Some(subpath) = subpath {
        return install_local_skill_from_selection(
            service.paths(),
            service.store(),
            source,
            subpath,
            name,
        )
        .map_err(map_install_error);
    }

    let candidates = service.local_install_candidates(source)?;
    let valid = candidates
        .into_iter()
        .filter(|candidate| candidate.valid)
        .collect::<Vec<_>>();
    for candidate in &valid {
        validate_skill_name(&candidate.name)?;
    }
    match valid.as_slice() {
        [] => Err(invalid_source()),
        [candidate] => install_local_skill_from_selection(
            service.paths(),
            service.store(),
            source,
            &candidate.subpath,
            name,
        )
        .map_err(map_install_error),
        _ => Err(multi_local_candidates(&valid)),
    }
}

fn install_git_request(
    service: &SkillsHubService,
    reference: &str,
    subpath: Option<&str>,
    name: Option<String>,
    cancel: Option<&CancelToken>,
) -> Result<crate::core::installer::InstallResult, ServiceError> {
    if let Some(subpath) = subpath {
        return install_git_skill_from_selection(
            service.paths(),
            service.store(),
            reference,
            subpath,
            name,
            cancel,
        )
        .map_err(map_install_error);
    }

    let candidates = service.git_install_candidates_with_cancel(reference, cancel)?;
    for candidate in &candidates {
        validate_skill_name(&candidate.name)?;
    }
    match candidates.as_slice() {
        [] => install_git_skill(service.paths(), service.store(), reference, name, cancel)
            .map_err(map_install_error),
        [candidate] => install_git_skill_from_selection(
            service.paths(),
            service.store(),
            reference,
            &candidate.subpath,
            name,
            cancel,
        )
        .map_err(map_install_error),
        _ => Err(multi_git_candidates(&candidates)),
    }
}

fn resolve_local_source(
    paths: &crate::core::runtime_paths::RuntimePaths,
    source: &Path,
) -> Result<PathBuf, ServiceError> {
    let source_text = source.to_string_lossy();
    let expanded = if source_text == "~" {
        paths.default_central_repo.parent().map(Path::to_path_buf)
    } else if let Some(relative) = source_text
        .strip_prefix("~/")
        .or_else(|| source_text.strip_prefix("~\\"))
    {
        paths
            .default_central_repo
            .parent()
            .map(|home| home.join(relative))
    } else {
        Some(source.to_path_buf())
    }
    .ok_or_else(invalid_source)?;
    std::fs::canonicalize(expanded).map_err(|_| invalid_source())
}

fn validate_git_reference(reference: &str) -> Result<(), ServiceError> {
    if !looks_like_git_source(reference) && !looks_like_marketplace_shorthand(reference) {
        return Err(invalid_source());
    }
    if let Ok(url) = reqwest::Url::parse(reference) {
        if url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || (matches!(url.scheme(), "http" | "https") && !url.username().is_empty())
        {
            return Err(invalid_source());
        }
        if url
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("github.com"))
            && !looks_like_marketplace_shorthand(url.path().trim_start_matches('/'))
        {
            return Err(invalid_source());
        }
    } else if let Some(shorthand) = reference.strip_prefix("github.com/") {
        if !looks_like_marketplace_shorthand(shorthand) {
            return Err(invalid_source());
        }
    }
    Ok(())
}

fn validate_subpath(subpath: &str) -> Result<(), ServiceError> {
    if subpath.trim().is_empty() {
        return Err(invalid_source());
    }
    for component in Path::new(subpath).components() {
        if !matches!(component, Component::Normal(_) | Component::CurDir) {
            return Err(invalid_source());
        }
    }
    Ok(())
}

pub(crate) fn validate_skill_name(name: &str) -> Result<(), ServiceError> {
    validate_core_skill_name(name).map_err(|_| {
        ServiceError::new(
            ErrorCode::InvalidArgument,
            "skill name must be a single file name",
            json!({ "argument": "name" }),
        )
    })
}

fn looks_like_explicit_local_path(input: &str) -> bool {
    input.starts_with("./")
        || input.starts_with(".\\")
        || input.starts_with("../")
        || input.starts_with("..\\")
        || input.starts_with("~/")
        || input.starts_with("~\\")
        || Path::new(input).is_absolute()
}

fn looks_like_git_source(input: &str) -> bool {
    input.contains("://")
        || input.starts_with("git@")
        || input.ends_with(".git")
        || input.starts_with("github.com/")
}

fn looks_like_marketplace_shorthand(input: &str) -> bool {
    if input.contains(':')
        || input.contains('@')
        || input.starts_with('.')
        || input.starts_with('/')
        || input.starts_with('~')
    {
        return false;
    }
    let parts = input.split('/').collect::<Vec<_>>();
    if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
        return false;
    }
    let safe = |segment: &str| {
        segment
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, '-' | '_' | '.'))
    };
    let repo = parts[1].strip_suffix(".git").unwrap_or(parts[1]);
    if !safe(parts[0])
        || matches!(parts[0], "." | "..")
        || repo.is_empty()
        || matches!(repo, "." | "..")
        || !safe(repo)
    {
        return false;
    }
    if parts.len() == 2 {
        return true;
    }
    if parts.len() < 5 || !matches!(parts[2], "tree" | "blob") {
        return false;
    }
    parts[3..]
        .iter()
        .all(|segment| !segment.is_empty() && *segment != "." && *segment != ".." && safe(segment))
}

fn multi_git_candidates(candidates: &[GitSkillCandidate]) -> ServiceError {
    ServiceError::new(
        ErrorCode::MultiSkills,
        "the source contains multiple skills",
        json!({
            "candidates": candidates.iter().map(|candidate| json!({
                "name": candidate.name,
                "description": candidate.description,
                "subpath": candidate.subpath,
            })).collect::<Vec<_>>()
        }),
    )
}

fn multi_local_candidates(candidates: &[LocalSkillCandidate]) -> ServiceError {
    ServiceError::new(
        ErrorCode::MultiSkills,
        "the source contains multiple skills",
        json!({
            "candidates": candidates.iter().map(|candidate| json!({
                "name": candidate.name,
                "description": candidate.description,
                "subpath": candidate.subpath,
                "valid": candidate.valid,
                "reason": candidate.reason,
            })).collect::<Vec<_>>()
        }),
    )
}

fn invalid_source() -> ServiceError {
    ServiceError::new(
        ErrorCode::InvalidSource,
        "the skill source is invalid or unsupported",
        json!({}),
    )
}

fn map_install_error(error: anyhow::Error) -> ServiceError {
    if let Some(conflict) = error.downcast_ref::<SkillAlreadyExistsError>() {
        return ServiceError::new(
            ErrorCode::TargetConflict,
            "the skill already exists in the Skills Hub library",
            json!({
                "legacy_category": "skill_exists",
                "path": conflict.central_path().to_string_lossy().into_owned(),
            }),
        );
    }
    let safe_error = format!("{error:#}");
    let lower = safe_error.to_lowercase();
    if lower.contains("cancelled|") {
        return ServiceError::new(
            ErrorCode::InternalError,
            "the operation was cancelled",
            json!({ "legacy_category": "cancelled" }),
        );
    }
    let category = crate::core::skill_issues::safe_code(&safe_error);
    match category {
        "sourceMissing" | "repoPathMissing" => invalid_source(),
        _ => map_remote_error(&safe_error)
            .unwrap_or_else(|| ServiceError::internal("skill installation failed")),
    }
}

fn map_update_error(error: anyhow::Error) -> ServiceError {
    let first = error.to_string();
    if first.starts_with("UPDATE_IN_PROGRESS|") {
        return ServiceError::new(
            ErrorCode::OperationBusy,
            "another Skills Hub operation is already in progress",
            json!({ "operation": "update" }),
        );
    }
    if let Some(count) = first.strip_prefix("UPDATE_HELD_BACK|") {
        return ServiceError::new(
            ErrorCode::UpdateHeldBack,
            "the update would remove managed files",
            json!({ "removal_count": count.parse::<usize>().unwrap_or(0) }),
        );
    }
    map_remote_error(&format!("{error:#}"))
        .unwrap_or_else(|| ServiceError::internal("skill update failed"))
}

fn map_remote_error(safe_error: &str) -> Option<ServiceError> {
    let lower = safe_error.to_lowercase();
    let category = crate::core::skill_issues::safe_code(safe_error);
    let remote_context = lower.contains("github")
        || lower.contains("git ")
        || lower.contains("clone")
        || lower.contains("remote")
        || lower.contains("repository");
    if category == "auth"
        || lower.contains("credentials")
        || lower.contains("unauthorized")
        || lower.contains("访问被拒绝")
        || (remote_context && lower.contains("permission denied"))
    {
        return Some(ServiceError::new(
            ErrorCode::AuthRequired,
            "the remote source requires authentication",
            json!({ "legacy_category": "github_auth" }),
        ));
    }
    if remote_context && (lower.contains("not found") || lower.contains("未找到")) {
        return Some(ServiceError::new(
            ErrorCode::InvalidSource,
            "the remote source was not found or is not accessible",
            json!({ "legacy_category": "github_not_found" }),
        ));
    }
    if lower.contains("rate limit") || lower.contains("频率限制") {
        return Some(ServiceError::new(
            ErrorCode::NetworkError,
            "the remote source rate limit was reached",
            json!({ "legacy_category": "github_rate_limited" }),
        ));
    }
    let network_category = if lower.contains("securetransport") {
        Some("github_tls")
    } else if lower.contains("failed to resolve")
        || lower.contains("could not resolve")
        || lower.contains("resolve host")
        || lower.contains("dns")
    {
        Some("github_dns")
    } else if lower.contains("timed out") || lower.contains("timeout") || lower.contains("超时") {
        Some("github_timeout")
    } else if lower.contains("connection refused")
        || lower.contains("connection reset")
        || lower.contains("failed to connect")
    {
        Some("github_connection")
    } else if category == "network"
        || lower.contains("proxy")
        || lower.contains("网络")
        || lower.contains("代理")
    {
        Some("github_network")
    } else {
        None
    };
    if let Some(legacy_category) = network_category {
        return Some(ServiceError::new(
            ErrorCode::NetworkError,
            "the remote source could not be reached",
            json!({ "legacy_category": legacy_category }),
        ));
    }
    None
}

#[cfg(test)]
mod error_mapping_tests {
    use super::*;

    #[test]
    fn localized_git_failures_keep_stable_safe_categories() {
        for (message, code, category) in [
            (
                "该 Skill 在 GitHub 上未找到（可能已被删除或路径已变更）。",
                ErrorCode::InvalidSource,
                "github_not_found",
            ),
            (
                "GitHub API 频率限制已触发。可在设置中配置 GitHub Token 以提升限额。",
                ErrorCode::NetworkError,
                "github_rate_limited",
            ),
            (
                "GitHub API 访问被拒绝（可能触发了频率限制）。请稍后再试。",
                ErrorCode::AuthRequired,
                "github_auth",
            ),
            (
                "git 操作超时（120s）。请检查网络/代理是否可访问 GitHub。",
                ErrorCode::NetworkError,
                "github_timeout",
            ),
        ] {
            let secret = "do-not-leak-service-secret";
            let error = map_install_error(anyhow::anyhow!("{message} secret={secret}"));
            assert_eq!(error.code, code);
            assert_eq!(error.details["legacy_category"], category);
            assert!(!serde_json::to_string(&error).unwrap().contains(secret));
        }
    }
}
