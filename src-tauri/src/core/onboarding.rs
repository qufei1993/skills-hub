use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use super::central_repo::resolve_central_repo_path;
use super::content_hash::hash_dir;
use super::skill_store::SkillStore;
use super::tool_adapters::{default_tool_adapters, scan_tool_dir, DetectedSkill};

#[derive(Clone, Debug, Serialize)]
pub struct OnboardingVariant {
    pub tool: String,
    pub name: String,
    pub path: PathBuf,
    pub fingerprint: Option<String>,
    pub is_link: bool,
    pub link_target: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OnboardingGroup {
    pub name: String,
    pub variants: Vec<OnboardingVariant>,
    pub has_conflict: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct OnboardingPlan {
    pub total_tools_scanned: usize,
    pub total_skills_found: usize,
    pub groups: Vec<OnboardingGroup>,
}

pub fn build_onboarding_plan<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
) -> Result<OnboardingPlan> {
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to resolve home directory"))?;
    let central = resolve_central_repo_path(app, store)?;
    let managed_targets = store
        .list_all_skill_target_paths()
        .unwrap_or_default()
        .into_iter()
        .map(|(tool, path)| managed_target_key(&tool, Path::new(&path)))
        .collect::<std::collections::HashSet<_>>();
    build_onboarding_plan_in_home(&home, Some(&central), Some(&managed_targets))
}

fn build_onboarding_plan_in_home(
    home: &Path,
    exclude_root: Option<&Path>,
    exclude_managed_targets: Option<&std::collections::HashSet<String>>,
) -> Result<OnboardingPlan> {
    let adapters = default_tool_adapters();
    let mut all_detected: Vec<DetectedSkill> = Vec::new();
    let mut scanned = 0usize;

    for adapter in &adapters {
        if !home.join(adapter.relative_detect_dir).exists() {
            continue;
        }
        scanned += 1;
        let dir = home.join(adapter.relative_skills_dir);
        let detected = scan_tool_dir(adapter, &dir)?;
        all_detected.extend(filter_detected(
            detected,
            exclude_root,
            exclude_managed_targets,
        ));
    }

    let mut grouped: HashMap<String, Vec<OnboardingVariant>> = HashMap::new();
    for skill in all_detected.iter() {
        let fingerprint = hash_dir(&skill.path).ok();
        let entry = grouped.entry(skill.name.clone()).or_default();
        entry.push(OnboardingVariant {
            tool: skill.tool.as_key().to_string(),
            name: skill.name.clone(),
            path: skill.path.clone(),
            fingerprint,
            is_link: skill.is_link,
            link_target: skill.link_target.clone(),
        });
    }

    let groups: Vec<OnboardingGroup> = grouped
        .into_iter()
        .map(|(name, variants)| {
            let mut uniq = variants
                .iter()
                .filter_map(|v| v.fingerprint.as_ref())
                .collect::<std::collections::HashSet<_>>()
                .len();
            if uniq == 0 {
                uniq = 1;
            }
            OnboardingGroup {
                name,
                has_conflict: uniq > 1,
                variants,
            }
        })
        .collect();

    Ok(OnboardingPlan {
        total_tools_scanned: scanned,
        total_skills_found: all_detected.len(),
        groups,
    })
}

fn filter_detected(
    detected: Vec<DetectedSkill>,
    exclude_root: Option<&Path>,
    exclude_managed_targets: Option<&std::collections::HashSet<String>>,
) -> Vec<DetectedSkill> {
    if exclude_root.is_none() && exclude_managed_targets.is_none() {
        return detected;
    }
    detected
        .into_iter()
        .filter(|skill| {
            if let Some(exclude_root) = exclude_root {
                if is_under(&skill.path, exclude_root) {
                    return false;
                }
                if let Some(target) = &skill.link_target {
                    if is_under(target, exclude_root) {
                        return false;
                    }
                }
            }
            if let Some(exclude) = exclude_managed_targets {
                if exclude.contains(&managed_target_key(skill.tool.as_key(), &skill.path)) {
                    return false;
                }
            }
            true
        })
        .collect()
}

fn is_under(path: &Path, base: &Path) -> bool {
    path.starts_with(base)
}

fn managed_target_key(tool: &str, path: &Path) -> String {
    let tool = tool.to_ascii_lowercase();
    let normalized = normalize_path_for_key(path);
    format!("{tool}\n{normalized}")
}

fn normalize_path_for_key(path: &Path) -> String {
    let normalized: PathBuf = path.components().collect();
    let s = normalized.to_string_lossy().to_string();
    #[cfg(windows)]
    {
        s.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        s
    }
}

#[cfg(test)]
#[path = "tests/onboarding.rs"]
mod tests;
