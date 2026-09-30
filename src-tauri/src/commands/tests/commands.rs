use super::*;
use crate::core::device_sync::credentials::{
    save_oauth_credential, save_personal_access_token, MemoryCredentialStore, OAuthCredential,
    DEVICE_SYNC_KEYRING_SERVICE,
};
use crate::core::device_sync::types::OAuthPollStatus;
use crate::core::github_token::{
    resolve_github_token, set_github_token as set_github_token_core, GITHUB_TOKEN_CREDENTIAL_KEY,
    GITHUB_TOKEN_KEYRING_SERVICE,
};
use crate::core::skill_store::SkillRecord;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

#[test]
fn shared_directory_scope_error_reaches_desktop_as_a_distinct_recovery_hint() {
    let error = crate::services::error::ServiceError::new(
        crate::services::error::ErrorCode::TargetConflict,
        "shared directory affects tools outside the selected scope",
        serde_json::json!({"reason":"shared_directory_scope_expansion", "path":"/tools/shared"}),
    );
    assert_eq!(
        format_service_error(error),
        "SHARED_DIRECTORY_SCOPE_EXPANSION"
    );
}

#[test]
fn ai_management_does_not_install_when_bundled_cli_is_unavailable() {
    use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
    let home = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join(".codex")).unwrap();
    let service = SkillsHubService::open(RuntimePaths::from_roots(
        RuntimeProfile::Test,
        home.path(),
        data.path(),
    ))
    .unwrap();
    assert_eq!(
        enable_ai_management_impl(&service, &|| Err("CLI_UNAVAILABLE".into())).unwrap_err(),
        "CLI_UNAVAILABLE"
    );
    assert!(service.list_skills().unwrap().is_empty());
}

#[test]
fn agent_access_status_does_not_install_or_repair_the_cli() {
    use crate::core::cli_bridge::{bundled_cli_bridge_status, CliBridgeHealth, BINARY_NAME};
    let directory = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let bridge = directory.path().join("missing");
    assert_eq!(
        bundled_cli_bridge_status(&bridge).status,
        CliBridgeHealth::Missing
    );
    assert!(!bridge.exists());
    std::fs::create_dir(&bridge).unwrap();
    let binary = bridge.join(BINARY_NAME);
    std::fs::write(&binary, b"existing user installation").unwrap();
    assert_eq!(
        bundled_cli_bridge_status(&bridge).status,
        CliBridgeHealth::Damaged
    );
    assert_eq!(
        std::fs::read(&binary).unwrap(),
        b"existing user installation"
    );
    assert_eq!(std::fs::read_dir(&bridge).unwrap().count(), 1);
}

#[test]
fn agent_access_commands_require_explicit_single_agent_actions_and_removal_confirmation() {
    use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
    let home = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join(".codex")).unwrap();
    std::fs::create_dir_all(home.path().join(".cursor")).unwrap();
    let service = SkillsHubService::open(RuntimePaths::from_roots(
        RuntimeProfile::Test,
        home.path(),
        data.path(),
    ))
    .unwrap();
    assert!(set_agent_access_impl(&service, "codex".into(), "execute".into(), false).is_err());
    assert!(!service.agent_access_status().unwrap().installed);
    let installed =
        set_agent_access_impl(&service, "codex".into(), "install".into(), false).unwrap();
    assert_eq!(installed.skill.unwrap().targets.len(), 1);
    assert!(!home
        .path()
        .join(".cursor/skills/manage-skills-hub")
        .exists());
    assert!(set_agent_access_impl(&service, "codex".into(), "remove".into(), false).is_err());
    assert!(home.path().join(".codex/skills/manage-skills-hub").exists());
    set_agent_access_impl(&service, "codex".into(), "repair".into(), false).unwrap();
    let removed = set_agent_access_impl(&service, "codex".into(), "remove".into(), true).unwrap();
    assert!(removed.installed);
    assert!(!removed.deployed);
}

fn make_store() -> (tempfile::TempDir, SkillStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = SkillStore::new(dir.path().join("test.db"));
    store.ensure_schema().expect("ensure_schema");
    (dir, store)
}

#[test]
fn device_sync_network_operations_share_the_saved_application_proxy() {
    let (_dir, store) = make_store();
    set_github_proxy_config_core(&store, true, 7897).unwrap();

    assert_eq!(
        oauth_proxy_url(&store, ProviderId::Github).unwrap(),
        "http://127.0.0.1:7897"
    );
    assert_eq!(
        oauth_proxy_url(&store, ProviderId::Gitlab).unwrap(),
        "http://127.0.0.1:7897"
    );
    assert_eq!(
        oauth_proxy_url(&store, ProviderId::Gitee).unwrap(),
        "http://127.0.0.1:7897"
    );
}

#[test]
fn manual_device_sync_token_does_not_require_saved_authorization() {
    let (_dir, store) = make_store();

    let token = resolve_device_sync_token(
        &store,
        ProviderId::Gitee,
        Some("  temporary-gitee-token  ".to_string()),
        None,
    )
    .unwrap();

    assert_eq!(token, "temporary-gitee-token");
}

fn sqlite_sidecar_path(db_path: &Path, suffix: &str) -> PathBuf {
    let mut path = db_path.as_os_str().to_os_string();
    path.push(suffix);
    PathBuf::from(path)
}

fn sqlite_visible_files_contain(db_path: &Path, needle: &[u8]) -> bool {
    [
        db_path.to_path_buf(),
        sqlite_sidecar_path(db_path, "-wal"),
        sqlite_sidecar_path(db_path, "-shm"),
    ]
    .into_iter()
    .filter_map(|path| std::fs::read(path).ok())
    .any(|bytes| bytes.windows(needle.len()).any(|window| window == needle))
}

#[derive(Default)]
struct FailingGithubTokenCredentialStore;

impl CredentialStore for FailingGithubTokenCredentialStore {
    fn set(&self, _key: &str, _secret: &str) -> anyhow::Result<()> {
        anyhow::bail!("credential write failed")
    }

