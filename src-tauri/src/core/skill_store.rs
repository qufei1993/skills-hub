use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use tauri::Manager;
use uuid::Uuid;

use super::device_sync::types::{
    DeviceSyncConfig, DeviceSyncDevice, SyncConflict, SyncHistoryEntry, TrashEntry,
};

const DB_FILE_NAME: &str = "skills_hub.db";
const LEGACY_APP_IDENTIFIERS: &[&str] = &["com.tauri.dev", "com.tauri.dev.skillshub"];
const LEGACY_GITHUB_TOKEN_SETTING: &str = "github_token";
const GITHUB_TOKEN_SECURE_CLEANUP_PENDING_SETTING: &str = "github_token_secure_cleanup_pending";

// Keep the shared schema compatible with v0.9.1. Feature-only additive tables
// use their own version marker in settings instead of PRAGMA user_version.
const SCHEMA_VERSION: i32 = 6;
const PRE_RELEASE_DEVICE_SYNC_SCHEMA_VERSION: i32 = 7;
const DEVICE_SYNC_SCHEMA_VERSION_KEY: &str = "schema.device_sync";
const DEVICE_SYNC_SCHEMA_VERSION: &str = "1";
const DEVICE_SYNC_STARTUP_CREDENTIAL_CONSENT_MIGRATION: &str =
    "migration.device_sync_startup_credential_consent_v1";

