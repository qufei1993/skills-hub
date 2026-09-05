use std::collections::{BTreeMap, BTreeSet};

use super::manifest::{PortableSkill, SyncManifest};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MergePlan {
    pub take_local: BTreeSet<String>,
    pub take_remote: BTreeSet<String>,
    pub merge_files: BTreeMap<String, BTreeSet<String>>,
    pub take_local_metadata: BTreeSet<String>,
    pub conflicts: BTreeMap<String, Vec<String>>,
    pub delete_local: BTreeSet<String>,
    pub delete_remote: BTreeSet<String>,
}

pub fn plan_merge(base: &SyncManifest, local: &SyncManifest, remote: &SyncManifest) -> MergePlan {
    let mut plan = MergePlan::default();
    let ids: BTreeSet<_> = base
        .skills
        .keys()
        .chain(local.skills.keys())
        .chain(remote.skills.keys())
        .cloned()
        .collect();

    for id in ids {
        let base_skill = base.skills.get(&id);
        let local_skill = local.skills.get(&id);
        let remote_skill = remote.skills.get(&id);
        match (base_skill, local_skill, remote_skill) {
            (_, Some(local), Some(remote)) if local.content_hash == remote.content_hash => {}
            (None, Some(_), None) => {
                plan.take_local.insert(id);
            }
            (None, None, Some(_)) => {
                plan.take_remote.insert(id);
            }
            (None, Some(_), Some(_)) => {
                plan.conflicts.insert(id, vec!["*".to_string()]);
            }
            (Some(base), Some(local), Some(remote)) => {
                if local.content_hash == base.content_hash {
                    plan.take_remote.insert(id);
                } else if remote.content_hash == base.content_hash {
                    plan.take_local.insert(id);
                } else {
                    plan_skill_files(&id, base, local, remote, &mut plan);
                }
            }
            (Some(base), Some(local), None) => {
                if local.content_hash == base.content_hash {
                    plan.delete_local.insert(id);
                } else {
                    plan.conflicts.insert(id, vec!["*".to_string()]);
                }
            }
            (Some(base), None, Some(remote)) => {
                if remote.content_hash == base.content_hash {
                    plan.delete_remote.insert(id);
                } else {
                    plan.conflicts.insert(id, vec!["*".to_string()]);
                }
            }
            (Some(_), None, None) | (None, None, None) => {}
        }
    }
    plan
}

