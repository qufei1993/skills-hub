//! Local-only preset definitions and durable operation journals.
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::skill_store::SkillStore;

const APPLICATION_HISTORY_LIMIT: usize = 100;

fn expired_application_ids(conn: &Connection, keep_id: Option<&str>) -> Result<Vec<String>> {
    // Unknown or unfinished states remain recoverable. Keep target-use markers even
    // after their journal expires: an older application must not regain undo rights.
    let ids = {
        let mut statement = conn.prepare(
            "SELECT id FROM workspace_documents
             WHERE kind='application' AND json_extract(payload,'$.record.confirmed')=1
               AND NOT EXISTS (
                   SELECT 1 FROM json_each(payload,'$.record.items')
                   WHERE COALESCE(json_extract(value,'$.status'),'')
                       NOT IN ('done','undone','failed','blocked')
               )
             ORDER BY (id=COALESCE(?1,'')) DESC,updated_at DESC,id
             LIMIT -1 OFFSET ?2",
        )?;
        let rows = statement.query_map(params![keep_id, APPLICATION_HISTORY_LIMIT], |row| {
            row.get::<_, String>(0)
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    Ok(ids)
}

fn prune_application_history(conn: &Connection, keep_id: Option<&str>) -> Result<()> {
    let ids = expired_application_ids(conn, keep_id)?;
    // Do not acquire a SQLite writer reservation for the usual read-only case.
    for id in ids {
        conn.execute(
            "DELETE FROM workspace_documents WHERE kind IN ('application','undo-preview') AND id=?1",
            [id],
        )?;
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PresetMember {
    pub skill_id: String,
    pub tools: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub members: Vec<PresetMember>,
    pub revision: u64,
}

pub(crate) fn migrate(conn: &Connection) -> Result<()> {
    let version: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key='schema.workspace'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    anyhow::ensure!(
        version.as_deref().map_or(true, |v| v == "1"),
        "unsupported workspace schema"
    );
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS workspace_documents (
        kind TEXT NOT NULL, id TEXT NOT NULL, payload TEXT NOT NULL,
        updated_at INTEGER NOT NULL, PRIMARY KEY(kind,id)
    );",
    )?;
    tx.execute("INSERT INTO settings(key,value) VALUES ('schema.workspace','1') ON CONFLICT(key) DO NOTHING", [])?;
    tx.commit()?;
    Ok(())
}

impl SkillStore {
    pub(crate) fn workspace_save<T: Serialize>(
        &self,
        kind: &str,
        id: &str,
        value: &T,
    ) -> Result<()> {
        let conn = Connection::open(self.db_path()).context("open workspace journal")?;
        let tx = conn.unchecked_transaction()?;
        tx.execute("INSERT INTO workspace_documents(kind,id,payload,updated_at) VALUES (?1,?2,?3,?4) ON CONFLICT(kind,id) DO UPDATE SET payload=excluded.payload,updated_at=excluded.updated_at",
            params![kind,id,serde_json::to_string(value)?,chrono::Utc::now().timestamp_millis()])?;
        if kind == "application" {
            prune_application_history(&tx, Some(id))?;
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn workspace_prune_applications(&self) -> Result<()> {
        let conn = Connection::open(self.db_path()).context("open workspace journal")?;
        let tx = conn.unchecked_transaction()?;
        prune_application_history(&tx, None)?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn workspace_applications_need_pruning(&self) -> Result<bool> {
        let conn = Connection::open(self.db_path()).context("open workspace journal")?;
        Ok(!expired_application_ids(&conn, None)?.is_empty())
    }

    pub(crate) fn workspace_delete_application(&self, id: &str) -> Result<()> {
        let conn = Connection::open(self.db_path()).context("open workspace journal")?;
        conn.execute(
            "DELETE FROM workspace_documents WHERE kind IN ('application','undo-preview') AND id=?1",
            [id],
        )?;
        Ok(())
    }

    pub(crate) fn workspace_get<T: DeserializeOwned>(
        &self,
        kind: &str,
        id: &str,
    ) -> Result<Option<T>> {
        let conn = Connection::open(self.db_path()).context("open workspace journal")?;
        let payload: Option<String> = conn
            .query_row(
                "SELECT payload FROM workspace_documents WHERE kind=?1 AND id=?2",
                params![kind, id],
                |row| row.get(0),
            )
            .optional()?;
        payload
            .map(|json| serde_json::from_str(&json).context("read workspace record"))
            .transpose()
    }

    pub(crate) fn workspace_list<T: DeserializeOwned>(&self, kind: &str) -> Result<Vec<T>> {
        let conn = Connection::open(self.db_path()).context("open workspace journal")?;
        let mut stmt = conn.prepare(
            "SELECT payload FROM workspace_documents WHERE kind=?1 ORDER BY updated_at DESC,id",
        )?;
        let rows = stmt.query_map([kind], |row| row.get::<_, String>(0))?;
        rows.map(|row| serde_json::from_str(&row?).context("read workspace record"))
            .collect()
    }

    pub(crate) fn workspace_delete(&self, kind: &str, id: &str) -> Result<()> {
        let conn = Connection::open(self.db_path()).context("open workspace journal")?;
        conn.execute(
            "DELETE FROM workspace_documents WHERE kind=?1 AND id=?2",
            params![kind, id],
        )?;
        Ok(())
    }
}