const DEVICE_SYNC_SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS device_sync_config (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  config_json TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS device_sync_runs (
  id TEXT PRIMARY KEY,
  started_at INTEGER NOT NULL,
  finished_at INTEGER NULL,
  status TEXT NOT NULL,
  added INTEGER NOT NULL DEFAULT 0,
  updated INTEGER NOT NULL DEFAULT 0,
  deleted INTEGER NOT NULL DEFAULT 0,
  conflicted INTEGER NOT NULL DEFAULT 0,
  commit_hash TEXT NULL,
  error TEXT NULL
);

CREATE TABLE IF NOT EXISTS device_sync_conflicts (
  id TEXT PRIMARY KEY,
  skill_id TEXT NOT NULL,
  skill_name TEXT NOT NULL,
  base_commit TEXT NULL,
  local_commit TEXT NOT NULL,
  remote_commit TEXT NOT NULL,
  files_json TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  status TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS device_sync_devices (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  last_commit TEXT NULL,
  last_seen_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS device_sync_tombstones (
  id TEXT PRIMARY KEY,
  skill_id TEXT NOT NULL,
  skill_name TEXT NOT NULL,
  trash_path TEXT NOT NULL,
  deleted_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_device_sync_runs_started_at
ON device_sync_runs(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_device_sync_conflicts_status
ON device_sync_conflicts(status, created_at DESC);
"#;

// Minimal schema for MVP: skills, skill_targets, settings, discovered_skills(optional).
const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS skills (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  source_type TEXT NOT NULL,
  source_ref TEXT NULL,
  source_revision TEXT NULL,
  central_path TEXT NOT NULL UNIQUE,
  content_hash TEXT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  last_sync_at INTEGER NULL,
  last_seen_at INTEGER NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  status TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS skill_targets (
  id TEXT PRIMARY KEY,
  skill_id TEXT NOT NULL,
  tool TEXT NOT NULL,
  scope TEXT NOT NULL DEFAULT 'global',
  project_path TEXT NULL,
  target_path TEXT NOT NULL,
  mode TEXT NOT NULL,
  status TEXT NOT NULL,
  last_error TEXT NULL,
  synced_at INTEGER NULL,
  FOREIGN KEY(skill_id) REFERENCES skills(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_skill_targets_unique_scope
ON skill_targets(skill_id, tool, scope, COALESCE(project_path, ''));

CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS discovered_skills (
  id TEXT PRIMARY KEY,
  tool TEXT NOT NULL,
  found_path TEXT NOT NULL,
  name_guess TEXT NULL,
  fingerprint TEXT NULL,
  found_at INTEGER NOT NULL,
  imported_skill_id TEXT NULL,
  FOREIGN KEY(imported_skill_id) REFERENCES skills(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_skills_name ON skills(name);
CREATE INDEX IF NOT EXISTS idx_skills_updated_at ON skills(updated_at);

CREATE TABLE IF NOT EXISTS skill_tags (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS skill_tag_links (
  skill_id TEXT NOT NULL,
  tag_id INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (skill_id, tag_id),
  FOREIGN KEY(skill_id) REFERENCES skills(id) ON DELETE CASCADE,
  FOREIGN KEY(tag_id) REFERENCES skill_tags(id) ON DELETE CASCADE
);
"#;

#[derive(Clone, Debug)]
pub struct SkillStore {
    db_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct SkillRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub source_subpath: Option<String>,
    pub source_revision: Option<String>,
    pub central_path: String,
    pub content_hash: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_sync_at: Option<i64>,
    pub last_seen_at: i64,
    pub enabled: bool,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct SkillTargetRecord {
    pub id: String,
    pub skill_id: String,
    pub tool: String,
    pub scope: String,
    pub project_path: Option<String>,
    pub target_path: String,
    pub mode: String,
    pub status: String,
    pub last_error: Option<String>,
    pub synced_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagRecord {
    pub id: i64,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagWithCountRecord {
    pub id: i64,
    pub name: String,
    pub skill_count: i64,
    pub updated_at: i64,
}

impl SkillStore {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    #[allow(dead_code)]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn ensure_schema(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute_batch("PRAGMA foreign_keys = ON;")?;

            let user_version: i32 = conn.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
            if user_version == 0 {
                conn.execute_batch(SCHEMA_V1)?;
                // V2: add description column
                conn.execute_batch("ALTER TABLE skills ADD COLUMN description TEXT NULL;")?;
                // V3: add source_subpath column
                conn.execute_batch("ALTER TABLE skills ADD COLUMN source_subpath TEXT NULL;")?;
                migrate_skill_targets_to_v4(conn)?;
                conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            } else if user_version < SCHEMA_VERSION {
                // Incremental migrations
                if user_version < 2 {
                    conn.execute_batch("ALTER TABLE skills ADD COLUMN description TEXT NULL;")?;
                }
                if user_version < 3 {
                    conn.execute_batch("ALTER TABLE skills ADD COLUMN source_subpath TEXT NULL;")?;
                }
                if user_version < 4 {
                    migrate_skill_targets_to_v4(conn)?;
                }
                if user_version < 5 {
                    migrate_tags_to_v5(conn)?;
                }
                if user_version < 6 {
                    migrate_skill_enabled_to_v6(conn)?;
                }
                conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            } else if user_version > SCHEMA_VERSION
                && user_version != PRE_RELEASE_DEVICE_SYNC_SCHEMA_VERSION
            {
                anyhow::bail!(
                    "database schema version {} is newer than app supports {}",
                    user_version,
                    SCHEMA_VERSION
                );
            }

            conn.execute_batch(DEVICE_SYNC_SCHEMA_V1)?;
            conn.execute_batch("CREATE TABLE IF NOT EXISTS skill_source_checks (
                skill_id TEXT PRIMARY KEY REFERENCES skills(id) ON DELETE CASCADE,
                error_code TEXT NULL, checked_at INTEGER NOT NULL
            );")?;
            if conn.query_row("SELECT value FROM settings WHERE key='migration.source_checks_v1'", [], |row| row.get::<_,String>(0)).optional()?.is_none() {
                let tx = conn.unchecked_transaction()?;
                tx.execute("INSERT OR IGNORE INTO skill_source_checks SELECT id,'unknown',0 FROM skills WHERE status='error'", [])?;
                let legacy = tx.query_row("SELECT value FROM settings WHERE key='skill_auto_update_last_error'", [], |row| row.get::<_,String>(0)).optional()?.unwrap_or_default();
                for line in legacy.lines() {
                    if let Some((id, reason)) = line.split_once(": ") {
                        if matches!(super::skill_issues::safe_code(reason), "sourceMissing" | "repoPathMissing") {
                            tx.execute("INSERT OR IGNORE INTO skill_source_checks (skill_id,error_code,checked_at) SELECT id,'recheck',0 FROM skills WHERE id=?1", params![id])?;
                        }
                    }
                }
                tx.execute("UPDATE skills SET status='error' WHERE id IN (SELECT skill_id FROM skill_source_checks WHERE error_code IS NOT NULL)", [])?;
                tx.execute("INSERT INTO settings (key,value) VALUES ('migration.source_checks_v1','1')", [])?;
                tx.commit()?;
            }
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO NOTHING",
                params![DEVICE_SYNC_SCHEMA_VERSION_KEY, DEVICE_SYNC_SCHEMA_VERSION],
            )?;
            if user_version == PRE_RELEASE_DEVICE_SYNC_SCHEMA_VERSION {
                conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            }

            Ok(())
        })
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
            let mut rows = stmt.query(params![key])?;
            Ok(rows
                .next()?
                .map(|row| row.get::<_, String>(0))
                .transpose()?)
        })
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
            Ok(())
        })
    }

    pub fn commit_central_repo_migration(
        &self,
        updates: &[(String, String, i64)],
        new_base: &str,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute_batch("BEGIN IMMEDIATE;")?;
            let result = (|| -> Result<()> {
                for (skill_id, central_path, updated_at) in updates {
                    let changed = conn.execute(
                        "UPDATE skills SET central_path = ?1, updated_at = ?2 WHERE id = ?3",
                        params![central_path, updated_at, skill_id],
                    )?;
                    if changed != 1 {
                        anyhow::bail!("skill not found during storage migration: {skill_id}");
                    }
                }
                conn.execute(
                    "INSERT INTO settings (key, value) VALUES ('central_repo_path', ?1)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    params![new_base],
                )?;
                Ok(())
            })();
            match result {
                Ok(()) => {
                    conn.execute_batch("COMMIT;")?;
                    Ok(())
                }
                Err(err) => {
                    let _ = conn.execute_batch("ROLLBACK;");
                    Err(err)
                }
            }
        })
    }

    pub fn delete_setting(&self, key: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
            Ok(())
        })
    }

    pub fn secure_delete_setting_with_pending_marker(
        &self,
        key: &str,
        pending_marker: &str,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.pragma_update(None, "secure_delete", "ON")?;
            conn.execute_batch("BEGIN IMMEDIATE;")?;
            let delete_result = (|| -> Result<()> {
                conn.execute(
                    "INSERT INTO settings (key, value) VALUES (?1, '1')
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    params![pending_marker],
                )?;
                conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
                Ok(())
            })();
            match delete_result {
                Ok(()) => conn.execute_batch("COMMIT;")?,
                Err(err) => {
                    let _ = conn.execute_batch("ROLLBACK;");
                    return Err(err);
                }
            }

            truncate_sqlite_wal(conn)?;
            conn.execute_batch("VACUUM;")?;
            truncate_sqlite_wal(conn)?;
            conn.execute(
                "DELETE FROM settings WHERE key = ?1",
                params![pending_marker],
            )?;
            Ok(())
        })
    }

    pub fn get_device_sync_config(&self) -> Result<Option<DeviceSyncConfig>> {
        self.with_conn(|conn| {
            let mut stmt =
                conn.prepare("SELECT config_json FROM device_sync_config WHERE id = 1 LIMIT 1")?;
            let mut rows = stmt.query([])?;
            rows.next()?
                .map(|row| row.get::<_, String>(0))
                .transpose()?
                .map(|json| serde_json::from_str(&json).context("decode device sync config"))
                .transpose()
        })
    }

    pub fn save_device_sync_config(&self, config: &DeviceSyncConfig) -> Result<()> {
        if let Some(schedule) = &config.auto_sync_schedule {
            schedule.validate()?;
        }
        let json = serde_json::to_string(config)?;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO device_sync_config (id, config_json, updated_at)
                 VALUES (1, ?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET
                   config_json = excluded.config_json,
                   updated_at = excluded.updated_at",
                params![json, now_ms()],
            )?;
            Ok(())
        })
    }

    pub fn migrate_device_sync_startup_credential_consent(&self) -> Result<()> {
        if self
            .get_setting(DEVICE_SYNC_STARTUP_CREDENTIAL_CONSENT_MIGRATION)?
            .is_some()
        {
            return Ok(());
        }

        if let Some(mut config) = self.get_device_sync_config()? {
            config.auto_check = false;
            self.save_device_sync_config(&config)?;
        }
        self.set_setting(DEVICE_SYNC_STARTUP_CREDENTIAL_CONSENT_MIGRATION, "complete")
    }

    pub fn clear_device_sync_config(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute("DELETE FROM device_sync_config WHERE id = 1", [])?;
            Ok(())
        })
    }

    pub fn clear_device_sync_repository_state(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute_batch(
                "DELETE FROM device_sync_devices;
                 DELETE FROM device_sync_conflicts;
                 DELETE FROM device_sync_runs;",
            )?;
            Ok(())
        })
    }

    pub fn start_device_sync_run(&self, id: &str, started_at: i64) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO device_sync_runs (id, started_at, status)
                 VALUES (?1, ?2, 'running')",
                params![id, started_at],
            )?;
            Ok(())
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn finish_device_sync_run(
        &self,
        id: &str,
        finished_at: i64,
        status: &str,
        added: usize,
        updated: usize,
        deleted: usize,
        conflicted: usize,
        commit: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            let unchanged = status == "success"
                && added == 0
                && updated == 0
                && deleted == 0
                && conflicted == 0;
            tx.execute(
                "INSERT INTO settings (key, value) VALUES ('device_sync_last_run', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![serde_json::to_string(&(
                    if unchanged { "unchanged" } else { status },
                    finished_at
                ))?],
            )?;
            if unchanged {
                tx.execute("DELETE FROM device_sync_runs WHERE id = ?1", params![id])?;
                tx.commit()?;
                return Ok(());
            }
            tx.execute(
                "UPDATE device_sync_runs SET
                   finished_at = ?2, status = ?3, added = ?4, updated = ?5,
                   deleted = ?6, conflicted = ?7, commit_hash = ?8, error = ?9
                 WHERE id = ?1",
                params![
                    id,
                    finished_at,
                    status,
                    added as i64,
                    updated as i64,
                    deleted as i64,
                    conflicted as i64,
                    commit,
                    error.map(crate::core::device_sync::errors::safe_message)
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    pub fn list_device_sync_history(&self, limit: usize) -> Result<Vec<SyncHistoryEntry>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, started_at, finished_at, status, added, updated, deleted,
                        conflicted, commit_hash, error
                 FROM device_sync_runs ORDER BY started_at DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit as i64], |row| {
                Ok(SyncHistoryEntry {
                    id: row.get(0)?,
                    started_at: row.get(1)?,
                    finished_at: row.get(2)?,
                    status: row.get(3)?,
                    added: row.get::<_, i64>(4)? as usize,
                    updated: row.get::<_, i64>(5)? as usize,
                    deleted: row.get::<_, i64>(6)? as usize,
                    conflicted: row.get::<_, i64>(7)? as usize,
                    commit: row.get(8)?,
                    error: row
                        .get::<_, Option<String>>(9)?
                        .map(|error| crate::core::device_sync::errors::safe_message(&error)),
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(Into::into)
        })
    }

    pub fn upsert_device_sync_device(&self, device: &DeviceSyncDevice) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO device_sync_devices (id, name, last_commit, last_seen_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                   name = excluded.name,
                   last_commit = excluded.last_commit,
                   last_seen_at = MAX(device_sync_devices.last_seen_at, excluded.last_seen_at)",
                params![
                    device.id,
                    device.name,
                    device.last_commit,
                    device.last_seen_at
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_device_sync_devices(&self, current_id: &str) -> Result<Vec<DeviceSyncDevice>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT device.id, device.name, alias.value, device.last_commit, device.last_seen_at
                 FROM device_sync_devices AS device
                 LEFT JOIN settings AS alias
                   ON alias.key = 'device_sync.device_alias.' || device.id
                 ORDER BY CASE WHEN id = ?1 THEN 0 ELSE 1 END, last_seen_at DESC",
            )?;
            let rows = stmt.query_map(params![current_id], |row| {
                let id: String = row.get(0)?;
                Ok(DeviceSyncDevice {
                    is_current: id == current_id,
                    id,
                    name: row.get(1)?,
                    alias: row.get(2)?,
                    last_commit: row.get(3)?,
                    last_seen_at: row.get(4)?,
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(Into::into)
        })
    }

    pub fn set_device_sync_device_alias(&self, device_id: &str, alias: Option<&str>) -> Result<()> {
        self.with_conn(|conn| {
            let exists = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM device_sync_devices WHERE id = ?1)",
                params![device_id],
                |row| row.get::<_, bool>(0),
            )?;
            if !exists {
                bail!("device sync device not found");
            }
            let key = format!("device_sync.device_alias.{device_id}");
            let normalized = alias.map(str::trim).filter(|value| !value.is_empty());
            if normalized.is_some_and(|value| value.chars().count() > 80) {
                bail!("device alias must be 80 characters or fewer");
            }
            match normalized {
                Some(value) => {
                    conn.execute(
                        "INSERT INTO settings (key, value) VALUES (?1, ?2)
                         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                        params![key, value],
                    )?;
                }
                None => {
                    conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
                }
            }
            Ok(())
        })
    }

    pub fn upsert_device_sync_conflict(&self, conflict: &SyncConflict) -> Result<()> {
        let files_json = serde_json::to_string(&conflict.files)?;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO device_sync_conflicts (
                   id, skill_id, skill_name, base_commit, local_commit, remote_commit,
                   files_json, created_at, status
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                   files_json = excluded.files_json,
                   status = excluded.status",
                params![
                    conflict.id,
                    conflict.skill_id,
                    conflict.skill_name,
                    conflict.base_commit,
                    conflict.local_commit,
                    conflict.remote_commit,
                    files_json,
                    conflict.created_at,
                    conflict.status
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_device_sync_conflicts(&self) -> Result<Vec<SyncConflict>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, skill_id, skill_name, base_commit, local_commit, remote_commit,
                        files_json, created_at, status
                 FROM device_sync_conflicts
                 WHERE status = 'pending'
                 ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                let files_json: String = row.get(6)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    files_json,
                    row.get::<_, i64>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })?;
            let mut items = Vec::new();
            for row in rows {
                let (
                    id,
                    skill_id,
                    skill_name,
                    base_commit,
                    local_commit,
                    remote_commit,
                    files,
                    created_at,
                    status,
                ) = row?;
                items.push(SyncConflict {
                    id,
                    skill_id,
                    skill_name,
                    base_commit,
                    local_commit,
                    remote_commit,
                    files: serde_json::from_str(&files).context("decode conflict files")?,
                    created_at,
                    status,
                });
            }
            Ok(items)
        })
    }

    pub fn resolve_device_sync_conflicts_and_save_config_if_clear(
        &self,
        ids: &[String],
        config: &DeviceSyncConfig,
    ) -> Result<()> {
        let config_json = serde_json::to_string(config)?;
        self.with_conn(|conn| {
            let transaction = conn.unchecked_transaction()?;
            for id in ids {
                transaction.execute(
                    "UPDATE device_sync_conflicts SET status = 'resolved'
                     WHERE id = ?1 AND status = 'pending'",
                    params![id],
                )?;
            }
            let pending: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM device_sync_conflicts WHERE status = 'pending'",
                [],
                |row| row.get(0),
            )?;
            if pending == 0 {
                transaction.execute(
                    "INSERT INTO device_sync_config (id, config_json, updated_at)
                     VALUES (1, ?1, ?2)
                     ON CONFLICT(id) DO UPDATE SET
                       config_json = excluded.config_json,
                       updated_at = excluded.updated_at",
                    params![config_json, now_ms()],
                )?;
            }
            transaction.commit()?;
            Ok(())
        })
    }

    pub fn add_device_sync_trash(&self, entry: &TrashEntry) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO device_sync_tombstones
                 (id, skill_id, skill_name, trash_path, deleted_at, expires_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    entry.id,
                    entry.skill_id,
                    entry.skill_name,
                    entry.trash_path,
                    entry.deleted_at,
                    entry.expires_at
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_device_sync_trash(&self) -> Result<Vec<TrashEntry>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, skill_id, skill_name, trash_path, deleted_at, expires_at
                 FROM device_sync_tombstones ORDER BY deleted_at DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(TrashEntry {
                    id: row.get(0)?,
                    skill_id: row.get(1)?,
                    skill_name: row.get(2)?,
                    trash_path: row.get(3)?,
                    deleted_at: row.get(4)?,
                    expires_at: row.get(5)?,
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(Into::into)
        })
    }

    pub fn remove_device_sync_trash(&self, id: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM device_sync_tombstones WHERE id = ?1",
                params![id],
            )?;
            Ok(())
        })
    }

    #[allow(dead_code)]
    pub fn set_onboarding_completed(&self, completed: bool) -> Result<()> {
        self.set_setting(
            "onboarding_completed",
            if completed { "true" } else { "false" },
        )
    }

    pub fn upsert_skill(&self, record: &SkillRecord) -> Result<()> {
        self.with_conn(|conn| {
            upsert_skill_with_conn(conn, record)?;
            Ok(())
        })
    }

    pub fn record_source_failure(&self, skill_id: &str, raw: &str) -> Result<()> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute("UPDATE skills SET status='error' WHERE id=?1", params![skill_id])?;
            tx.execute("INSERT INTO skill_source_checks (skill_id,error_code,checked_at) VALUES (?1,?2,?3)
                ON CONFLICT(skill_id) DO UPDATE SET error_code=excluded.error_code,checked_at=excluded.checked_at",
                params![skill_id, super::skill_issues::safe_code(raw), now_ms()])?;
            tx.commit()?;
            Ok(())
        })
    }

    pub fn source_checks(
        &self,
    ) -> Result<std::collections::HashMap<String, (Option<String>, i64)>> {
        self.with_conn(|conn| {
            let mut statement =
                conn.prepare("SELECT skill_id,error_code,checked_at FROM skill_source_checks")?;
            let rows =
                statement.query_map([], |row| Ok((row.get(0)?, (row.get(1)?, row.get(2)?))))?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }

    pub fn upsert_skill_target(&self, record: &SkillTargetRecord) -> Result<()> {
        self.with_conn(|conn| {
            upsert_skill_target_with_conn(conn, record)?;
            Ok(())
        })
    }

    pub fn commit_skill_update(
        &self,
        skill: &SkillRecord,
        targets: &[SkillTargetRecord],
    ) -> Result<()> {
        self.with_conn(|conn| {
            let transaction = conn.unchecked_transaction()?;
            upsert_skill_with_conn(&transaction, skill)?;
            transaction.execute("INSERT INTO skill_source_checks (skill_id,error_code,checked_at) VALUES (?1,NULL,?2)
                ON CONFLICT(skill_id) DO UPDATE SET error_code=NULL,checked_at=excluded.checked_at", params![skill.id, now_ms()])?;
            for target in targets {
                upsert_skill_target_with_conn(&transaction, target)?;
            }
            transaction.commit()?;
            Ok(())
        })
    }

    pub(crate) fn commit_device_sync_library(
        &self,
        skills: &[(SkillRecord, Vec<String>)],
        targets: &[(SkillTargetRecord, String)],
    ) -> Result<()> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            let now = now_ms();
            for (skill, tags) in skills {
                let mut local = skill.clone();
                if let Some(status) = tx.query_row("SELECT status FROM skills WHERE id=?1", params![skill.id], |row| row.get::<_,String>(0)).optional()? {
                    local.status = status;
                }
                upsert_skill_with_conn(&tx, &local)?;
                tx.execute("DELETE FROM skill_tag_links WHERE skill_id = ?1", params![skill.id])?;
                for name in tags {
                    let normalized = normalize_tag_name(name)?;
                    tx.execute("INSERT INTO skill_tags (name, created_at, updated_at) VALUES (?1, ?2, ?2) ON CONFLICT(name) DO NOTHING", params![normalized, now])?;
                    let tag_id: i64 = tx.query_row("SELECT id FROM skill_tags WHERE name = ?1 COLLATE NOCASE", params![normalized], |row| row.get(0))?;
                    tx.execute("INSERT OR IGNORE INTO skill_tag_links (skill_id, tag_id, created_at) VALUES (?1, ?2, ?3)", params![skill.id, tag_id, now])?;
                }
            }
            for (target, hash) in targets {
                upsert_skill_target_with_conn(&tx, target)?;
                tx.execute("INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    params![format!("device_sync.target_baseline.{}", target.id), serde_json::to_string(&(&target.target_path, hash))?])?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    pub fn list_skills(&self) -> Result<Vec<SkillRecord>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
        "SELECT id, name, description, source_type, source_ref, source_subpath, source_revision, central_path, content_hash,
                created_at, updated_at, last_sync_at, last_seen_at, enabled, status
         FROM skills
         ORDER BY updated_at DESC",
      )?;
            let rows = stmt.query_map([], |row| {
                Ok(SkillRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    source_type: row.get(3)?,
                    source_ref: row.get(4)?,
                    source_subpath: row.get(5)?,
                    source_revision: row.get(6)?,
                    central_path: row.get(7)?,
                    content_hash: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                    last_sync_at: row.get(11)?,
                    last_seen_at: row.get(12)?,
                    enabled: row.get::<_, i32>(13)? != 0,
                    status: row.get(14)?,
                })
            })?;

            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            Ok(items)
        })
    }

    pub fn get_skill_by_id(&self, skill_id: &str) -> Result<Option<SkillRecord>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
        "SELECT id, name, description, source_type, source_ref, source_subpath, source_revision, central_path, content_hash,
                created_at, updated_at, last_sync_at, last_seen_at, enabled, status
         FROM skills
         WHERE id = ?1
         LIMIT 1",
      )?;
            let mut rows = stmt.query(params![skill_id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(SkillRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    source_type: row.get(3)?,
                    source_ref: row.get(4)?,
                    source_subpath: row.get(5)?,
                    source_revision: row.get(6)?,
                    central_path: row.get(7)?,
                    content_hash: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                    last_sync_at: row.get(11)?,
                    last_seen_at: row.get(12)?,
                    enabled: row.get::<_, i32>(13)? != 0,
                    status: row.get(14)?,
                }))
            } else {
                Ok(None)
            }
        })
    }

    pub fn update_skill_description(
        &self,
        skill_id: &str,
        description: Option<&str>,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE skills SET description = ?1 WHERE id = ?2",
                params![description, skill_id],
            )?;
            Ok(())
        })
    }

    pub fn set_skill_enabled(&self, skill_id: &str, enabled: bool) -> Result<()> {
        self.with_conn(|conn| {
            let changed = conn.execute(
                "UPDATE skills SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
                params![enabled as i32, now_ms(), skill_id],
            )?;
            if changed == 0 {
                anyhow::bail!("skill not found: {}", skill_id);
            }
            Ok(())
        })
    }

    pub fn delete_skill(&self, skill_id: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute("DELETE FROM skills WHERE id = ?1", params![skill_id])?;
            Ok(())
        })
    }

    pub fn adopt_skill_id(&self, old_id: &str, new_id: &str) -> Result<()> {
        if old_id == new_id || self.get_skill_by_id(new_id)?.is_some() {
            return Ok(());
        }
        self.with_conn(|conn| {
            let original_path: String = conn.query_row(
                "SELECT central_path FROM skills WHERE id = ?1",
                params![old_id],
                |row| row.get(0),
            )?;
            let temporary_path = format!("{}.identity-{}", original_path, Uuid::new_v4());
            conn.execute_batch("BEGIN;")?;
            let result = (|| -> Result<()> {
                conn.execute(
                    "UPDATE skills SET central_path = ?1 WHERE id = ?2",
                    params![temporary_path, old_id],
                )?;
                conn.execute(
                    "INSERT INTO skills (
                       id, name, description, source_type, source_ref, source_subpath,
                       source_revision, central_path, content_hash, created_at, updated_at,
                       last_sync_at, last_seen_at, enabled, status
                     ) SELECT ?1, name, description, source_type, source_ref, source_subpath,
                       source_revision, ?2, content_hash, created_at, updated_at,
                       last_sync_at, last_seen_at, enabled, status
                     FROM skills WHERE id = ?3",
                    params![new_id, original_path, old_id],
                )?;
                conn.execute(
                    "UPDATE skill_targets SET skill_id = ?1 WHERE skill_id = ?2",
                    params![new_id, old_id],
                )?;
                conn.execute(
                    "UPDATE skill_tag_links SET skill_id = ?1 WHERE skill_id = ?2",
                    params![new_id, old_id],
                )?;
                conn.execute(
                    "UPDATE skill_source_checks SET skill_id=?1 WHERE skill_id=?2",
                    params![new_id, old_id],
                )?;
                conn.execute("DELETE FROM skills WHERE id = ?1", params![old_id])?;
                Ok(())
            })();
            match result {
                Ok(()) => {
                    conn.execute_batch("COMMIT;")?;
                    Ok(())
                }
                Err(err) => {
                    let _ = conn.execute_batch("ROLLBACK;");
                    Err(err)
                }
            }
        })
    }

    pub fn create_tag(&self, name: &str) -> Result<TagRecord> {
        let normalized = normalize_tag_name(name)?;
        self.with_conn(|conn| {
            let now = now_ms();
            conn.execute(
                "INSERT INTO skill_tags (name, created_at, updated_at) VALUES (?1, ?2, ?2)",
                params![normalized, now],
            )
            .with_context(|| format!("tag already exists: {}", normalized))?;
            let id = conn.last_insert_rowid();
            Ok(TagRecord {
                id,
                name: normalized,
            })
        })
    }

    pub fn rename_tag(&self, tag_id: i64, name: &str) -> Result<TagRecord> {
        let normalized = normalize_tag_name(name)?;
        self.with_conn(|conn| {
            let changed = conn
                .execute(
                    "UPDATE skill_tags SET name = ?1, updated_at = ?2 WHERE id = ?3",
                    params![normalized, now_ms(), tag_id],
                )
                .with_context(|| format!("tag already exists: {}", normalized))?;
            if changed == 0 {
                anyhow::bail!("tag not found: {}", tag_id);
            }
            Ok(TagRecord {
                id: tag_id,
                name: normalized,
            })
        })
    }

    pub fn delete_tag(&self, tag_id: i64) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute("DELETE FROM skill_tags WHERE id = ?1", params![tag_id])?;
            Ok(())
        })
    }

    pub fn list_tags_with_counts(&self) -> Result<Vec<TagWithCountRecord>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT t.id, t.name, COUNT(l.skill_id) AS skill_count,
                        COALESCE(MAX(l.created_at), t.updated_at) AS last_used_at
                 FROM skill_tags t
                 LEFT JOIN skill_tag_links l ON l.tag_id = t.id
                 GROUP BY t.id, t.name, t.updated_at
                 ORDER BY LOWER(t.name) ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(TagWithCountRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    skill_count: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?;

            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            Ok(items)
        })
    }

    pub fn get_skill_tags(&self, skill_id: &str) -> Result<Vec<TagRecord>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT t.id, t.name
                 FROM skill_tags t
                 INNER JOIN skill_tag_links l ON l.tag_id = t.id
                 WHERE l.skill_id = ?1
                 ORDER BY LOWER(t.name) ASC",
            )?;
            let rows = stmt.query_map(params![skill_id], |row| {
                Ok(TagRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })?;

            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            Ok(items)
        })
    }

    pub fn set_skill_tags(&self, skill_id: &str, tag_ids: &[i64]) -> Result<()> {
        self.with_conn(|conn| {
            let now = now_ms();
            conn.execute_batch("BEGIN;")?;
            let result = (|| -> Result<()> {
                conn.execute(
                    "DELETE FROM skill_tag_links WHERE skill_id = ?1",
                    params![skill_id],
                )?;
                for tag_id in tag_ids {
                    conn.execute(
                        "INSERT OR IGNORE INTO skill_tag_links (skill_id, tag_id, created_at)
                         VALUES (?1, ?2, ?3)",
                        params![skill_id, tag_id, now],
                    )?;
                }
                Ok(())
            })();

            match result {
                Ok(()) => {
                    conn.execute_batch("COMMIT;")?;
                    Ok(())
                }
                Err(err) => {
                    let _ = conn.execute_batch("ROLLBACK;");
                    Err(err)
                }
            }
        })
    }

    pub fn set_skill_tag_names(&self, skill_id: &str, tag_names: &[String]) -> Result<()> {
        self.with_conn(|conn| {
            let now = now_ms();
            conn.execute_batch("BEGIN;")?;
            let result = (|| -> Result<()> {
                conn.execute(
                    "DELETE FROM skill_tag_links WHERE skill_id = ?1",
                    params![skill_id],
                )?;
                for name in tag_names {
                    let normalized = normalize_tag_name(name)?;
                    conn.execute(
                        "INSERT INTO skill_tags (name, created_at, updated_at)
                         VALUES (?1, ?2, ?2)
                         ON CONFLICT(name) DO NOTHING",
                        params![normalized, now],
                    )?;
                    let tag_id: i64 = conn.query_row(
                        "SELECT id FROM skill_tags WHERE name = ?1 COLLATE NOCASE",
                        params![normalized],
                        |row| row.get(0),
                    )?;
                    conn.execute(
                        "INSERT OR IGNORE INTO skill_tag_links (skill_id, tag_id, created_at)
                         VALUES (?1, ?2, ?3)",
                        params![skill_id, tag_id, now],
                    )?;
                }
                Ok(())
            })();
            match result {
                Ok(()) => {
                    conn.execute_batch("COMMIT;")?;
                    Ok(())
                }
                Err(err) => {
                    let _ = conn.execute_batch("ROLLBACK;");
                    Err(err)
                }
            }
        })
    }

    pub fn list_untagged_skill_ids(&self) -> Result<Vec<String>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT s.id
                 FROM skills s
                 WHERE NOT EXISTS (
                   SELECT 1 FROM skill_tag_links l WHERE l.skill_id = s.id
                 )
                 ORDER BY s.updated_at DESC",
            )?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            Ok(items)
        })
    }

    pub fn list_skill_targets(&self, skill_id: &str) -> Result<Vec<SkillTargetRecord>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, skill_id, tool, scope, project_path, target_path, mode, status, last_error, synced_at
         FROM skill_targets
         WHERE skill_id = ?1
         ORDER BY tool ASC, scope ASC, project_path ASC",
            )?;
            let rows = stmt.query_map(params![skill_id], |row| {
                Ok(SkillTargetRecord {
                    id: row.get(0)?,
                    skill_id: row.get(1)?,
                    tool: row.get(2)?,
                    scope: row.get(3)?,
                    project_path: row.get(4)?,
                    target_path: row.get(5)?,
                    mode: row.get(6)?,
                    status: row.get(7)?,
                    last_error: row.get(8)?,
                    synced_at: row.get(9)?,
                })
            })?;

            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            Ok(items)
        })
    }

    pub fn list_skill_targets_by_tool(&self, tool: &str) -> Result<Vec<SkillTargetRecord>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, skill_id, tool, scope, project_path, target_path, mode, status, last_error, synced_at
         FROM skill_targets
         WHERE tool = ?1
         ORDER BY skill_id ASC, scope ASC, project_path ASC",
            )?;
            let rows = stmt.query_map(params![tool], |row| {
                Ok(SkillTargetRecord {
                    id: row.get(0)?,
                    skill_id: row.get(1)?,
                    tool: row.get(2)?,
                    scope: row.get(3)?,
                    project_path: row.get(4)?,
                    target_path: row.get(5)?,
                    mode: row.get(6)?,
                    status: row.get(7)?,
                    last_error: row.get(8)?,
                    synced_at: row.get(9)?,
                })
            })?;

            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            Ok(items)
        })
    }

    pub fn is_skill_target_path_used_by_another_record(
        &self,
        target_path: &str,
        record_id: &str,
    ) -> Result<bool> {
        self.with_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*)
                 FROM skill_targets
                 WHERE target_path = ?1
                   AND id != ?2
                   AND status != 'disabled'",
                params![target_path, record_id],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    pub fn is_target_used_by_other_skill(&self, path: &str, skill_id: &str) -> Result<bool> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM skill_targets WHERE target_path = ?1 AND skill_id != ?2 AND status != 'disabled')",
                params![path, skill_id],
                |row| row.get(0),
            ).map_err(Into::into)
        })
    }

    pub fn list_tool_sync_issues(
        &self,
    ) -> Result<Vec<crate::core::device_sync::types::ToolSyncIssue>> {
        self.with_conn(|conn| {
            let mut statement = conn.prepare("SELECT DISTINCT s.name, t.tool FROM skill_targets t JOIN skills s ON s.id=t.skill_id WHERE s.enabled=1 AND t.status='error' ORDER BY s.name,t.tool")?;
            let rows = statement.query_map([], |row| Ok(crate::core::device_sync::types::ToolSyncIssue {
                skill_name: row.get(0)?, tool: row.get(1)?,
            }))?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn list_all_skill_target_paths(&self) -> Result<Vec<(String, String)>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT tool, target_path
         FROM skill_targets",
            )?;
            let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;

            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            Ok(items)
        })
    }

    pub fn get_skill_target(
        &self,
        skill_id: &str,
        tool: &str,
        scope: &str,
        project_path: Option<&str>,
    ) -> Result<Option<SkillTargetRecord>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, skill_id, tool, scope, project_path, target_path, mode, status, last_error, synced_at
         FROM skill_targets
         WHERE skill_id = ?1
           AND tool = ?2
           AND scope = ?3
           AND ((?4 IS NULL AND project_path IS NULL) OR project_path = ?4)",
            )?;
            let mut rows = stmt.query(params![skill_id, tool, scope, project_path])?;
            if let Some(row) = rows.next()? {
                Ok(Some(SkillTargetRecord {
                    id: row.get(0)?,
                    skill_id: row.get(1)?,
                    tool: row.get(2)?,
                    scope: row.get(3)?,
                    project_path: row.get(4)?,
                    target_path: row.get(5)?,
                    mode: row.get(6)?,
                    status: row.get(7)?,
                    last_error: row.get(8)?,
                    synced_at: row.get(9)?,
                }))
            } else {
                Ok(None)
            }
        })
    }

    pub fn delete_skill_target(
        &self,
        skill_id: &str,
        tool: &str,
        scope: &str,
        project_path: Option<&str>,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM skill_targets
                 WHERE skill_id = ?1
                   AND tool = ?2
                   AND scope = ?3
                   AND ((?4 IS NULL AND project_path IS NULL) OR project_path = ?4)",
                params![skill_id, tool, scope, project_path],
            )?;
            Ok(())
        })
    }

    pub fn update_skill_target_status(
        &self,
        skill_id: &str,
        tool: &str,
        scope: &str,
        project_path: Option<&str>,
        status: &str,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE skill_targets
                 SET status = ?5
                 WHERE skill_id = ?1
                   AND tool = ?2
                   AND scope = ?3
                   AND ((?4 IS NULL AND project_path IS NULL) OR project_path = ?4)",
                params![skill_id, tool, scope, project_path, status],
            )?;
            Ok(())
        })
    }

    fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let conn = Connection::open(&self.db_path)
            .with_context(|| format!("failed to open db at {:?}", self.db_path))?;
        // Enforce foreign key constraints on every connection (rusqlite PRAGMA is per-connection).
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        f(&conn)
    }
}

