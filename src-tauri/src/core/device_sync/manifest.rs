use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::{DirEntry, WalkDir};

use crate::core::skill_store::{SkillRecord, SkillStore};

pub const MANIFEST_PATH: &str = ".skills-hub/manifest.json";
const FORMAT_PATH: &str = ".skills-hub/format.json";

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct SyncManifest {
    pub format_version: u32,
    pub skills: BTreeMap<String, PortableSkill>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PortableSkill {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub source_subpath: Option<String>,
    pub source_revision: Option<String>,
    pub tags: Vec<String>,
    pub content_hash: String,
    pub files: BTreeMap<String, String>,
}

impl SyncManifest {
    pub fn empty() -> Self {
        Self {
            format_version: 1,
            skills: BTreeMap::new(),
        }
    }

    pub fn read(root: &Path) -> Result<Self> {
        let path = root.join(MANIFEST_PATH);
        if !path.exists() {
            return Ok(Self::empty());
        }
        let bytes = fs::read(&path).with_context(|| format!("read {:?}", path))?;
        serde_json::from_slice(&bytes).context("decode device sync manifest")
    }

    pub fn write(&self, root: &Path) -> Result<()> {
        let path = root.join(MANIFEST_PATH);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(self)?;
        fs::write(&path, bytes).with_context(|| format!("write {:?}", path))?;
        fs::write(root.join(FORMAT_PATH), b"{\n  \"formatVersion\": 1\n}\n")?;
        for skill in self.skills.values() {
            let metadata_path = root.join("skills").join(&skill.id).join("skill.json");
            if let Some(parent) = metadata_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(metadata_path, serde_json::to_vec_pretty(skill)?)?;
        }
        let gitignore = root.join(".gitignore");
        if !gitignore.exists() {
            fs::write(
                gitignore,
                ".DS_Store\n.env\n.env.*\nnode_modules/\ntarget/\ndist/\nbuild/\n*.pem\n*.key\n",
            )?;
        }
        Ok(())
    }
}

pub fn export_library(store: &SkillStore, destination: &Path) -> Result<SyncManifest> {
    fs::create_dir_all(destination)?;
    let mut manifest = SyncManifest::empty();
    for skill in store.list_skills()? {
        let source = Path::new(&skill.central_path);
        if !source.is_dir() {
            continue;
        }
        let target = skill_dir(destination, &skill.id);
        if target.exists() {
            fs::remove_dir_all(&target)?;
        }
        fs::create_dir_all(&target)?;
        copy_skill_files(source, &target)?;
        let files = hash_files(&target)?;
        let tags = store
            .get_skill_tags(&skill.id)?
            .into_iter()
            .map(|tag| tag.name)
            .collect();
        let mut portable = PortableSkill {
            id: skill.id,
            name: skill.name,
            description: skill.description,
            source_type: skill.source_type,
            source_ref: skill.source_ref,
            source_subpath: skill.source_subpath,
            source_revision: skill.source_revision,
            tags,
            content_hash: aggregate_hash(&files),
            files,
        };
        portable.content_hash = portable_hash(&portable);
        manifest.skills.insert(portable.id.clone(), portable);
    }
    manifest.write(destination)?;
    Ok(manifest)
}

pub fn skill_dir(root: &Path, skill_id: &str) -> PathBuf {
    root.join("skills").join(skill_id).join("content")
}

pub fn copy_skill_files(source: &Path, destination: &Path) -> Result<()> {
    reject_private_keys(source)?;
    for entry in WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_ignored(entry))
    {
        let entry = entry?;
        if entry.path() == source || is_ignored(&entry) {
            continue;
        }
        let relative = entry.path().strip_prefix(source)?;
        validate_relative_path(relative)?;
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &target)
                .with_context(|| format!("copy {:?} -> {:?}", entry.path(), target))?;
        }
    }
    Ok(())
}

pub(super) fn reject_private_keys(root: &Path) -> Result<()> {
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if name.ends_with(".pem")
            || name.ends_with(".key")
            || name == "id_rsa"
            || name == "id_ed25519"
        {
            bail!("refusing to sync potential private key: {}", name);
        }
        let bytes = fs::read(entry.path())?;
        let contents = String::from_utf8_lossy(&bytes);
        if [
            "BEGIN PRIVATE KEY",
            "BEGIN OPENSSH PRIVATE KEY",
            "BEGIN RSA PRIVATE KEY",
            "BEGIN EC PRIVATE KEY",
            "BEGIN DSA PRIVATE KEY",
            "BEGIN ENCRYPTED PRIVATE KEY",
        ]
        .iter()
        .any(|marker| contents.contains(marker))
        {
            bail!("refusing to sync file containing a private key: {}", name);
        }
    }
    Ok(())
}