    fn get(&self, _key: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    fn delete(&self, _key: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct InvalidExistingGithubTokenCredentialStore;

impl CredentialStore for InvalidExistingGithubTokenCredentialStore {
    fn set(&self, _key: &str, _secret: &str) -> anyhow::Result<()> {
        anyhow::bail!("credential repair write failed")
    }

    fn get(&self, _key: &str) -> anyhow::Result<Option<String>> {
        Ok(Some("invalid-keychain-envelope".to_string()))
    }

    fn delete(&self, _key: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct FailingGithubTokenReadStore {
    set_calls: AtomicUsize,
}

impl CredentialStore for FailingGithubTokenReadStore {
    fn set(&self, _key: &str, _secret: &str) -> anyhow::Result<()> {
        self.set_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn get(&self, _key: &str) -> anyhow::Result<Option<String>> {
        anyhow::bail!("credential read temporarily unavailable")
    }

    fn delete(&self, _key: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct ConcurrentGithubTokenCredentialStore {
    active_calls: AtomicUsize,
    overlap_detected: AtomicBool,
    secret: Mutex<Option<String>>,
}

impl ConcurrentGithubTokenCredentialStore {
    fn enter(&self) {
        if self.active_calls.fetch_add(1, Ordering::SeqCst) > 0 {
            self.overlap_detected.store(true, Ordering::SeqCst);
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    fn leave(&self) {
        self.active_calls.fetch_sub(1, Ordering::SeqCst);
    }
}

impl CredentialStore for ConcurrentGithubTokenCredentialStore {
    fn set(&self, _key: &str, secret: &str) -> anyhow::Result<()> {
        self.enter();
        *self.secret.lock().unwrap() = Some(secret.to_string());
        self.leave();
        Ok(())
    }

    fn get(&self, _key: &str) -> anyhow::Result<Option<String>> {
        self.enter();
        let value = self.secret.lock().unwrap().clone();
        self.leave();
        Ok(value)
    }

    fn delete(&self, _key: &str) -> anyhow::Result<()> {
        self.enter();
        *self.secret.lock().unwrap() = None;
        self.leave();
        Ok(())
    }
}

#[derive(Default)]
struct FailOnceDeleteCredentialStore {
    secrets: Mutex<std::collections::HashMap<String, String>>,
    failed_once: AtomicBool,
}

impl CredentialStore for FailOnceDeleteCredentialStore {
    fn set(&self, key: &str, secret: &str) -> anyhow::Result<()> {
        self.secrets
            .lock()
            .unwrap()
            .insert(key.to_string(), secret.to_string());
        Ok(())
    }

    fn get(&self, key: &str) -> anyhow::Result<Option<String>> {
        Ok(self.secrets.lock().unwrap().get(key).cloned())
    }

    fn delete(&self, key: &str) -> anyhow::Result<()> {
        if !self.failed_once.swap(true, Ordering::SeqCst) {
            anyhow::bail!("injected credential delete failure");
        }
        self.secrets.lock().unwrap().remove(key);
        Ok(())
    }
}

#[test]
fn github_search_token_uses_a_keyring_service_isolated_from_device_sync() {
    assert_ne!(GITHUB_TOKEN_KEYRING_SERVICE, DEVICE_SYNC_KEYRING_SERVICE);
    assert!(GITHUB_TOKEN_KEYRING_SERVICE.ends_with(".dev"));
    assert!(DEVICE_SYNC_KEYRING_SERVICE.ends_with(".dev"));
}

#[test]
fn github_token_status_does_not_read_the_system_credential_store() {
    let (_dir, store) = make_store();
    let credentials = FailingGithubTokenReadStore::default();

    let status = get_github_token_status_impl(&store, &credentials).unwrap();

    assert_eq!(status, GithubTokenStatusDto { has_token: false });
}

#[test]
fn github_search_token_rejects_an_oauth_credential_envelope() {
    let (_dir, store) = make_store();
    let credentials = MemoryCredentialStore::default();
    save_oauth_credential(
        &credentials,
        GITHUB_TOKEN_CREDENTIAL_KEY,
        ProviderId::Github,
        &OAuthCredential {
            access_token: "oauth-token-not-a-pat".to_string(),
            refresh_token: None,
            expires_at: None,
            token_url: "https://github.com/login/oauth/access_token".to_string(),
            client_id: "client".to_string(),
        },
    )
    .unwrap();

    let error = resolve_github_token(&store, &credentials)
        .unwrap_err()
        .to_string();

    assert!(error.contains("personal access token"), "{error}");
}

#[test]
fn legacy_github_token_status_defers_migration_until_the_token_is_used() {
    let (_dir, store) = make_store();
    let credentials = MemoryCredentialStore::default();
    store
        .set_setting("github_token", "legacy-github-secret")
        .unwrap();

    let status = get_github_token_status_impl(&store, &credentials).unwrap();

    assert_eq!(status, GithubTokenStatusDto { has_token: true });
    assert_eq!(
        store.get_setting("github_token").unwrap().as_deref(),
        Some("legacy-github-secret")
    );
    assert_eq!(credentials.get(GITHUB_TOKEN_CREDENTIAL_KEY).unwrap(), None);

    assert_eq!(
        resolve_github_token(&store, &credentials)
            .unwrap()
            .as_deref(),
        Some("legacy-github-secret")
    );
    assert_eq!(store.get_setting("github_token").unwrap(), None);
    assert!(credentials
        .get(GITHUB_TOKEN_CREDENTIAL_KEY)
        .unwrap()
        .is_some());
}

#[test]
fn legacy_github_token_migration_erases_secret_from_database_and_wal_files() {
    const SECRET: &str = "github_pat_UNIQUE_PHYSICAL_ERASURE_TEST_6f93c5a1";
    let (_dir, store) = make_store();
    let keeper = Connection::open(store.db_path()).unwrap();
    let journal_mode: String = keeper
        .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
        .unwrap();
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
    keeper
        .execute_batch("PRAGMA wal_autocheckpoint=0;")
        .unwrap();
    store.set_setting("github_token", SECRET).unwrap();
    assert!(sqlite_visible_files_contain(
        store.db_path(),
        SECRET.as_bytes()
    ));

    let token = resolve_github_token(&store, &MemoryCredentialStore::default()).unwrap();

    assert!(token.is_some());
    assert_eq!(store.get_setting("github_token").unwrap(), None);
    assert!(!sqlite_visible_files_contain(
        store.db_path(),
        SECRET.as_bytes()
    ));
    drop(keeper);
}

#[test]
fn legacy_github_token_cleanup_retries_after_a_busy_wal_checkpoint() {
    const SECRET: &str = "github_pat_UNIQUE_BUSY_WAL_RETRY_TEST_92d81ef4";
    const CLEANUP_MARKER: &str = "github_token_secure_cleanup_pending";
    let (_dir, store) = make_store();
    let reader = Connection::open(store.db_path()).unwrap();
    let journal_mode: String = reader
        .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
        .unwrap();
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
    reader
        .execute_batch("PRAGMA wal_autocheckpoint=0;")
        .unwrap();
    store.set_setting("github_token", SECRET).unwrap();
    reader.execute_batch("BEGIN;").unwrap();
    assert_eq!(
        reader
            .query_row(
                "SELECT value FROM settings WHERE key = 'github_token'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        SECRET
    );

    let credentials = MemoryCredentialStore::default();
    let error = resolve_github_token(&store, &credentials)
        .unwrap_err()
        .to_string();

    assert!(error.contains("WAL"), "{error}");
    assert_eq!(store.get_setting("github_token").unwrap(), None);
    assert_eq!(
        store.get_setting(CLEANUP_MARKER).unwrap().as_deref(),
        Some("1"),
        "a durable marker must make the physical cleanup retryable"
    );
    assert!(sqlite_visible_files_contain(
        store.db_path(),
        SECRET.as_bytes()
    ));

    reader.execute_batch("ROLLBACK;").unwrap();
    let token = resolve_github_token(&store, &credentials).unwrap();

    assert!(token.is_some());
    assert_eq!(store.get_setting(CLEANUP_MARKER).unwrap(), None);
    assert!(!sqlite_visible_files_contain(
        store.db_path(),
        SECRET.as_bytes()
    ));
}

#[test]
fn failed_legacy_github_token_migration_keeps_the_sqlite_secret() {
    let (_dir, store) = make_store();
    let credentials = FailingGithubTokenCredentialStore;
    store
        .set_setting("github_token", "legacy-github-secret")
        .unwrap();

    let result = resolve_github_token(&store, &credentials);

    assert!(result.is_err());
    assert_eq!(
        store.get_setting("github_token").unwrap().as_deref(),
        Some("legacy-github-secret")
    );
}

#[test]
fn legacy_github_token_migration_fails_closed_when_keychain_read_fails() {
    let (_dir, store) = make_store();
    let credentials = FailingGithubTokenReadStore::default();
    store
        .set_setting("github_token", "legacy-github-secret")
        .unwrap();

    let result = resolve_github_token(&store, &credentials);

    assert!(result.is_err());
    assert_eq!(credentials.set_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        store.get_setting("github_token").unwrap().as_deref(),
        Some("legacy-github-secret")
    );
}

#[test]
fn github_token_status_serialization_never_discloses_the_secret() {
    let (_dir, store) = make_store();
    let credentials = MemoryCredentialStore::default();
    store
        .set_setting("github_token", "never-return-this-secret")
        .unwrap();

    let status = get_github_token_status_impl(&store, &credentials).unwrap();
    let json = serde_json::to_string(&status).unwrap();

    assert_eq!(json, r#"{"has_token":true}"#);
    assert!(!json.contains("never-return-this-secret"));
}

#[test]
fn recovery_after_sqlite_cleanup_failure_preserves_the_new_keychain_token() {
    let (_dir, store) = make_store();
    let credentials = MemoryCredentialStore::default();
    save_personal_access_token(
        &credentials,
        GITHUB_TOKEN_CREDENTIAL_KEY,
        &CredentialUsage::official(ProviderId::Github),
        "new-keychain-token",
    )
    .unwrap();
    store
        .set_setting("github_token", "stale-sqlite-token")
        .unwrap();

    assert_eq!(
        resolve_github_token(&store, &credentials)
            .unwrap()
            .as_deref(),
        Some("new-keychain-token")
    );
    assert_eq!(store.get_setting("github_token").unwrap(), None);
}

#[test]
fn invalid_keychain_envelope_is_repaired_from_the_legacy_sqlite_token() {
    let (_dir, store) = make_store();
    let credentials = MemoryCredentialStore::default();
    credentials
        .set(GITHUB_TOKEN_CREDENTIAL_KEY, "invalid-keychain-envelope")
        .unwrap();
    store
        .set_setting("github_token", "recoverable-legacy-token")
        .unwrap();

    assert_eq!(
        resolve_github_token(&store, &credentials)
            .unwrap()
            .as_deref(),
        Some("recoverable-legacy-token")
    );
    assert_eq!(store.get_setting("github_token").unwrap(), None);
}

#[test]
fn failed_invalid_keychain_repair_preserves_the_legacy_sqlite_token() {
    let (_dir, store) = make_store();
    let credentials = InvalidExistingGithubTokenCredentialStore;
    store
        .set_setting("github_token", "recoverable-legacy-token")
        .unwrap();

    let result = resolve_github_token(&store, &credentials);

    assert!(result.is_err());
    assert_eq!(
        store.get_setting("github_token").unwrap().as_deref(),
        Some("recoverable-legacy-token")
    );
}

#[test]
fn saving_github_token_replaces_keychain_value_and_clears_legacy_sqlite() {
    let (_dir, store) = make_store();
    let credentials = MemoryCredentialStore::default();
    store
        .set_setting("github_token", "stale-sqlite-token")
        .unwrap();

    set_github_token_core(&store, &credentials, "  replacement-token  ").unwrap();

    assert_eq!(
        resolve_github_token(&store, &credentials)
            .unwrap()
            .as_deref(),
        Some("replacement-token")
    );
    assert_eq!(store.get_setting("github_token").unwrap(), None);
}

#[test]
fn deleting_github_token_removes_keychain_and_legacy_sqlite_values() {
    let (_dir, store) = make_store();
    let credentials = MemoryCredentialStore::default();
    set_github_token_core(&store, &credentials, "saved-token").unwrap();
    store
        .set_setting("github_token", "stale-sqlite-token")
        .unwrap();

    set_github_token_core(&store, &credentials, "").unwrap();

    assert_eq!(resolve_github_token(&store, &credentials).unwrap(), None);
    assert_eq!(store.get_setting("github_token").unwrap(), None);
}

#[test]
fn github_token_migration_save_and_delete_are_serialized() {
    let (_dir, store) = make_store();
    let credentials = ConcurrentGithubTokenCredentialStore::default();
    store
        .set_setting("github_token", "legacy-sqlite-token")
        .unwrap();

    let results = std::thread::scope(|scope| {
        let mut handles =
            vec![scope.spawn(|| resolve_github_token(&store, &credentials).map(|_| ()))];
        for index in 0..6 {
            let store = &store;
            let credentials = &credentials;
            handles.push(scope.spawn(move || {
                let token = if index % 2 == 0 {
                    ""
                } else {
                    "replacement-token"
                };
                set_github_token_core(store, credentials, token)
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });

    for result in results {
        result.unwrap();
    }
    assert!(!credentials.overlap_detected.load(Ordering::SeqCst));
}

#[test]
fn pending_device_sync_authorization_survives_reload_until_cleared() {
    let (_dir, store) = make_store();
    let pending = PendingOAuthAuthorization {
        provider: ProviderId::Github,
        credential_key: "credential-key".to_string(),
        account: ProviderAccount {
            login: "may".to_string(),
            display_name: Some("May".to_string()),
        },
    };
    save_pending_oauth(&store, &pending).unwrap();
    assert_eq!(load_pending_oauth(&store).unwrap(), Some(pending));
    clear_pending_oauth(&store, false).unwrap();
    assert_eq!(load_pending_oauth(&store).unwrap(), None);
}

#[test]
fn clearing_pending_oauth_preserves_a_credential_owned_by_active_config() {
    let (_dir, store) = make_store();
    let credentials = MemoryCredentialStore::default();
    credentials.set("shared-key", "shared-secret").unwrap();
    store
        .save_device_sync_config(&DeviceSyncConfig {
            credential_key: Some("shared-key".to_string()),
            ..DeviceSyncConfig::default()
        })
        .unwrap();
    save_pending_oauth(
        &store,
        &PendingOAuthAuthorization {
            provider: ProviderId::Github,
            credential_key: "shared-key".to_string(),
            account: ProviderAccount {
                login: "may".to_string(),
                display_name: None,
            },
        },
    )
    .unwrap();

    clear_pending_oauth_with_credentials(&store, &credentials, true).unwrap();

    assert_eq!(load_pending_oauth(&store).unwrap(), None);
    assert_eq!(
        credentials.get("shared-key").unwrap().as_deref(),
        Some("shared-secret")
    );
}

#[test]
fn failed_pending_credential_delete_is_queued_and_retried() {
    const CLEANUP_QUEUE: &str = "device_sync_credential_cleanup_queue_v1";
    let (_dir, store) = make_store();
    let credentials = FailOnceDeleteCredentialStore::default();
    credentials.set("pending-key", "pending-secret").unwrap();
    save_pending_oauth(
        &store,
        &PendingOAuthAuthorization {
            provider: ProviderId::Github,
            credential_key: "pending-key".to_string(),
            account: ProviderAccount {
                login: "may".to_string(),
                display_name: None,
            },
        },
    )
    .unwrap();

    let error = format!(
        "{:#}",
        clear_pending_oauth_with_credentials(&store, &credentials, true).unwrap_err()
    );

    assert!(
        error.contains("injected credential delete failure"),
        "{error}"
    );
    assert_eq!(load_pending_oauth(&store).unwrap(), None);
    assert_eq!(
        serde_json::from_str::<Vec<String>>(
            store
                .get_setting(CLEANUP_QUEUE)
                .unwrap()
                .as_deref()
                .expect("failed cleanup must stay durable"),
        )
        .unwrap(),
        vec!["pending-key"]
    );
    assert_eq!(
        credentials.get("pending-key").unwrap().as_deref(),
        Some("pending-secret")
    );

    clear_pending_oauth_with_credentials(&store, &credentials, true).unwrap();

    assert_eq!(credentials.get("pending-key").unwrap(), None);
    assert_eq!(store.get_setting(CLEANUP_QUEUE).unwrap(), None);
}

#[test]
fn queued_credential_cleanup_never_deletes_the_current_active_key() {
    const CLEANUP_QUEUE: &str = "device_sync_credential_cleanup_queue_v1";
    let (_dir, store) = make_store();
    let credentials = MemoryCredentialStore::default();
    credentials.set("active-key", "active-secret").unwrap();
    store
        .save_device_sync_config(&DeviceSyncConfig {
            credential_key: Some("active-key".to_string()),
            ..DeviceSyncConfig::default()
        })
        .unwrap();
    enqueue_credential_cleanup(&store, "active-key").unwrap();

    retry_queued_credential_cleanup(&store, &credentials).unwrap();

    assert_eq!(
        credentials.get("active-key").unwrap().as_deref(),
        Some("active-secret")
    );
    assert_eq!(
        serde_json::from_str::<Vec<String>>(
            store
                .get_setting(CLEANUP_QUEUE)
                .unwrap()
                .as_deref()
                .unwrap(),
        )
        .unwrap(),
        vec!["active-key"]
    );

    store.clear_device_sync_config().unwrap();
    retry_queued_credential_cleanup(&store, &credentials).unwrap();

    assert_eq!(credentials.get("active-key").unwrap(), None);
    assert_eq!(store.get_setting(CLEANUP_QUEUE).unwrap(), None);
}

#[test]
fn config_replacement_records_cleanup_intent_before_changing_the_active_pointer() {
    const CLEANUP_QUEUE: &str = "device_sync_credential_cleanup_queue_v1";
    let (_dir, store) = make_store();
    let credentials = FailOnceDeleteCredentialStore::default();
    credentials.set("old-active", "old-secret").unwrap();
    store
        .save_device_sync_config(&DeviceSyncConfig {
            credential_key: Some("old-active".to_string()),
            ..DeviceSyncConfig::default()
        })
        .unwrap();
    let replacement = DeviceSyncConfig {
        credential_key: Some("new-active".to_string()),
        ..DeviceSyncConfig::default()
    };

    let error = persist_device_sync_credential_replacement_with(
        &store,
        &credentials,
        Some("old-active"),
        || {
            assert_eq!(
                serde_json::from_str::<Vec<String>>(
                    store
                        .get_setting(CLEANUP_QUEUE)?
                        .as_deref()
                        .expect("cleanup intent must precede the DB pointer change"),
                )?,
                vec!["old-active"]
            );
            store.save_device_sync_config(&replacement)
        },
    )
    .unwrap_err();

    assert!(format!("{error:#}").contains("injected credential delete failure"));
    assert_eq!(
        store
            .get_device_sync_config()
            .unwrap()
            .unwrap()
            .credential_key
            .as_deref(),
        Some("new-active")
    );
    assert!(credentials.get("old-active").unwrap().is_some());
    assert!(store.get_setting(CLEANUP_QUEUE).unwrap().is_some());

    persist_device_sync_credential_replacement_with(&store, &credentials, None, || Ok(())).unwrap();

    assert_eq!(credentials.get("old-active").unwrap(), None);
    assert_eq!(store.get_setting(CLEANUP_QUEUE).unwrap(), None);
}

#[test]
fn disconnect_delete_failure_keeps_a_retryable_intent_after_config_is_cleared() {
    const CLEANUP_QUEUE: &str = "device_sync_credential_cleanup_queue_v1";
    let (_dir, store) = make_store();
    let credentials = FailOnceDeleteCredentialStore::default();
    credentials.set("active-key", "active-secret").unwrap();
    store
        .save_device_sync_config(&DeviceSyncConfig {
            credential_key: Some("active-key".to_string()),
            ..DeviceSyncConfig::default()
        })
        .unwrap();

    let error = disconnect_device_sync_with_credentials(&store, &credentials).unwrap_err();

    assert!(format!("{error:#}").contains("injected credential delete failure"));
    assert_eq!(store.get_device_sync_config().unwrap(), None);
    assert!(credentials.get("active-key").unwrap().is_some());
    assert_eq!(
        serde_json::from_str::<Vec<String>>(
            store
                .get_setting(CLEANUP_QUEUE)
                .unwrap()
                .as_deref()
                .unwrap(),
        )
        .unwrap(),
        vec!["active-key"]
    );

    disconnect_device_sync_with_credentials(&store, &credentials).unwrap();

    assert_eq!(credentials.get("active-key").unwrap(), None);
    assert_eq!(store.get_setting(CLEANUP_QUEUE).unwrap(), None);
}

#[test]
fn replacing_pending_oauth_preserves_the_old_key_when_active_config_owns_it() {
    let (_dir, store) = make_store();
    let credentials = MemoryCredentialStore::default();
    credentials.set("active-key", "active-secret").unwrap();
    credentials.set("new-key", "new-secret").unwrap();
    store
        .save_device_sync_config(&DeviceSyncConfig {
            credential_key: Some("active-key".to_string()),
            ..DeviceSyncConfig::default()
        })
        .unwrap();
    save_pending_oauth(
        &store,
        &PendingOAuthAuthorization {
            provider: ProviderId::Github,
            credential_key: "active-key".to_string(),
            account: ProviderAccount {
                login: "old".to_string(),
                display_name: None,
            },
        },
    )
    .unwrap();
    let result = OAuthPollResult {
        provider: ProviderId::Github,
        status: OAuthPollStatus::Authorized,
        interval_seconds: 3,
        credential_key: Some("new-key".to_string()),
        account: Some(ProviderAccount {
            login: "new".to_string(),
            display_name: None,
        }),
    };

    persist_pending_oauth_result_with(&store, &credentials, &result, |pending| {
        save_pending_oauth(&store, pending)
    })
    .unwrap();

    assert!(credentials.get("active-key").unwrap().is_some());
    assert_eq!(
        load_pending_oauth(&store).unwrap().unwrap().credential_key,
        "new-key"
    );
}

#[test]
fn failed_replaced_pending_delete_is_queued_and_next_oauth_operation_retries_it() {
    const CLEANUP_QUEUE: &str = "device_sync_credential_cleanup_queue_v1";
    let (_dir, store) = make_store();
    let credentials = FailOnceDeleteCredentialStore::default();
    credentials.set("old-key", "old-secret").unwrap();
    credentials.set("new-key", "new-secret").unwrap();
    save_pending_oauth(
        &store,
        &PendingOAuthAuthorization {
            provider: ProviderId::Github,
            credential_key: "old-key".to_string(),
            account: ProviderAccount {
                login: "old".to_string(),
                display_name: None,
            },
        },
    )
    .unwrap();
    let result = OAuthPollResult {
        provider: ProviderId::Github,
        status: OAuthPollStatus::Authorized,
        interval_seconds: 3,
        credential_key: Some("new-key".to_string()),
        account: Some(ProviderAccount {
            login: "new".to_string(),
            display_name: None,
        }),
    };

    let error = format!(
        "{:#}",
        persist_pending_oauth_result_with(&store, &credentials, &result, |pending| {
            save_pending_oauth(&store, pending)
        })
        .unwrap_err()
    );

    assert!(
        error.contains("injected credential delete failure"),
        "{error}"
    );
    assert_eq!(
        load_pending_oauth(&store).unwrap().unwrap().credential_key,
        "new-key"
    );
    assert_eq!(
        serde_json::from_str::<Vec<String>>(
            store
                .get_setting(CLEANUP_QUEUE)
                .unwrap()
                .as_deref()
                .expect("failed replacement cleanup must stay durable"),
        )
        .unwrap(),
        vec!["old-key"]
    );
    assert!(credentials.get("old-key").unwrap().is_some());

    persist_pending_oauth_result_with(&store, &credentials, &result, |pending| {
        save_pending_oauth(&store, pending)
    })
    .unwrap();

    assert_eq!(credentials.get("old-key").unwrap(), None);
    assert!(credentials.get("new-key").unwrap().is_some());
    assert_eq!(store.get_setting(CLEANUP_QUEUE).unwrap(), None);
}

#[test]
fn oauth_poll_and_pending_persistence_share_the_device_sync_lock() {
    let (_dir, store) = make_store();
    let credentials = MemoryCredentialStore::default();
    let result = OAuthPollResult {
        provider: ProviderId::Github,
        status: OAuthPollStatus::Authorized,
        interval_seconds: 3,
        credential_key: Some("new-key".to_string()),
        account: Some(ProviderAccount {
            login: "may".to_string(),
            display_name: None,
        }),
    };

    let persisted = poll_device_sync_oauth_with(
        &store,
        &credentials,
        || {
            assert!(
                crate::core::device_sync::try_lock_device_sync().is_err(),
                "the device-sync lock must already cover OAuth polling and its keychain write"
            );
            credentials.set("new-key", "new-secret")?;
            Ok(result.clone())
        },
        |pending| {
            assert!(
                crate::core::device_sync::try_lock_device_sync().is_err(),
                "the same lock must remain held while the pending DB pointer is saved"
            );
            save_pending_oauth(&store, pending)
        },
    )
    .unwrap();

    assert_eq!(persisted.credential_key.as_deref(), Some("new-key"));
    assert_eq!(
        load_pending_oauth(&store).unwrap().unwrap().credential_key,
        "new-key"
    );
    drop(crate::core::device_sync::try_lock_device_sync().unwrap());
}

#[test]
fn oauth_pending_database_failure_deletes_the_new_credential() {
    let (_dir, store) = make_store();
    let credentials = MemoryCredentialStore::default();
    credentials.set("new-key", "new-secret").unwrap();
    let result = OAuthPollResult {
        provider: ProviderId::Github,
        status: OAuthPollStatus::Authorized,
        interval_seconds: 3,
        credential_key: Some("new-key".to_string()),
        account: Some(ProviderAccount {
            login: "may".to_string(),
            display_name: None,
        }),
    };

    let error = persist_pending_oauth_result_with(&store, &credentials, &result, |_| {
        anyhow::bail!("injected pending database failure")
    })
    .unwrap_err()
    .to_string();

    assert!(
        error.contains("injected pending database failure"),
        "{error}"
    );
    assert_eq!(credentials.get("new-key").unwrap(), None);
}

#[test]
fn oauth_pending_database_and_delete_failure_leaves_a_retryable_cleanup_intent() {
    const CLEANUP_QUEUE: &str = "device_sync_credential_cleanup_queue_v1";
    let (_dir, store) = make_store();
    let credentials = FailOnceDeleteCredentialStore::default();
    credentials.set("new-key", "new-secret").unwrap();
    let result = OAuthPollResult {
        provider: ProviderId::Github,
        status: OAuthPollStatus::Authorized,
        interval_seconds: 3,
        credential_key: Some("new-key".to_string()),
        account: Some(ProviderAccount {
            login: "may".to_string(),
            display_name: None,
        }),
    };

    let error = format!(
        "{:#}",
        persist_pending_oauth_result_with(&store, &credentials, &result, |_| {
            anyhow::bail!("injected pending database failure")
        })
        .unwrap_err()
    );

    assert!(
        error.contains("injected pending database failure"),
        "{error}"
    );
    assert!(
        error.contains("injected credential delete failure"),
        "{error}"
    );
    assert_eq!(
        serde_json::from_str::<Vec<String>>(
            store
                .get_setting(CLEANUP_QUEUE)
                .unwrap()
                .as_deref()
                .unwrap(),
        )
        .unwrap(),
        vec!["new-key"]
    );
    assert!(credentials.get("new-key").unwrap().is_some());

    clear_pending_oauth_with_credentials(&store, &credentials, false).unwrap();

    assert_eq!(credentials.get("new-key").unwrap(), None);
    assert_eq!(store.get_setting(CLEANUP_QUEUE).unwrap(), None);
}

#[test]
fn manual_pat_database_failure_deletes_staged_key_and_preserves_active_key() {
    let (_dir, store) = make_store();
    let credentials = MemoryCredentialStore::default();
    let usage = CredentialUsage::official(ProviderId::Github);
    save_personal_access_token(&credentials, "active-key", &usage, "active-token").unwrap();
    let staged_key = Mutex::new(None::<String>);
    let candidate = DeviceSyncConfig {
        provider: ProviderId::Github,
        remote_url: "https://github.com/acme/sync.git".to_string(),
        credential_key: Some("active-key".to_string()),
        ..DeviceSyncConfig::default()
    };

    let error = persist_config_with_staged_personal_access_token(
        &store,
        &credentials,
        &usage,
        "replacement-token",
        candidate,
        |staged| {
            let key = staged.credential_key.as_deref().unwrap();
            assert_ne!(key, "active-key");
            assert_eq!(
                resolve_access_token(&credentials, "active-key", &usage)
                    .unwrap()
                    .as_deref(),
                Some("active-token")
            );
            *staged_key.lock().unwrap() = Some(key.to_string());
            anyhow::bail!("injected config database failure")
        },
    )
    .unwrap_err()
    .to_string();

    assert!(
        error.contains("injected config database failure"),
        "{error}"
    );
    assert_eq!(
        resolve_access_token(&credentials, "active-key", &usage)
            .unwrap()
            .as_deref(),
        Some("active-token")
    );
    assert_eq!(
        credentials
            .get(staged_key.lock().unwrap().as_deref().unwrap())
            .unwrap(),
        None
    );
}

#[test]
fn failed_manual_pat_compensation_is_queued_without_touching_the_active_key() {
    const CLEANUP_QUEUE: &str = "device_sync_credential_cleanup_queue_v1";
    let (_dir, store) = make_store();
    let credentials = FailOnceDeleteCredentialStore::default();
    let usage = CredentialUsage::official(ProviderId::Github);
    save_personal_access_token(&credentials, "active-key", &usage, "active-token").unwrap();
    store
        .save_device_sync_config(&DeviceSyncConfig {
            credential_key: Some("active-key".to_string()),
            ..DeviceSyncConfig::default()
        })
        .unwrap();
    let staged_key = Mutex::new(None::<String>);
    let candidate = DeviceSyncConfig {
        provider: ProviderId::Github,
        remote_url: "https://github.com/acme/sync.git".to_string(),
        credential_key: Some("active-key".to_string()),
        ..DeviceSyncConfig::default()
    };

    let error = format!(
        "{:#}",
        persist_config_with_staged_personal_access_token(
            &store,
            &credentials,
            &usage,
            "replacement-token",
            candidate,
            |staged| {
                *staged_key.lock().unwrap() = staged.credential_key.clone();
                anyhow::bail!("injected config database failure")
            },
        )
        .unwrap_err()
    );
    let staged_key = staged_key.lock().unwrap().clone().unwrap();

    assert!(
        error.contains("injected config database failure"),
        "{error}"
    );
    assert_eq!(
        serde_json::from_str::<Vec<String>>(
            store
                .get_setting(CLEANUP_QUEUE)
                .unwrap()
                .as_deref()
                .expect("failed compensation must stay durable"),
        )
        .unwrap(),
        vec![staged_key.clone()]
    );
    assert!(credentials.get(&staged_key).unwrap().is_some());
    assert!(credentials.get("active-key").unwrap().is_some());

    retry_queued_credential_cleanup(&store, &credentials).unwrap();

    assert_eq!(credentials.get(&staged_key).unwrap(), None);
    assert!(credentials.get("active-key").unwrap().is_some());
    assert_eq!(store.get_setting(CLEANUP_QUEUE).unwrap(), None);
}

#[test]
fn changing_remote_host_does_not_inherit_the_previous_credential_key() {
    let previous = DeviceSyncConfig {
        provider: ProviderId::Github,
        remote_url: "https://github.com/example/first.git".to_string(),
        credential_key: Some("github-credential".to_string()),
        ..DeviceSyncConfig::default()
    };
    let changed_host = CredentialUsage::from_https_remote(
        ProviderId::Github,
        "https://attacker.example/example/second.git",
    )
    .unwrap();

    assert_eq!(
        inherited_device_sync_credential(Some(&previous), &changed_host),
        None
    );
}

#[test]
fn enabling_auto_sync_preserves_the_owned_credential_without_reading_it() {
    let (_dir, store) = make_store();
    let previous = DeviceSyncConfig {
        provider: ProviderId::Gitee,
        remote_url: "https://gitee.com/example/sync.git".to_string(),
        credential_key: Some("existing-credential".to_string()),
        ..DeviceSyncConfig::default()
    };
    store.save_device_sync_config(&previous).unwrap();
    let usage =
        CredentialUsage::from_https_remote(previous.provider, &previous.remote_url).unwrap();
    let mut saved = previous.clone();
    saved.auto_sync = true;
    saved.credential_key = inherited_device_sync_credential(Some(&previous), &usage);
    struct NoCredentialAccess;
    impl CredentialStore for NoCredentialAccess {
        fn get(&self, _: &str) -> anyhow::Result<Option<String>> {
            panic!("settings save must not read credentials")
        }
        fn set(&self, _: &str, _: &str) -> anyhow::Result<()> {
            panic!("settings save must not replace credentials")
        }
        fn delete(&self, _: &str) -> anyhow::Result<()> {
            panic!("settings save must not delete credentials")
        }
    }
    persist_device_sync_credential_replacement_with(&store, &NoCredentialAccess, None, || {
        store.save_device_sync_config(&saved)
    })
    .unwrap();
    let actual = store.get_device_sync_config().unwrap().unwrap();
    assert!(actual.auto_sync);
    assert_eq!(
        actual.credential_key.as_deref(),
        Some("existing-credential")
    );
}

#[test]
fn format_anyhow_error_passthrough_prefixes() {
    for message in [
        "MULTI_SKILLS|abc",
        "TARGET_EXISTS|/tmp/skill",
        "TOOL_NOT_INSTALLED|cursor",
        "TOOL_NOT_WRITABLE|Cursor|/tmp/skills",
        "TARGET_MODIFIED|/tmp/skills/demo",
        "UPDATE_IN_PROGRESS|/tmp/skills",
        "CENTRAL_MODIFIED|/tmp/skills/demo",
        r#"ROLLBACK_CONFLICT|{"target":"/tmp/a","recovery":"/tmp/b"}"#,
    ] {
        assert_eq!(format_anyhow_error(anyhow::anyhow!(message)), message);
    }
}

#[test]
fn format_anyhow_error_redacts_clone_temp_path() {
    let err = anyhow::anyhow!("clone https://example.com/a/b into /tmp/skills-hub-git-123");
    let msg = format_anyhow_error(err);
    assert!(msg.contains("已省略临时目录"));
    assert!(!msg.contains("/tmp/skills-hub-git-123"));
}

#[test]
fn format_anyhow_error_github_hint_auth() {
    let err = anyhow::anyhow!("git clone https://github.com/a/b failed: authentication failed");
    let msg = format_anyhow_error(err);
    assert!(msg.contains("无法访问该仓库"));
}

#[test]
fn format_service_error_serializes_all_multi_skill_candidates_for_desktop() {
    let error = crate::services::error::ServiceError::new(
        crate::services::error::ErrorCode::MultiSkills,
        "the source contains multiple skills",
        serde_json::json!({
            "candidates": [
                { "name": "中文技能", "description": "中文说明", "subpath": "skills/zh" },
                { "name": "한국어 스킬", "description": "한국어 설명", "subpath": "skills/ko" }
            ]
        }),
    );

    let formatted = format_service_error(error);
    let payload = formatted.strip_prefix("MULTI_SKILLS|").unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(payload).unwrap(),
        serde_json::json!([
            { "name": "中文技能", "description": "中文说明", "subpath": "skills/zh" },
            { "name": "한국어 스킬", "description": "한국어 설명", "subpath": "skills/ko" }
        ])
    );
}

#[test]
fn format_service_error_restores_safe_legacy_auth_and_network_hints() {
    let auth = crate::services::error::ServiceError::new(
        crate::services::error::ErrorCode::AuthRequired,
        "the remote source requires authentication",
        serde_json::json!({ "legacy_category": "github_auth" }),
    );
    let network = crate::services::error::ServiceError::new(
        crate::services::error::ErrorCode::NetworkError,
        "the remote source could not be reached",
        serde_json::json!({ "legacy_category": "github_network" }),
    );

    let auth_message = format_service_error(auth);
    let network_message = format_service_error(network);

    assert!(auth_message.contains("无法访问该仓库"));
    assert!(network_message.contains("请检查网络/代理"));
    assert!(!auth_message.contains("INTERNAL_ERROR"));
    assert!(!network_message.contains("INTERNAL_ERROR"));
}

#[test]
fn format_service_error_restores_specific_safe_git_failure_hints() {
    for (code, category, expected) in [
        (
            crate::services::error::ErrorCode::NetworkError,
            "github_tls",
            "TLS/证书校验失败",
        ),
        (
            crate::services::error::ErrorCode::InvalidSource,
            "github_not_found",
            "仓库不存在或无权限访问",
        ),
        (
            crate::services::error::ErrorCode::NetworkError,
            "github_dns",
            "无法解析 GitHub 域名",
        ),
        (
            crate::services::error::ErrorCode::NetworkError,
            "github_timeout",
            "连接 GitHub 超时",
        ),
        (
            crate::services::error::ErrorCode::NetworkError,
            "github_connection",
            "连接 GitHub 失败",
        ),
        (
            crate::services::error::ErrorCode::NetworkError,
            "github_rate_limited",
            "GitHub API 频率限制已触发",
        ),
    ] {
        let error = crate::services::error::ServiceError::new(
            code,
            "safe remote failure",
            serde_json::json!({ "legacy_category": category }),
        );
        assert!(format_service_error(error).contains(expected));
    }
}

#[test]
fn format_service_error_preserves_existing_prefixes_and_localized_messages() {
    for message in [
        "TARGET_EXISTS|/tmp/safe-target",
        "无法安装此技能，请检查来源。",
        "이 스킬을 설치할 수 없습니다. 소스를 확인하세요.",
    ] {
        let error = crate::services::error::ServiceError::new(
            crate::services::error::ErrorCode::InternalError,
            message,
            serde_json::json!({}),
        );
        assert_eq!(format_service_error(error), message);
    }
}

#[test]
fn format_service_error_maps_target_conflict_back_to_the_desktop_prefix() {
    let error = crate::services::error::ServiceError::new(
        crate::services::error::ErrorCode::TargetConflict,
        "the deployment target already contains unmanaged content",
        serde_json::json!({ "path": "/tmp/safe-target" }),
    );

    assert_eq!(
        format_service_error(error),
        "TARGET_EXISTS|/tmp/safe-target"
    );
}

#[test]
fn format_service_error_preserves_modified_target_semantics_for_safe_removal() {
    let error = crate::services::error::ServiceError::new(
        crate::services::error::ErrorCode::TargetConflict,
        "a managed target contains user changes",
        serde_json::json!({
            "path": "/tmp/safe-target",
            "reason": "target_modified"
        }),
    );

    assert_eq!(
        format_service_error(error),
        "TARGET_MODIFIED|/tmp/safe-target"
    );
}

fn make_install_service() -> (
    tempfile::TempDir,
    tempfile::TempDir,
    crate::core::runtime_paths::RuntimePaths,
    SkillsHubService,
) {
    let home = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let paths = crate::core::runtime_paths::RuntimePaths::from_roots(
        crate::core::runtime_paths::RuntimeProfile::Test,
        home.path(),
        data.path(),
    );
    let service = SkillsHubService::open(paths.clone()).unwrap();
    (home, data, paths, service)
}

fn write_install_test_skill(path: &Path, name: &str) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::write(
        path.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Test skill\n---\n\nBody\n"),
    )
    .unwrap();
}

fn init_install_test_git_repo(path: &Path) {
    let repo = git2::Repository::init(path).unwrap();
    let signature = git2::Signature::now("Skills Hub Test", "test@example.com").unwrap();
    let mut index = repo.index().unwrap();
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo.commit(Some("HEAD"), &signature, &signature, "initial", &tree, &[])
        .unwrap();
}

#[test]
fn duplicate_local_install_reaches_the_desktop_target_exists_protocol() {
    let (_home, _data, paths, service) = make_install_service();
    let source = paths.app_data_dir.join("private-local-source");
    write_install_test_skill(&source, "duplicate-local");
    service.install(InstallRequest::local(&source)).unwrap();

    let error = service.install(InstallRequest::local(&source)).unwrap_err();
    let formatted = format_service_error(error);

    assert_eq!(
        formatted,
        format!(
            "TARGET_EXISTS|{}",
            paths.default_central_repo.join("duplicate-local").display()
        )
    );
    assert!(!formatted.contains(source.to_string_lossy().as_ref()));
}

#[test]
fn git_reinstall_reuses_records_and_reports_other_source_conflicts_without_leaking_urls() {
    let (_home, _data, paths, service) = make_install_service();
    let secret = "do-not-leak-source-secret";
    let repo_path = paths.app_data_dir.join(secret);
    write_install_test_skill(&repo_path, "duplicate-git");
    init_install_test_git_repo(&repo_path);
    let source_url = format!("file://{}", repo_path.display());
    let first = service
        .install(InstallRequest::git(&source_url).with_name(Some("duplicate-git".to_string())))
        .unwrap();

    let repeated = service
        .install(InstallRequest::git(&source_url).with_name(Some("duplicate-git".to_string())))
        .unwrap();
    let dto = super::to_service_install_dto(repeated);
    assert_eq!(dto.skill_id, first.id);
    let serialized = serde_json::to_value(dto).unwrap();
    assert_eq!(serialized["action"], "unchanged");
    assert!(!serialized.to_string().contains(secret));

    let other_repo = paths.app_data_dir.join("other-private-source");
    write_install_test_skill(&other_repo, "duplicate-git");
    init_install_test_git_repo(&other_repo);
    let error = service
        .install(InstallRequest::git(format!(
            "file://{}",
            other_repo.display()
        )))
        .unwrap_err();
    let formatted = format_service_error(error);

    assert_eq!(
        formatted,
        format!(
            "TARGET_EXISTS|{}",
            paths.default_central_repo.join("duplicate-git").display()
        )
    );
    assert!(!formatted.contains(secret));
    assert!(!formatted.contains(&source_url));
    assert!(!formatted.contains("other-private-source"));
}

#[test]
fn expand_home_path_basic() {
    let home = dirs::home_dir().expect("home");
    assert_eq!(expand_home_path("~").unwrap(), home);
    assert_eq!(expand_home_path("~/abc").unwrap(), home.join("abc"));
}

#[test]
fn expand_home_path_empty_is_error() {
    let err = expand_home_path("  ").unwrap_err().to_string();
    assert!(err.contains("storage path is empty"));
}

#[test]
fn saving_custom_tool_config_creates_enabled_skills_dir() {
    let (dir, store) = make_store();
    let existing = dir.path().join("existing-skills");
    std::fs::create_dir_all(&existing).unwrap();
    let created = dir.path().join("created-skills");
    assert!(!created.exists());

    save_tool_config(
        &store,
        ToolConfig {
            disabled_builtin_tools: Vec::new(),
            custom_tools: vec![
                CustomToolConfig {
                    key: "custom_existing".to_string(),
                    label: "Existing".to_string(),
                    avatar: Some("data:image/png;base64,AA==".to_string()),
                    skills_dir: existing.to_string_lossy().to_string(),
                    project_skills_dir: Some(".custom/skills".to_string()),
                    sync_mode: SyncMode::Auto,
                    enabled: true,
                },
                CustomToolConfig {
                    key: "custom_created".to_string(),
                    label: "Created".to_string(),
                    avatar: None,
                    skills_dir: created.to_string_lossy().to_string(),
                    project_skills_dir: None,
                    sync_mode: SyncMode::Copy,
                    enabled: true,
                },
            ],
        },
    )
    .unwrap();
    assert!(created.is_dir());

    let tools = runtime_tools(&store, true).unwrap();
    let existing_tool = tools
        .iter()
        .find(|tool| tool.key == "custom_existing")
        .unwrap();
    let created_tool = tools
        .iter()
        .find(|tool| tool.key == "custom_created")
        .unwrap();

    assert!(existing_tool.enabled);
    assert!(existing_tool.installed);
    assert_eq!(
        existing_tool.avatar.as_deref(),
        Some("data:image/png;base64,AA==")
    );
    assert_eq!(existing_tool.sync_mode, SyncMode::Auto);
    assert_eq!(
        existing_tool.supports_project_scope,
        cfg!(any(target_os = "macos", target_os = "linux"))
    );
    for tool in tools.iter().filter(|tool| !tool.is_custom) {
        let expected = cfg!(any(target_os = "macos", target_os = "linux"))
            && !matches!(tool.key.as_str(), "hermes_agent" | "workbuddy");
        assert_eq!(tool.supports_project_scope, expected, "{}", tool.key);
    }
    assert!(created_tool.enabled);
    assert!(created_tool.installed);
    assert_eq!(created_tool.sync_mode, SyncMode::Copy);
}

#[test]
fn tool_status_adapter_keeps_detected_separate_from_enabled() {
    let dto = ToolInfoDto::from(ServiceAgent {
        key: "disabled-agent".to_string(),
        label: "Disabled Agent".to_string(),
        avatar: None,
        detected: true,
        enabled: false,
        is_custom: false,
        skills_dir: "/tmp/agent-skills".to_string(),
        project_skills_dir: ".agent/skills".to_string(),
        supports_project_scope: true,
        sync_mode: SyncMode::Auto,
    });

    assert!(dto.installed);
    assert!(!dto.enabled);
}

#[test]
fn desktop_tool_status_tracks_newly_detected_agents_after_service_read() {
    let (_dir, store) = make_store();
    store
        .set_setting("installed_tools_v1", r#"["existing"]"#)
        .unwrap();
    let status = desktop_tool_status(
        &store,
        crate::services::types::AgentList {
            agents: vec![ServiceAgent {
                key: "disabled-new".to_string(),
                label: "Disabled New".to_string(),
                avatar: None,
                detected: true,
                enabled: false,
                is_custom: false,
                skills_dir: "/tmp/agent-skills".to_string(),
                project_skills_dir: ".agent/skills".to_string(),
                supports_project_scope: true,
                sync_mode: SyncMode::Auto,
            }],
            installed: vec!["existing".to_string(), "disabled-new".to_string()],
        },
    );

    assert_eq!(status.newly_installed, vec!["disabled-new"]);
    assert_eq!(
        store.get_setting("installed_tools_v1").unwrap().as_deref(),
        Some(r#"["existing","disabled-new"]"#)
    );
}

#[test]
fn normalize_scope_defaults_to_global_and_rejects_unknown() {
    assert_eq!(normalize_scope(None).unwrap(), "global");
    assert_eq!(normalize_scope(Some("global")).unwrap(), "global");
    assert_eq!(normalize_scope(Some("project")).unwrap(), "project");
    assert!(normalize_scope(Some("workspace")).is_err());
}

#[test]
fn recent_projects_are_deduped_ordered_and_limited() {
    let (_dir, store) = make_store();
    let project_root = tempfile::tempdir().unwrap();
    let mut paths = Vec::new();
    for i in 0..9 {
        let path = project_root.path().join(format!("project-{i}"));
        std::fs::create_dir_all(&path).unwrap();
        paths.push(path);
    }

    for path in &paths {
        save_recent_project_impl(&store, path.to_string_lossy().as_ref()).unwrap();
    }

    let recent = get_recent_projects_impl(&store).unwrap();
    assert_eq!(recent.len(), 8);
    assert_eq!(recent[0], paths[8].to_string_lossy());
    assert_eq!(recent[7], paths[1].to_string_lossy());
    assert!(!recent.contains(&paths[0].to_string_lossy().to_string()));

    save_recent_project_impl(&store, paths[3].to_string_lossy().as_ref()).unwrap();
    let recent = get_recent_projects_impl(&store).unwrap();
    assert_eq!(recent.len(), 8);
    assert_eq!(recent[0], paths[3].to_string_lossy());
    assert_eq!(
        recent
            .iter()
            .filter(|item| *item == &paths[3].to_string_lossy())
            .count(),
        1
    );
}

#[test]
fn save_recent_project_rejects_missing_directory() {
    let (_dir, store) = make_store();
    let missing = tempfile::tempdir().unwrap().path().join("missing-project");
    let err = save_recent_project_impl(&store, missing.to_string_lossy().as_ref())
        .unwrap_err()
        .to_string();
    assert!(err.contains("projectPath must be an existing directory"));
}

#[test]
fn remove_path_any_handles_file_dir_and_missing() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("f.txt");
    std::fs::write(&file, b"1").unwrap();
    remove_path_any(file.to_string_lossy().as_ref()).unwrap();
    assert!(!file.exists());

    let sub = dir.path().join("d");
    std::fs::create_dir_all(&sub).unwrap();
    remove_path_any(sub.to_string_lossy().as_ref()).unwrap();
    assert!(!sub.exists());

    remove_path_any(dir.path().join("missing").to_string_lossy().as_ref()).unwrap();
}

#[test]
#[cfg(unix)]
fn remove_path_any_removes_symlink_only() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("real");
    std::fs::create_dir_all(&target).unwrap();
    let link = dir.path().join("link");
    symlink(&target, &link).unwrap();

    remove_path_any(link.to_string_lossy().as_ref()).unwrap();
    assert!(!link.exists());
    assert!(target.exists());
}

#[test]
#[cfg(windows)]
fn remove_path_any_removes_junction_only() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("real");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("keep.txt"), b"keep").unwrap();
    let link = dir.path().join("link");
    junction::create(&target, &link).unwrap();

    remove_path_any(link.to_string_lossy().as_ref()).unwrap();
    assert!(std::fs::symlink_metadata(&link).is_err());
    assert!(target.join("keep.txt").exists());
}

#[test]
fn get_managed_skills_impl_maps_targets() {
    let (dir, store) = make_store();
    let skill = SkillRecord {
        id: "s1".to_string(),
        name: "S1".to_string(),
        description: None,
        source_type: "local".to_string(),
        source_ref: Some(
            dir.path()
                .join("missing-source")
                .to_string_lossy()
                .to_string(),
        ),
        source_subpath: None,
        source_revision: None,
        central_path: "/tmp/central".to_string(),
        content_hash: None,
        created_at: 1,
        updated_at: 2,
        last_sync_at: None,
        last_seen_at: 1,
        enabled: true,
        status: "ok".to_string(),
    };
    store.upsert_skill(&skill).unwrap();

    let target = SkillTargetRecord {
        id: "t1".to_string(),
        skill_id: "s1".to_string(),
        tool: "cursor".to_string(),
        scope: "global".to_string(),
        project_path: None,
        target_path: "/tmp/target".to_string(),
        mode: "copy".to_string(),
        status: "error".to_string(),
        last_error: Some("permission denied".to_string()),
        synced_at: None,
    };
    store.upsert_skill_target(&target).unwrap();
    let tag = store.create_tag("Frontend").unwrap();
    store.set_skill_tags("s1", &[tag.id]).unwrap();

    let out = get_managed_skills_impl(&store).unwrap();
    assert_eq!(out.len(), 1);
    assert!(out[0].enabled);
    assert_eq!(out[0].tags.len(), 1);
    assert_eq!(out[0].tags[0].name, "Frontend");
    assert_eq!(out[0].targets.len(), 1);
    assert_eq!(out[0].targets[0].tool, "cursor");
    assert_eq!(out[0].targets[0].scope, "global");
    assert_eq!(out[0].targets[0].status, "error");
    assert_eq!(
        out[0].targets[0].last_error.as_deref(),
        Some("SKILL_ISSUE|permission")
    );
    assert!(out[0].targets[0].project_path.is_none());
    assert_eq!(out[0].status, "error");
}

#[test]
fn managed_skill_status_keeps_existing_local_sources_healthy() {
    let source = tempfile::tempdir().unwrap();
    let skill = SkillRecord {
        id: "s1".to_string(),
        name: "S1".to_string(),
        description: None,
        source_type: "local".to_string(),
        source_ref: Some(source.path().to_string_lossy().to_string()),
        source_subpath: None,
        source_revision: None,
        central_path: "/tmp/central".to_string(),
        content_hash: None,
        created_at: 1,
        updated_at: 1,
        last_sync_at: None,
        last_seen_at: 1,
        enabled: true,
        status: "ok".to_string(),
    };

    assert_eq!(managed_skill_status(&skill), "ok");
    let mut unbound = skill;
    unbound.source_ref = None;
    assert_eq!(managed_skill_status(&unbound), "ok");
    unbound.source_ref = Some("/definitely-missing-source".into());
    assert_eq!(managed_skill_status(&unbound), "error");
}

#[test]
fn deployment_errors_preserve_desktop_prefixes() {
    use crate::services::error::{ErrorCode, ServiceError};
    for (code, details, expected) in [
        (
            ErrorCode::AgentNotFound,
            serde_json::json!({"agent":"cursor"}),
            "TOOL_NOT_INSTALLED|cursor",
        ),
        (
            ErrorCode::ProjectScopeUnsupported,
            serde_json::json!({"agent":"workbuddy"}),
            "PROJECT_SCOPE_UNSUPPORTED|workbuddy",
        ),
        (
            ErrorCode::TargetConflict,
            serde_json::json!({"agent":"cursor","path":"/tools/demo","reason":"unmanaged_target"}),
            "TARGET_EXISTS|/tools/demo",
        ),
        (
            ErrorCode::PlanStale,
            serde_json::json!({}),
            "PLAN_STALE|deployment state changed",
        ),
    ] {
        assert_eq!(
            format_deployment_error(ServiceError::new(code, "domain error", details)),
            expected
        );
    }
}

#[test]
fn desktop_deployment_request_keeps_an_explicit_agent_and_project_scope() {
    let request = desktop_deployment_request(
        "id".into(),
        "cursor".into(),
        Some("project".into()),
        Some("/project".into()),
    )
    .unwrap();
    assert_eq!(
        request.skill,
        crate::services::types::SkillSelector::Id("id".into())
    );
    assert_eq!(request.agents, ["cursor"]);
    assert_eq!(
        request.scope,
        crate::services::deployment::DeploymentScope::Project("/project".into())
    );
    assert!(
        desktop_deployment_request("id".into(), "cursor".into(), Some("project".into()), None)
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn imported_local_skill_can_resync_its_existing_tool_link() {
    let (dir, store) = make_store();
    let central = dir.path().join("central");
    let target = dir.path().join("tool");
    std::fs::create_dir(&central).unwrap();
    std::fs::write(central.join("SKILL.md"), "# Test").unwrap();
    std::os::unix::fs::symlink(&central, &target).unwrap();
    let mut skill = SkillRecord {
        id: "imported".into(),
        name: "imported".into(),
        description: None,
        source_type: "local".into(),
        source_ref: Some(central.to_string_lossy().into()),
        source_subpath: None,
        source_revision: None,
        central_path: central.to_string_lossy().into(),
        content_hash: None,
        created_at: 1,
        updated_at: 1,
        last_sync_at: Some(1),
        last_seen_at: 1,
        enabled: true,
        status: "ok".into(),
    };
    store.upsert_skill(&skill).unwrap();
    ensure_target_does_not_overlap_local_source(&store, &skill.id, &target).unwrap();
    // An independent original source remains protected, even when reached through a link.
    let original = dir.path().join("original");
    std::fs::create_dir(&original).unwrap();
    std::fs::remove_file(&target).unwrap();
    std::os::unix::fs::symlink(&original, &target).unwrap();
    skill.source_ref = Some(original.to_string_lossy().into());
    store.upsert_skill(&skill).unwrap();
    assert!(ensure_target_does_not_overlap_local_source(&store, &skill.id, &target).is_err());
}

#[test]
fn ai_management_preflight_rejects_no_tools_before_preparing_cli() {
    use crate::core::runtime_paths::{RuntimePaths, RuntimeProfile};
    let home = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let service = SkillsHubService::open(RuntimePaths::from_roots(
        RuntimeProfile::Test,
        home.path(),
        data.path(),
    ))
    .unwrap();
    let calls = AtomicUsize::new(0);
    let result = enable_ai_management_impl(&service, &|| {
        calls.fetch_add(1, Ordering::SeqCst);
        Err("CLI_UNAVAILABLE".into())
    });
    assert_eq!(result.unwrap_err(), "AGENT_NOT_FOUND");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(service.list_skills().unwrap().is_empty());
}