fn upsert_skill_with_conn(conn: &Connection, record: &SkillRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO skills (
          id, name, description, source_type, source_ref, source_subpath, source_revision, central_path, content_hash,
          created_at, updated_at, last_sync_at, last_seen_at, enabled, status
        ) VALUES (
          ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
          ?10, ?11, ?12, ?13, ?14, ?15
        )
        ON CONFLICT(id) DO UPDATE SET
          name = excluded.name,
          description = excluded.description,
          source_type = excluded.source_type,
          source_ref = excluded.source_ref,
          source_subpath = excluded.source_subpath,
          source_revision = excluded.source_revision,
          central_path = excluded.central_path,
          content_hash = excluded.content_hash,
          created_at = excluded.created_at,
          updated_at = excluded.updated_at,
          last_sync_at = excluded.last_sync_at,
          last_seen_at = excluded.last_seen_at,
          enabled = excluded.enabled,
          status = excluded.status",
        params![
            record.id,
            record.name,
            record.description,
            record.source_type,
            record.source_ref,
            record.source_subpath,
            record.source_revision,
            record.central_path,
            record.content_hash,
            record.created_at,
            record.updated_at,
            record.last_sync_at,
            record.last_seen_at,
            record.enabled as i32,
            record.status
        ],
    )?;
    Ok(())
}

