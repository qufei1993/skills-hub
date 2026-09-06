use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{ensure, Context, Result};
use git2::{Oid, Repository};
use serde::{Deserialize, Serialize};

use super::types::DeviceSyncDevice;

const REGISTRY_PATH: &str = "devices.json";
const MAX_REGISTRY_BYTES: usize = 1024 * 1024;

#[derive(Deserialize, Serialize)]
pub(super) struct DeviceRegistry {
    version: u32,
    devices: BTreeMap<String, DeviceRecord>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceRecord {
    name: String,
    last_synced_at: i64,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

impl DeviceRegistry {
    pub(super) fn read_at(repo: &Repository, head: Option<Oid>) -> Result<Self> {
        let mut registry = Self {
            version: 1,
            devices: BTreeMap::new(),
            extra: BTreeMap::new(),
        };
        let Some(head) = head else {
            return Ok(registry);
        };
        let commit = repo.find_commit(head)?;
        let tree = commit.tree()?;
        let entry = match tree.get_path(Path::new(REGISTRY_PATH)) {
            Ok(entry) => Some(entry),
            Err(error) if error.code() == git2::ErrorCode::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        if let Some(entry) = entry {
            ensure!(
                matches!(entry.filemode(), 0o100644 | 0o100755),
                "invalid device registry file type"
            );
            let blob = repo.find_blob(entry.id())?;
            ensure!(
                blob.size() <= MAX_REGISTRY_BYTES,
                "device registry is too large"
            );
            registry = serde_json::from_slice(blob.content())
                .map_err(|_| anyhow::anyhow!("invalid device registry format"))?;
            ensure!(registry.version == 1, "unsupported device registry version");
            // Older clients can append commits without updating the registry. Only
            // inspect that tail, stopping at the commit that last wrote this blob.
            let mut tail = Some(commit);
            let mut seen = std::collections::HashSet::new();
            while let Some(commit) = tail {
                let parent = if commit.parent_count() > 0 {
                    Some(commit.parent(0)?)
                } else {
                    None
                };
                let same_blob = parent
                    .as_ref()
                    .and_then(|parent| parent.tree().ok())
                    .and_then(|tree| tree.get_path(Path::new(REGISTRY_PATH)).ok())
                    .is_some_and(|previous| previous.id() == entry.id());
                if !same_blob {
                    break;
                }
                if let Some((id, name)) = super::git_repo::device_identity(&commit) {
                    if seen.insert(id.clone()) {
                        let record = registry.devices.entry(id).or_insert(DeviceRecord {
                            name: name.clone(),
                            last_synced_at: 0,
                            extra: BTreeMap::new(),
                        });
                        let timestamp = commit.time().seconds().saturating_mul(1_000);
                        record.name = name;
                        record.last_synced_at = record.last_synced_at.max(timestamp);
                    }
                }
                tail = parent;
            }
        } else {
            for device in super::git_repo::discover_devices_at(repo, head)? {
                registry.record(&device);
            }
        }
        for (id, record) in &registry.devices {
            ensure!(
                !id.is_empty()
                    && !record.name.is_empty()
                    && (0..=8_640_000_000_000_000).contains(&record.last_synced_at),
                "invalid device registry record"
            );
        }
        Ok(registry)
    }

    pub(super) fn record(&mut self, device: &DeviceSyncDevice) {
        let record = self
            .devices
            .entry(device.id.clone())
            .or_insert(DeviceRecord {
                name: device.name.clone(),
                last_synced_at: device.last_seen_at,
                extra: BTreeMap::new(),
            });
        record.name.clone_from(&device.name);
        record.last_synced_at = device.last_seen_at;
    }

    pub(super) fn write(&self, root: &Path) -> Result<()> {
        let path = root.join(REGISTRY_PATH);
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            ensure!(metadata.is_file(), "invalid device registry file type");
        }
        let mut bytes = serde_json::to_vec_pretty(self).context("encode device registry")?;
        bytes.push(b'\n');
        ensure!(
            bytes.len() <= MAX_REGISTRY_BYTES,
            "device registry is too large"
        );
        fs::write(path, bytes).context("write device registry")
    }

    pub(super) fn devices(&self) -> impl Iterator<Item = DeviceSyncDevice> + '_ {
        self.devices.iter().map(|(id, record)| DeviceSyncDevice {
            id: id.clone(),
            name: record.name.clone(),
            alias: None,
            last_seen_at: record.last_synced_at,
            last_commit: None,
            is_current: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit_registry(root: &Path, content: &[u8]) -> (Repository, Oid) {
        let repo = Repository::init(root).unwrap();
        fs::write(root.join(REGISTRY_PATH), content).unwrap();
        let oid = super::super::git_repo::commit_all(&repo, "Registry fixture", None)
            .unwrap()
            .unwrap();
        (repo, oid)
    }

    #[test]
    fn rejects_unsupported_versions_and_unrenderable_timestamps() {
        for value in [
            r#"{"version":2,"devices":{}}"#,
            r#"{"version":1,"devices":{"one":{"name":"One","lastSyncedAt":9223372036854775807}}}"#,
        ] {
            let temp = tempfile::tempdir().unwrap();
            let (repo, oid) = commit_registry(temp.path(), value.as_bytes());
            assert!(DeviceRegistry::read_at(&repo, Some(oid)).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_without_reading_or_overwriting_the_destination() {
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("outside.json");
        fs::write(&destination, "private data").unwrap();
        let root = temp.path().join("repo");
        let repo = Repository::init(&root).unwrap();
        std::os::unix::fs::symlink(&destination, root.join(REGISTRY_PATH)).unwrap();
        let oid = super::super::git_repo::commit_all(&repo, "Symlink", None)
            .unwrap()
            .unwrap();
        assert!(DeviceRegistry::read_at(&repo, Some(oid)).is_err());
        let empty = DeviceRegistry::read_at(&repo, None).unwrap();
        assert!(empty.write(&root).is_err());
        assert_eq!(fs::read_to_string(destination).unwrap(), "private data");
    }

    #[test]
    fn preserves_other_records_and_forward_compatible_fields() {
        let temp = tempfile::tempdir().unwrap();
        let (repo, oid) = commit_registry(temp.path(), br#"{"version":1,"future":true,"devices":{"one":{"name":"One","lastSyncedAt":123,"future":"keep"}}}"#);
        let mut registry = DeviceRegistry::read_at(&repo, Some(oid)).unwrap();
        registry.record(&DeviceSyncDevice {
            id: "two".into(),
            name: "Two".into(),
            alias: Some("local alias".into()),
            last_commit: None,
            last_seen_at: 456,
            is_current: true,
        });
        registry.write(temp.path()).unwrap();
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(temp.path().join(REGISTRY_PATH)).unwrap()).unwrap();
        assert_eq!(saved["future"], true);
        assert_eq!(saved["devices"]["one"]["future"], "keep");
        assert_eq!(saved["devices"]["one"]["lastSyncedAt"], 123);
        assert_eq!(saved["devices"]["two"]["lastSyncedAt"], 456);
        assert!(!saved.to_string().contains("local alias"));
    }
}