fn plan_skill_files(
    id: &str,
    base: &PortableSkill,
    local: &PortableSkill,
    remote: &PortableSkill,
    plan: &mut MergePlan,
) {
    let base_metadata = super::manifest::metadata_hash(base);
    let local_metadata = super::manifest::metadata_hash(local);
    let remote_metadata = super::manifest::metadata_hash(remote);
    let local_metadata_changed = local_metadata != base_metadata;
    let remote_metadata_changed = remote_metadata != base_metadata;
    if local_metadata_changed && remote_metadata_changed && local_metadata != remote_metadata {
        plan.conflicts
            .insert(id.to_string(), vec!["_metadata".to_string()]);
        return;
    }
    let paths: BTreeSet<_> = base
        .files
        .keys()
        .chain(local.files.keys())
        .chain(remote.files.keys())
        .cloned()
        .collect();
    let mut local_paths = BTreeSet::new();
    let mut conflicts = Vec::new();
    for path in paths {
        let base_hash = base.files.get(&path);
        let local_hash = local.files.get(&path);
        let remote_hash = remote.files.get(&path);
        let local_changed = local_hash != base_hash;
        let remote_changed = remote_hash != base_hash;
        if local_changed && remote_changed && local_hash != remote_hash {
            conflicts.push(path);
        } else if local_changed {
            local_paths.insert(path);
        }
    }
    if conflicts.is_empty() {
        plan.merge_files.insert(id.to_string(), local_paths);
        if local_metadata_changed {
            plan.take_local_metadata.insert(id.to_string());
        }
    } else {
        plan.conflicts.insert(id.to_string(), conflicts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(id: &str, files: &[(&str, &str)]) -> PortableSkill {
        let files = files
            .iter()
            .map(|(path, hash)| ((*path).to_string(), (*hash).to_string()))
            .collect::<BTreeMap<_, _>>();
        PortableSkill {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            source_type: "local".to_string(),
            source_ref: None,
            source_subpath: None,
            source_revision: None,
            tags: vec![],
            content_hash: super::super::manifest::aggregate_hash(&files),
            files,
        }
    }

    #[test]
    fn merges_changes_to_different_files() {
        let base = SyncManifest {
            format_version: 1,
            skills: BTreeMap::from([("one".to_string(), skill("one", &[("a", "1"), ("b", "1")]))]),
        };
        let local = SyncManifest {
            format_version: 1,
            skills: BTreeMap::from([("one".to_string(), skill("one", &[("a", "2"), ("b", "1")]))]),
        };
        let remote = SyncManifest {
            format_version: 1,
            skills: BTreeMap::from([("one".to_string(), skill("one", &[("a", "1"), ("b", "2")]))]),
        };
        let plan = plan_merge(&base, &local, &remote);
        assert_eq!(plan.merge_files["one"], BTreeSet::from(["a".to_string()]));
        assert!(plan.conflicts.is_empty());
    }

    #[test]
    fn reports_same_file_conflict() {
        let base_skill = skill("one", &[("a", "1")]);
        let base = SyncManifest {
            format_version: 1,
            skills: BTreeMap::from([("one".to_string(), base_skill)]),
        };
        let local = SyncManifest {
            format_version: 1,
            skills: BTreeMap::from([("one".to_string(), skill("one", &[("a", "2")]))]),
        };
        let remote = SyncManifest {
            format_version: 1,
            skills: BTreeMap::from([("one".to_string(), skill("one", &[("a", "3")]))]),
        };
        assert_eq!(
            plan_merge(&base, &local, &remote).conflicts["one"],
            vec!["a"]
        );
    }

    #[test]
    fn reports_conflicting_metadata_changes() {
        let mut base_skill = skill("one", &[("a", "1")]);
        base_skill.name = "Base".to_string();
        base_skill.content_hash = super::super::manifest::portable_hash(&base_skill);
        let mut local_skill = base_skill.clone();
        local_skill.name = "Local".to_string();
        local_skill.files.insert("a".to_string(), "2".to_string());
        local_skill.content_hash = super::super::manifest::portable_hash(&local_skill);
        let mut remote_skill = base_skill.clone();
        remote_skill.name = "Remote".to_string();
        remote_skill.files.insert("a".to_string(), "1".to_string());
        remote_skill.files.insert("b".to_string(), "2".to_string());
        remote_skill.content_hash = super::super::manifest::portable_hash(&remote_skill);
        let wrap = |value: PortableSkill| SyncManifest {
            format_version: 1,
            skills: BTreeMap::from([("one".to_string(), value)]),
        };
        assert_eq!(
            plan_merge(&wrap(base_skill), &wrap(local_skill), &wrap(remote_skill)).conflicts["one"],
            vec!["_metadata"]
        );
    }

    #[test]
    fn detects_delete_versus_modify_and_accepts_single_sided_changes() {
        let base_skill = skill("one", &[("a", "1")]);
        let changed_skill = skill("one", &[("a", "2")]);
        let base = SyncManifest {
            format_version: 1,
            skills: BTreeMap::from([("one".to_string(), base_skill.clone())]),
        };
        let empty = SyncManifest::empty();
        let changed = SyncManifest {
            format_version: 1,
            skills: BTreeMap::from([("one".to_string(), changed_skill)]),
        };
        assert_eq!(
            plan_merge(&base, &empty, &changed).conflicts["one"],
            vec!["*"]
        );
        let unchanged = SyncManifest {
            format_version: 1,
            skills: BTreeMap::from([("one".to_string(), base_skill)]),
        };
        assert!(plan_merge(&base, &unchanged, &changed)
            .take_remote
            .contains("one"));
    }
}