fn upsert_skill_target_with_conn(conn: &Connection, record: &SkillTargetRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO skill_targets (
          id, skill_id, tool, scope, project_path, target_path, mode, status, last_error, synced_at
        ) VALUES (
          ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10
        )
        ON CONFLICT DO UPDATE SET
          target_path = excluded.target_path,
          mode = excluded.mode,
          status = excluded.status,
          last_error = excluded.last_error,
          synced_at = excluded.synced_at",
        params![
            record.id,
            record.skill_id,
            record.tool,
            record.scope,
            record.project_path,
            record.target_path,
            record.mode,
            record.status,
            record.last_error,
            record.synced_at
        ],
    )?;
    Ok(())
}

fn truncate_sqlite_wal(conn: &Connection) -> Result<()> {
    let (busy, _, _): (i64, i64, i64) =
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
    if busy != 0 {
        anyhow::bail!("secure setting deletion could not truncate the SQLite WAL");
    }
    Ok(())
}

fn migrate_skill_targets_to_v4(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "BEGIN;
         DROP INDEX IF EXISTS idx_skill_targets_unique_scope;
         CREATE TABLE skill_targets_new (
           id TEXT PRIMARY KEY,
           skill_id TEXT NOT NULL,
           tool TEXT NOT NULL,
           scope TEXT NOT NULL DEFAULT 'global',
           project_path TEXT NULL,
           target_path TEXT NOT NULL,
           mode TEXT NOT NULL,
           status TEXT NOT NULL,
           last_error TEXT NULL,
           synced_at INTEGER NULL,
           FOREIGN KEY(skill_id) REFERENCES skills(id) ON DELETE CASCADE
         );
         INSERT INTO skill_targets_new (
           id, skill_id, tool, scope, project_path, target_path, mode, status, last_error, synced_at
         )
         SELECT id, skill_id, tool, 'global', NULL, target_path, mode, status, last_error, synced_at
         FROM skill_targets;
         DROP TABLE skill_targets;
         ALTER TABLE skill_targets_new RENAME TO skill_targets;
         CREATE UNIQUE INDEX idx_skill_targets_unique_scope
         ON skill_targets(skill_id, tool, scope, COALESCE(project_path, ''));
         COMMIT;",
    )?;
    Ok(())
}