pub fn replace_directory(source: &Path, destination: &Path) -> Result<()> {
    let parent = destination.parent().context("destination has no parent")?;
    fs::create_dir_all(parent)?;
    let staging = parent.join(format!(".sync-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&staging)?;
    copy_skill_files(source, &staging)?;
    if destination.exists() {
        fs::remove_dir_all(destination)?;
    }
    fs::rename(&staging, destination).with_context(|| format!("replace {:?}", destination))?;
    Ok(())
}

pub fn hash_files(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    if !root.exists() {
        return Ok(files);
    }
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry.path().strip_prefix(root)?;
        validate_relative_path(relative)?;
        let bytes = fs::read(entry.path())?;
        files.insert(
            relative.to_string_lossy().replace('\\', "/"),
            hex::encode(Sha256::digest(bytes)),
        );
    }
    Ok(files)
}

pub fn aggregate_hash(files: &BTreeMap<String, String>) -> String {
    let mut hasher = Sha256::new();
    for (path, hash) in files {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(hash.as_bytes());
    }
    hex::encode(hasher.finalize())
}

pub fn portable_hash(skill: &PortableSkill) -> String {
    let mut hasher = Sha256::new();
    hasher.update(metadata_hash(skill).as_bytes());
    hasher.update(aggregate_hash(&skill.files).as_bytes());
    hex::encode(hasher.finalize())
}

pub fn metadata_hash(skill: &PortableSkill) -> String {
    let mut hasher = Sha256::new();
    hasher.update(skill.name.as_bytes());
    hasher.update([0]);
    hasher.update(skill.description.as_deref().unwrap_or_default().as_bytes());
    hasher.update([0]);
    hasher.update(skill.source_type.as_bytes());
    hasher.update([0]);
    hasher.update(skill.source_ref.as_deref().unwrap_or_default().as_bytes());
    hasher.update([0]);
    for tag in &skill.tags {
        hasher.update(tag.as_bytes());
        hasher.update([0]);
    }
    hex::encode(hasher.finalize())
}

pub fn record_from_portable(skill: &PortableSkill, central_path: &Path, now: i64) -> SkillRecord {
    SkillRecord {
        id: skill.id.clone(),
        name: skill.name.clone(),
        description: skill.description.clone(),
        source_type: skill.source_type.clone(),
        source_ref: skill.source_ref.clone(),
        source_subpath: skill.source_subpath.clone(),
        source_revision: skill.source_revision.clone(),
        central_path: central_path.to_string_lossy().to_string(),
        content_hash: Some(skill.content_hash.clone()),
        created_at: now,
        updated_at: now,
        last_sync_at: Some(now),
        last_seen_at: now,
        enabled: true,
        status: "ok".to_string(),
    }
}

fn is_ignored(entry: &DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    matches!(
        name.as_ref(),
        ".git"
            | ".env"
            | ".env.local"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | "__pycache__"
            | ".DS_Store"
            | "Thumbs.db"
    ) || name.ends_with(".pem")
        || name.ends_with(".key")
        || name == "id_rsa"
        || name == "id_ed25519"
}

fn validate_relative_path(path: &Path) -> Result<()> {
    if path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::RootDir))
    {
        bail!("unsafe sync path: {:?}", path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_hash_is_stable() {
        let files = BTreeMap::from([
            ("b.txt".to_string(), "2".to_string()),
            ("a.txt".to_string(), "1".to_string()),
        ]);
        assert_eq!(aggregate_hash(&files), aggregate_hash(&files));
    }

    #[test]
    fn copy_omits_credentials_and_build_outputs() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        fs::write(source.path().join("SKILL.md"), "ok").unwrap();
        fs::write(source.path().join(".env"), "TOKEN=secret").unwrap();
        fs::create_dir(source.path().join("node_modules")).unwrap();
        fs::write(source.path().join("node_modules/a"), "ignored").unwrap();
        copy_skill_files(source.path(), target.path()).unwrap();
        assert!(target.path().join("SKILL.md").exists());
        assert!(!target.path().join(".env").exists());
        assert!(!target.path().join("node_modules").exists());
    }

    #[test]
    fn copy_rejects_private_key_headers_beyond_prefix_without_copying_contents() {
        for label in [
            "PRIVATE KEY",
            "OPENSSH PRIVATE KEY",
            "RSA PRIVATE KEY",
            "EC PRIVATE KEY",
            "DSA PRIVATE KEY",
            "ENCRYPTED PRIVATE KEY",
        ] {
            let source = tempfile::tempdir().unwrap();
            let target = tempfile::tempdir().unwrap();
            let content = format!(
                "{}\n-----BEGIN {label}-----\nnot-a-real-secret\n",
                "documentation\n".repeat(400)
            );
            fs::write(source.path().join("notes.md"), content).unwrap();
            let error = copy_skill_files(source.path(), target.path()).unwrap_err();
            assert!(!error.to_string().contains("not-a-real-secret"));
            assert!(!target.path().join("notes.md").exists());
        }
    }

    #[test]
    fn copy_rejects_private_keys() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        fs::write(source.path().join("deploy.pem"), "private").unwrap();
        assert!(copy_skill_files(source.path(), target.path()).is_err());
    }
}
