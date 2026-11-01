//! Lightweight, non-executable observations of managed Skill sources.
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::core::cancel_token::CancelToken;
use crate::core::skill_store::SkillRecord;

use super::error::{ErrorCode, ServiceError};
use super::operation_lock::OperationKind;
use super::skills_hub::SkillsHubService;

type Result<T> = std::result::Result<T, ServiceError>;
const SUMMARY_KIND: &str = "content-check-summary";
const RUN_KIND: &str = "content-check-run";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContentCheckSummary {
    pub skill_id: String,
    pub name: String,
    pub signature: String,
    pub checked_at: i64,
    pub status: String,
    pub source_revision: Option<String>,
    pub target_count: usize,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCheckRun {
    pub check_id: String,
    pub summaries: Vec<ContentCheckSummary>,
    pub cancelled: bool,
    pub total: usize,
    pub completed: usize,
    pub checked_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCheckProgress {
    pub check_id: String,
    pub phase: String,
    pub completed: usize,
    pub total: usize,
    pub current_skill_id: Option<String>,
    pub summary: Option<ContentCheckSummary>,
}

fn storage(_: impl std::fmt::Display) -> ServiceError {
    ServiceError::internal(
        "update check results could not be saved or inspected; check storage and retry",
    )
}

impl SkillsHubService {
    fn content_check_signature(&self, record: &SkillRecord) -> Result<String> {
        let mut targets = self
            .store()
            .list_skill_targets(&record.id)
            .map_err(storage)?;
        targets.sort_by(|a, b| a.id.cmp(&b.id));
        let identities = targets
            .iter()
            .map(|t| json!([t.id, t.tool, t.scope, t.project_path, t.target_path, t.mode]))
            .collect::<Vec<_>>();
        // Version the observation contract. Do not include polling timestamps,
        // tags or errors, and never persist the source URL in plaintext here.
        let input = json!([
            1,
            record.id,
            record.name,
            record.enabled,
            record.source_type,
            record.source_ref,
            record.source_subpath,
            record.source_revision,
            record.content_hash,
            record.central_path,
            record.updated_at,
            identities
        ]);
        Ok(format!(
            "{:x}",
            Sha256::digest(input.to_string().as_bytes())
        ))
    }

    /// Read only: no source access, credentials, cache refresh or plan creation.
    pub fn cached_content_checks(&self) -> Result<Option<ContentCheckRun>> {
        self.ensure_database_compatible()?;
        let Some(mut run) = self
            .store()
            .workspace_get::<ContentCheckRun>(RUN_KIND, "latest")
            .map_err(storage)?
        else {
            return Ok(None);
        };
        let records = self.store().list_skills().map_err(storage)?;
        let records = records
            .into_iter()
            .filter(|r| r.enabled)
            .map(|r| (r.id.clone(), r))
            .collect::<HashMap<_, _>>();
        run.summaries.clear();
        for summary in self
            .store()
            .workspace_list::<ContentCheckSummary>(SUMMARY_KIND)
            .map_err(storage)?
        {
            let Some(record) = records.get(&summary.skill_id) else {
                continue;
            };
            if self.content_check_signature(record)? == summary.signature {
                run.summaries.push(summary);
            }
        }
        run.summaries.sort_by(|a, b| a.skill_id.cmp(&b.skill_id));
        // Cached results describe the current library's last observations, not
        // the size of a previous retry-only request.
        run.total = records.len();
        run.completed = run.summaries.len();
        Ok(Some(run))
    }

    pub fn check_content_sources(
        &self,
        check_id: &str,
        skill_ids: &[String],
        cancel: &CancelToken,
        mut on_progress: impl FnMut(ContentCheckProgress),
    ) -> Result<ContentCheckRun> {
        uuid::Uuid::parse_str(check_id).map_err(|_| {
            ServiceError::new(
                ErrorCode::InvalidArgument,
                "checkId must be a UUID",
                json!({}),
            )
        })?;
        self.ensure_database_compatible()?;
        let mut seen = HashSet::new();
        let ids = skill_ids
            .iter()
            .filter(|id| seen.insert(id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let mut run = ContentCheckRun {
            check_id: check_id.into(),
            summaries: vec![],
            cancelled: false,
            total: ids.len(),
            completed: 0,
            checked_at: chrono::Utc::now().timestamp_millis(),
        };
        on_progress(progress(&run, "preparing", None));
        if cancel.is_cancelled() {
            run.cancelled = true;
            on_progress(progress(&run, "completed", None));
            return Ok(run);
        }
        let _operation = self.begin_write(OperationKind::Update)?;
        let mut records = HashMap::new();
        let mut signatures = HashMap::new();
        for id in &ids {
            let record = self
                .store()
                .get_skill_by_id(id)
                .map_err(storage)?
                .ok_or_else(|| ServiceError::skill_not_found(id))?;
            if !record.enabled {
                return Err(ServiceError::new(
                    ErrorCode::InvalidArgument,
                    "only enabled Skills can be checked",
                    json!({ "skill_id": id }),
                ));
            }
            signatures.insert(id.clone(), self.content_check_signature(&record)?);
            records.insert(id.clone(), record);
        }
        let mut persistence_error = None;
        crate::core::content_recovery::check_batch(
            self.paths(),
            self.store(),
            &ids,
            Some(cancel),
            |id, result| {
                if persistence_error.is_some() {
                    return;
                }
                let record = &records[id];
                // A core-only writer can finish after the operation-lock
                // snapshot but before check_batch acquires the update lock.
                // Never publish its observation under the earlier identity.
                let current_signature =
                    self.store()
                        .get_skill_by_id(id)
                        .map_err(storage)
                        .and_then(|current| {
                            current
                                .filter(|r| r.enabled)
                                .map(|r| self.content_check_signature(&r))
                                .transpose()
                        });
                let result = match current_signature {
                    Ok(Some(current)) if current == signatures[id] => result,
                    Ok(_) => Err(anyhow::anyhow!("PLAN_STALE")),
                    Err(error) => {
                        persistence_error = Some(error);
                        return;
                    }
                };
                let (status, source_revision, target_count, error) = match result {
                    Ok(item) => (
                        if item.changed { "changed" } else { "unchanged" },
                        item.source_revision,
                        item.target_count,
                        None,
                    ),
                    Err(error) => {
                        let count = match self.store().list_skill_targets(id) {
                            Ok(targets) => targets.len(),
                            Err(error) => {
                                persistence_error = Some(storage(error));
                                return;
                            }
                        };
                        ("failed", None, count, Some(check_error(error, record)))
                    }
                };
                let summary = ContentCheckSummary {
                    skill_id: id.into(),
                    name: record.name.clone(),
                    signature: signatures[id].clone(),
                    checked_at: chrono::Utc::now().timestamp_millis(),
                    status: status.into(),
                    source_revision,
                    target_count,
                    error,
                };
                if let Err(error) = self.store().workspace_save(SUMMARY_KIND, id, &summary) {
                    persistence_error = Some(storage(error));
                    return;
                }
                run.summaries.push(summary.clone());
                run.completed = run.summaries.len();
                on_progress(progress(&run, "checking", Some(summary)));
            },
        )
        .map_err(batch_error)?;
        if let Some(error) = persistence_error {
            return Err(error);
        }
        run.cancelled = cancel.is_cancelled();
        run.checked_at = chrono::Utc::now().timestamp_millis();
        // One observation per extant Skill and one run header. No history of
        // downloaded content, diff text or executable plans is retained here.
        let current_ids = self
            .store()
            .list_skills()
            .map_err(storage)?
            .into_iter()
            .map(|r| r.id)
            .collect::<HashSet<_>>();
        for summary in self
            .store()
            .workspace_list::<ContentCheckSummary>(SUMMARY_KIND)
            .map_err(storage)?
        {
            if !current_ids.contains(&summary.skill_id) {
                self.store()
                    .workspace_delete(SUMMARY_KIND, &summary.skill_id)
                    .map_err(storage)?;
            }
        }
        let mut header = run.clone();
        header.summaries.clear();
        self.store()
            .workspace_save(RUN_KIND, "latest", &header)
            .map_err(storage)?;
        on_progress(progress(&run, "completed", None));
        Ok(run)
    }
}

fn progress(
    run: &ContentCheckRun,
    phase: &str,
    summary: Option<ContentCheckSummary>,
) -> ContentCheckProgress {
    ContentCheckProgress {
        check_id: run.check_id.clone(),
        phase: phase.into(),
        completed: run.completed,
        total: run.total,
        current_skill_id: summary.as_ref().map(|s| s.skill_id.clone()),
        summary,
    }
}

fn check_error(error: anyhow::Error, record: &SkillRecord) -> String {
    let error = super::workspace::content_error(error);
    match error.code {
        ErrorCode::TargetConflict => match (
            error.details["reason"].as_str(),
            error.details["path"].as_str(),
        ) {
            (Some("target_modified" | "modified_target"), Some(path)) => {
                format!("TARGET_MODIFIED|{path}")
            }
            (_, Some(path)) => format!("TARGET_EXISTS|{path}"),
            _ => "TARGET_MODIFIED".into(),
        },
        ErrorCode::PlanStale => "PLAN_STALE".into(),
        _ => {
            // Error chains from Git may contain authenticated URLs. Keep a
            // stable category; only a local filesystem source is safe detail.
            match record
                .source_ref
                .as_deref()
                .filter(|p| record.source_type == "local" && !p.contains("://"))
            {
                Some(path) => format!("INVALID_SOURCE|{path}"),
                None => "INVALID_SOURCE".into(),
            }
        }
    }
}

fn batch_error(error: anyhow::Error) -> ServiceError {
    if error.to_string().starts_with("UPDATE_IN_PROGRESS|") {
        ServiceError::new(
            ErrorCode::OperationBusy,
            "another Skills Hub operation is already in progress",
            json!({}),
        )
    } else {
        super::workspace::content_error(error)
    }
}