fn migrate_tags_to_v5(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS skill_tags (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           name TEXT NOT NULL UNIQUE COLLATE NOCASE,
           created_at INTEGER NOT NULL,
           updated_at INTEGER NOT NULL
         );

         CREATE TABLE IF NOT EXISTS skill_tag_links (
           skill_id TEXT NOT NULL,
           tag_id INTEGER NOT NULL,
           created_at INTEGER NOT NULL,
           PRIMARY KEY (skill_id, tag_id),
           FOREIGN KEY(skill_id) REFERENCES skills(id) ON DELETE CASCADE,
           FOREIGN KEY(tag_id) REFERENCES skill_tags(id) ON DELETE CASCADE
         );",
    )?;
    Ok(())
}

fn migrate_skill_enabled_to_v6(conn: &Connection) -> Result<()> {
    conn.execute_batch("ALTER TABLE skills ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1;")?;
    Ok(())
}

fn normalize_tag_name(name: &str) -> Result<String> {
    let normalized = name.trim().to_string();
    if normalized.is_empty() {
        anyhow::bail!("tag name cannot be empty");
    }
    Ok(normalized)
}

fn now_ms() -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    now.as_millis() as i64
}

pub fn default_db_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf> {
    let app_dir = app
        .path()
        .app_data_dir()
        .context("failed to resolve app data dir")?;
    std::fs::create_dir_all(&app_dir)
        .with_context(|| format!("failed to create app data dir {:?}", app_dir))?;
    Ok(app_dir.join(DB_FILE_NAME))
}

