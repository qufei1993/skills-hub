use super::{
    network_proxy::{app_http_client, get_github_proxy_url},
    skill_store::SkillStore,
};
use anyhow::{ensure, Context, Result};
use serde_json::Value;
use std::io::Read;

const BASE: &str = "https://skills-hub-collections.qzfweb.workers.dev";
const LIMIT: u64 = 32 * 1024 * 1024;
const INDEX_KEY: &str = "website_collections_snapshots_v1_index";

fn valid_version(version: &str) -> bool {
    version.len() == 64
        && version
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

fn filename(id: Option<&str>) -> Result<String> {
    match id {
        None => Ok("index.json".into()),
        Some(id) => {
            let parsed = uuid::Uuid::parse_str(id).context("invalid collection ID")?;
            ensure!(parsed.to_string() == id, "invalid collection ID");
            Ok(format!("{id}.json"))
        }
    }
}

fn pointer_version(value: &Value) -> Result<&str> {
    let version = value["version"]
        .as_str()
        .context("missing snapshot version")?;
    ensure!(
        value["schemaVersion"] == 1
            && valid_version(version)
            && value["preview"] == false
            && value["snapshotPath"] == format!("snapshots/{version}/"),
        "invalid collection publication pointer"
    );
    Ok(version)
}

fn validate(body: &str, id: Option<&str>) -> Result<Value> {
    let value: Value = serde_json::from_str(body)?;
    ensure!(value["schemaVersion"] == 1, "unsupported collection schema");
    if let Some(id) = id {
        ensure!(
            value["id"] == id
                && value["status"] == "active"
                && value["review"]["status"] == "approved",
            "unpublished collection"
        );
    } else {
        ensure!(value["collections"].is_array(), "invalid collection index");
    }
    Ok(value)
}

fn cache_key(id: Option<&str>, version: Option<&str>) -> Result<String> {
    let file = filename(id)?;
    if id.is_none() {
        return Ok(INDEX_KEY.into());
    }
    let version = version.context("collection details require an index snapshot")?;
    ensure!(valid_version(version), "invalid snapshot version");
    Ok(format!("website_collections_snapshots_v1_{version}_{file}"))
}

pub fn cached(store: &SkillStore, id: Option<&str>, version: Option<&str>) -> Result<Value> {
    let key = cache_key(id, version)?;
    let body = store
        .get_setting(&key)?
        .context("no cached collection snapshot")?;
    let value = validate(&body, id)?;
    let stored_version = value["snapshotVersion"]
        .as_str()
        .context("missing cached version")?;
    ensure!(valid_version(stored_version), "invalid cached version");
    if let Some(version) = version {
        ensure!(
            stored_version == version,
            "cached collection snapshot mismatch"
        );
    }
    Ok(value)
}

// Only persist after the frontend has validated the full collection contract.
pub fn cache_validated(store: &SkillStore, id: Option<&str>, value: Value) -> Result<()> {
    let version = value["snapshotVersion"]
        .as_str()
        .context("missing snapshot version")?;
    ensure!(valid_version(version), "invalid snapshot version");
    let key = cache_key(id, Some(version))?;
    let body = serde_json::to_string(&value)?;
    ensure!(body.len() as u64 <= LIMIT, "collection response too large");
    validate(&body, id)?;
    store.set_setting(&key, &body)
}

pub fn fetch(store: &SkillStore, id: Option<&str>, version: Option<&str>) -> Result<Value> {
    fetch_from(store, BASE, id, version)
}

fn fetch_from(
    store: &SkillStore,
    base: &str,
    id: Option<&str>,
    version: Option<&str>,
) -> Result<Value> {
    let file = filename(id)?;
    cache_key(id, version)?;
    if id.is_some() {
        if let Ok(value) = cached(store, id, version) {
            return Ok(value);
        }
    }
    let proxy = get_github_proxy_url(store)?;
    let client = app_http_client(&proxy, Some(15))?;
    let read = |path: &str| -> Result<String> {
        let limit = if path == "current.json" {
            256 * 1024
        } else {
            LIMIT
        };
        let response = client
            .get(format!("{base}/{path}"))
            .header("User-Agent", "skills-hub")
            .send()?
            .error_for_status()?;
        let mut body = String::new();
        response.take(limit + 1).read_to_string(&mut body)?;
        ensure!(body.len() as u64 <= limit, "collection response too large");
        Ok(body)
    };
    let version = if id.is_some() {
        version.context("missing snapshot version")?.to_string()
    } else {
        let pointer: Value = serde_json::from_str(&read("current.json")?)?;
        pointer_version(&pointer)?.to_string()
    };
    if id.is_none() {
        if let Ok(value) = cached(store, None, Some(&version)) {
            return Ok(value);
        }
    }
    let mut value = validate(&read(&format!("snapshots/{version}/{file}"))?, id)?;
    value["snapshotVersion"] = Value::String(version);
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_paths_preview_and_invalid_pointers() {
        assert!(filename(Some("../index")).is_err());
        assert!(cache_key(Some("00000000-0000-4000-8000-000000000001"), Some("../bad")).is_err());
        let version = "a".repeat(64);
        let valid = json!({"schemaVersion":1,"version":version,"preview":false,"snapshotPath":format!("snapshots/{version}/")});
        assert!(pointer_version(&valid).is_ok());
        for (key, value) in [
            ("preview", json!(true)),
            ("snapshotPath", json!("https://outside.test/")),
            ("version", json!("../bad")),
            ("schemaVersion", json!(2)),
        ] {
            let mut invalid = valid.clone();
            invalid[key] = value;
            assert!(pointer_version(&invalid).is_err());
        }
        assert!(validate(
            r#"{"schemaVersion":1,"id":"x","status":"active","review":{"status":"pending"}}"#,
            Some("x")
        )
        .is_err());
    }

    #[test]
    fn reads_published_index_and_pins_details_and_cache_to_its_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkillStore::new(dir.path().join("test.db"));
        store.ensure_schema().unwrap();
        let mut server = mockito::Server::new();
        let version = "a".repeat(64);
        let id = "00000000-0000-4000-8000-000000000001";
        let pointer = server.mock("GET", "/current.json").with_body(json!({"schemaVersion":1,"version":version,"preview":false,"snapshotPath":format!("snapshots/{version}/")}).to_string()).expect(1).create();
        let index = server
            .mock("GET", format!("/snapshots/{version}/index.json").as_str())
            .with_body(r#"{"schemaVersion":1,"collections":[]}"#)
            .create();
        let detail = server
            .mock("GET", format!("/snapshots/{version}/{id}.json").as_str())
            .with_body(
                json!({"schemaVersion":1,"id":id,"status":"active","review":{"status":"approved"}})
                    .to_string(),
            )
            .create();
        let value = fetch_from(&store, &server.url(), None, None).unwrap();
        assert_eq!(value["snapshotVersion"], version);
        cache_validated(&store, None, value).unwrap();
        let value = fetch_from(&store, &server.url(), Some(id), Some(&version)).unwrap();
        cache_validated(&store, Some(id), value).unwrap();
        pointer.assert();
        index.assert();
        detail.assert();
        assert!(cached(&store, None, None).is_ok());
        assert!(cached(&store, Some(id), Some(&version)).is_ok());
        assert!(cached(&store, Some(id), Some(&"b".repeat(64))).is_err());
    }

    #[test]
    fn accepts_catalogs_larger_than_the_legacy_four_megabyte_limit() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkillStore::new(dir.path().join("test.db"));
        store.ensure_schema().unwrap();
        let value = json!({"schemaVersion":1,"snapshotVersion":"a".repeat(64),"collections":[],"searchText":"x".repeat(5 * 1024 * 1024)});
        cache_validated(&store, None, value).unwrap();
        assert!(cached(&store, None, None).is_ok());
    }

    #[test]
    fn unchanged_publication_reuses_validated_index_and_details() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkillStore::new(dir.path().join("test.db"));
        store.ensure_schema().unwrap();
        let mut server = mockito::Server::new();
        let version = "a".repeat(64);
        let id = "00000000-0000-4000-8000-000000000001";
        cache_validated(
            &store,
            None,
            json!({"schemaVersion":1,"snapshotVersion":version,"collections":[]}),
        )
        .unwrap();
        cache_validated(&store, Some(id), json!({"schemaVersion":1,"snapshotVersion":version,"id":id,"status":"active","review":{"status":"approved"}})).unwrap();
        let pointer = server.mock("GET", "/current.json").with_body(json!({"schemaVersion":1,"version":version,"preview":false,"snapshotPath":format!("snapshots/{version}/")}).to_string()).expect(1).create();
        let index = server
            .mock("GET", format!("/snapshots/{version}/index.json").as_str())
            .expect(0)
            .create();
        let detail = server
            .mock("GET", format!("/snapshots/{version}/{id}.json").as_str())
            .expect(0)
            .create();
        assert!(fetch_from(&store, &server.url(), None, None).is_ok());
        assert!(fetch_from(&store, &server.url(), Some(id), Some(&version)).is_ok());
        pointer.assert();
        index.assert();
        detail.assert();
        pointer.remove();
        let next = "b".repeat(64);
        let updated = server.mock("GET", "/current.json").with_body(json!({"schemaVersion":1,"version":next,"preview":false,"snapshotPath":format!("snapshots/{next}/")}).to_string()).create();
        let changed = server
            .mock("GET", format!("/snapshots/{next}/index.json").as_str())
            .with_body(r#"{"schemaVersion":1,"collections":[]}"#)
            .create();
        assert_eq!(
            fetch_from(&store, &server.url(), None, None).unwrap()["snapshotVersion"],
            next
        );
        updated.assert();
        changed.assert();
    }

    #[test]
    fn does_not_reuse_the_old_four_collection_cache() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkillStore::new(dir.path().join("test.db"));
        store.ensure_schema().unwrap();
        store
            .set_setting(
                "website_collections_v1_index.json",
                r#"{"schemaVersion":1,"collections":[]}"#,
            )
            .unwrap();
        assert!(cached(&store, None, None).is_err());
    }
}