pub fn migrate_legacy_db_if_needed(target_db_path: &Path) -> Result<()> {
    let Some(data_dir) = dirs::data_dir() else {
        return Ok(());
    };

    migrate_legacy_db_if_needed_in_data_dir(target_db_path, &data_dir)
}

fn migrate_legacy_db_if_needed_in_data_dir(target_db_path: &Path, data_dir: &Path) -> Result<()> {
    let legacy_db_paths = LEGACY_APP_IDENTIFIERS
        .iter()
        .map(|id| data_dir.join(id).join(DB_FILE_NAME))
        .collect::<Vec<_>>();

    if let Ok(true) = db_has_any_skills(target_db_path) {
        scrub_app_managed_historical_databases(target_db_path, &legacy_db_paths)?;
        return Ok(());
    }

    let legacy_db_path = legacy_db_paths.iter().find(|path| path.exists());

    let Some(legacy_db_path) = legacy_db_path else {
        scrub_app_managed_historical_databases(target_db_path, &legacy_db_paths)?;
        return Ok(());
    };

    if legacy_db_path == target_db_path {
        scrub_app_managed_historical_databases(target_db_path, &legacy_db_paths)?;
        return Ok(());
    }

    if let Some(parent) = target_db_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create app data dir {:?}", parent))?;
    }

    if target_db_path.exists() {
        let backup = target_db_path.with_extension(format!(
            "bak-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        ));
        std::fs::rename(target_db_path, &backup).with_context(|| {
            format!(
                "failed to backup existing db {:?} -> {:?}",
                target_db_path, backup
            )
        })?;
    }

    std::fs::copy(legacy_db_path, target_db_path).with_context(|| {
        format!(
            "failed to migrate legacy db {:?} -> {:?}",
            legacy_db_path, target_db_path
        )
    })?;

    scrub_app_managed_historical_databases(target_db_path, &legacy_db_paths)?;
    Ok(())
}

fn scrub_app_managed_historical_databases(
    target_db_path: &Path,
    legacy_db_paths: &[PathBuf],
) -> Result<()> {
    let mut historical_paths = legacy_db_paths
        .iter()
        .filter(|path| path.as_path() != target_db_path)
        .filter(|path| path.exists())
        .cloned()
        .collect::<Vec<_>>();
    historical_paths.extend(app_created_database_backups(target_db_path)?);
    historical_paths.sort();
    historical_paths.dedup();

    for path in historical_paths {
        scrub_legacy_github_token_from_database(&path)
            .with_context(|| format!("securely scrub legacy GitHub token from {:?}", path))?;
    }
    Ok(())
}

fn app_created_database_backups(target_db_path: &Path) -> Result<Vec<PathBuf>> {
    let Some(parent) = target_db_path.parent() else {
        return Ok(Vec::new());
    };
    if !parent.exists() {
        return Ok(Vec::new());
    }
    let Some(stem) = target_db_path.file_stem().and_then(|value| value.to_str()) else {
        return Ok(Vec::new());
    };
    let backup_prefix = format!("{stem}.bak-");
    let mut backups = Vec::new();
    for entry in std::fs::read_dir(parent)
        .with_context(|| format!("list app data directory {:?}", parent))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name.starts_with(&backup_prefix)
            && !file_name.ends_with("-wal")
            && !file_name.ends_with("-shm")
            && !file_name.ends_with("-journal")
        {
            backups.push(entry.path());
        }
    }
    Ok(backups)
}

fn scrub_legacy_github_token_from_database(db_path: &Path) -> Result<()> {
    let conn = Connection::open(db_path)
        .with_context(|| format!("failed to open historical db at {:?}", db_path))?;
    let has_settings_table: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='settings'",
        [],
        |row| row.get(0),
    )?;
    drop(conn);
    if has_settings_table == 0 {
        return Ok(());
    }

    SkillStore::new(db_path.to_path_buf()).secure_delete_setting_with_pending_marker(
        LEGACY_GITHUB_TOKEN_SETTING,
        GITHUB_TOKEN_SECURE_CLEANUP_PENDING_SETTING,
    )
}

fn db_has_any_skills(db_path: &Path) -> Result<bool> {
    if !db_path.exists() {
        return Ok(false);
    }

    let conn =
        Connection::open(db_path).with_context(|| format!("failed to open db at {:?}", db_path))?;
    let has_table: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='skills';",
        [],
        |row| row.get(0),
    )?;
    if has_table == 0 {
        return Ok(false);
    }

    let count: i64 = conn.query_row("SELECT COUNT(*) FROM skills;", [], |row| row.get(0))?;
    Ok(count > 0)
}

#[cfg(test)]
#[path = "tests/skill_store.rs"]
mod tests;
