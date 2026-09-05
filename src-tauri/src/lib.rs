mod commands;
mod core;

use std::sync::Arc;

use core::cancel_token::CancelToken;
use core::device_sync::credentials::{CredentialStore, SystemCredentialStore};
use core::skill_store::{default_db_path, migrate_legacy_db_if_needed, SkillStore};
use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind};

fn init_store<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> anyhow::Result<SkillStore> {
    let db_path = default_db_path(app)?;
    migrate_legacy_db_if_needed(&db_path)?;
    let store = SkillStore::new(db_path);
    store.ensure_schema()?;
    Ok(store)
}

fn retry_device_sync_credential_cleanup_at_startup(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
) -> anyhow::Result<()> {
    commands::retry_queued_credential_cleanup(store, credentials)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(debug_assertions)]
    let _ = dotenvy::dotenv();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .targets([
                        Target::new(TargetKind::LogDir { file_name: None }),
                        #[cfg(desktop)]
                        Target::new(TargetKind::Stdout),
                    ])
                    .build(),
            )?;

            let is_background_update = std::env::args()
                .collect::<Vec<_>>()
                .windows(2)
                .any(|pair| pair[0] == "--background-task" && pair[1] == "update-skills");
            let force_background_update = std::env::args().any(|arg| arg == "--force");

            let store = init_store(app.handle()).map_err(tauri::Error::from)?;
            let credentials = SystemCredentialStore;
            if let Err(err) = retry_device_sync_credential_cleanup_at_startup(&store, &credentials)
            {
                log::warn!("deferred device sync credential cleanup remains queued: {err:#}");
            }

            if is_background_update {
                #[cfg(target_os = "macos")]
                {
                    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                }
                let run_result = if force_background_update {
                    core::auto_update::run_auto_update_now(app.handle(), &store).map(Some)
                } else {
                    core::auto_update::run_due_auto_update(app.handle(), &store)
                };
                match run_result {
                    Ok(Some(result)) => {
                        log::info!(
                            "auto update finished: checked={}, updated={}, failed={}",
                            result.checked,
                            result.updated,
                            result.failed
                        );
                        app.handle().exit(if result.failed == 0 { 0 } else { 2 });
                    }
                    Ok(None) => {
                        app.handle().exit(0);
                    }
                    Err(err) => {
                        eprintln!("auto update failed: {err:#}");
                        app.handle().exit(1);
                    }
                }
                return Ok(());
            }

            app.manage(store.clone());
            app.manage(Arc::new(CancelToken::new()));

            if let Ok(Some(config)) = store.get_device_sync_config() {
                if config.auto_check || config.auto_sync {
                    let handle = app.handle().clone();
                    let store_for_device_sync = store.clone();
                    tauri::async_runtime::spawn(async move {
                        let auto_sync = config.auto_sync;
                        let result = tauri::async_runtime::spawn_blocking(move || {
                            let workspace = handle.path().app_data_dir()?.join("device-sync");
                            let central = core::central_repo::resolve_central_repo_path(
                                &handle,
                                &store_for_device_sync,
                            )?;
                            let credentials = core::device_sync::credentials::SystemCredentialStore;
                            let service = core::device_sync::DeviceSyncService::new(
                                &store_for_device_sync,
                                &credentials,
                                workspace,
                                central,
                            );
                            if auto_sync {
                                service.sync().map(|_| ())
                            } else {
                                service.check().map(|_| ())
                            }
                        })
                        .await;
                        if let Err(err) = result.and_then(|inner| inner.map_err(Into::into)) {
                            log::warn!("automatic device sync check failed: {err:#}");
                        }
                    });
                }
            }

            // Backfill description for skills that were installed before V2 schema.
            core::installer::backfill_skill_descriptions(&store);

            // Best-effort cleanup of our own old git temp directories.
            // Safety:
            // - Only deletes directories that match prefix `skills-hub-git-*`
            // - And contain our marker file `.skills-hub-git-temp`
            // - And are older than the max age.
            let handle = app.handle().clone();
            let store_for_cleanup = store.clone();
            tauri::async_runtime::spawn(async move {
                let removed = core::temp_cleanup::cleanup_old_git_temp_dirs(
                    &handle,
                    std::time::Duration::from_secs(24 * 60 * 60),
                )
                .unwrap_or(0);
                if removed > 0 {
                    log::info!("cleaned up {} old git temp dirs", removed);
                }

                let cleanup_days =
                    core::cache_cleanup::get_git_cache_cleanup_days(&store_for_cleanup);
                if cleanup_days > 0 {
                    let max_age =
                        std::time::Duration::from_secs(cleanup_days as u64 * 24 * 60 * 60);
                    let removed =
                        core::cache_cleanup::cleanup_git_cache_dirs(&handle, max_age).unwrap_or(0);
                    if removed > 0 {
                        log::info!("cleaned up {} git cache dirs", removed);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_central_repo_path,
            commands::preview_central_repo_path_change,
            commands::set_central_repo_path,
            commands::get_recent_projects,
            commands::save_recent_project,
            commands::get_tool_config,
            commands::set_tool_config,
            commands::get_tool_status,
            commands::get_git_cache_cleanup_days,
            commands::get_git_cache_ttl_secs,
            commands::set_git_cache_cleanup_days,
            commands::set_git_cache_ttl_secs,
            commands::clear_git_cache_now,
            commands::get_auto_update_config,
            commands::set_auto_update_config,
            commands::run_auto_update_now,
            commands::trigger_auto_update_task_now_cmd,
            commands::get_onboarding_plan,
            commands::get_discovery_scan_settings,
            commands::set_discovery_scan_config,
            commands::install_local,
            commands::list_local_skills_cmd,
            commands::install_local_selection,
            commands::install_git,
            commands::list_git_skills_cmd,
            commands::install_git_selection,
            commands::sync_skill_dir,
            commands::sync_skill_to_tool,
            commands::unsync_skill_from_tool,
            commands::set_skill_enabled,
            commands::update_managed_skill,
            commands::search_github,
            commands::get_github_release_notes,
            commands::get_github_token_status,
            commands::set_github_token,
            commands::get_github_proxy_config,
            commands::set_github_proxy_config,
            commands::get_github_proxy_url,
            commands::set_github_proxy_url,
            commands::import_existing_skill,
            commands::get_managed_skills,
            commands::get_tags,
            commands::create_tag,
            commands::rename_tag,
            commands::delete_tag,
            commands::get_skill_tags,
            commands::set_skill_tags,
            commands::get_untagged_skill_ids,
            commands::delete_managed_skill,
            commands::get_featured_skills,
            commands::search_skills_online,
            commands::list_skill_files,
            commands::read_skill_file,
            commands::get_device_sync_config,
            commands::save_device_sync_config,
            commands::get_device_sync_oauth_availability,
            commands::get_device_sync_pending_oauth,
            commands::start_device_sync_oauth,
            commands::poll_device_sync_oauth,
            commands::cancel_device_sync_oauth,
            commands::clear_device_sync_pending_oauth,
            commands::validate_device_sync_account,
            commands::create_device_sync_repository,
            commands::list_device_sync_repositories,
            commands::get_device_sync_status,
            commands::check_device_sync,
            commands::run_device_sync,
            commands::get_device_sync_history,
            commands::get_device_sync_devices,
            commands::set_device_sync_device_alias,
            commands::get_device_sync_conflicts,
            commands::get_device_sync_trash,
            commands::resolve_device_sync_conflict,
            commands::restore_device_sync_trash,
            commands::disconnect_device_sync,
            commands::cancel_current_operation
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                window.app_handle().exit(0);
            }
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app, _event| {});
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex;

    use anyhow::{bail, Result};
    use tempfile::TempDir;

    use super::retry_device_sync_credential_cleanup_at_startup;
    use crate::core::device_sync::credentials::CredentialStore;
    use crate::core::device_sync::types::DeviceSyncConfig;
    use crate::core::skill_store::SkillStore;

    const CLEANUP_QUEUE_SETTING: &str = "device_sync_credential_cleanup_queue_v1";

    struct NoTouchCredentialStore {
        calls: AtomicUsize,
    }

    impl CredentialStore for NoTouchCredentialStore {
        fn set(&self, _key: &str, _value: &str) -> Result<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            bail!("credential store must not be touched")
        }

        fn get(&self, _key: &str) -> Result<Option<String>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            bail!("credential store must not be touched")
        }

        fn delete(&self, _key: &str) -> Result<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            bail!("credential store must not be touched")
        }
    }

    struct FailOnceCredentialStore {
        secrets: Mutex<HashMap<String, String>>,
        fail_next_delete: AtomicBool,
    }

    impl FailOnceCredentialStore {
        fn new(entries: [(&str, &str); 2]) -> Self {
            Self {
                secrets: Mutex::new(
                    entries
                        .into_iter()
                        .map(|(key, value)| (key.to_string(), value.to_string()))
                        .collect(),
                ),
                fail_next_delete: AtomicBool::new(true),
            }
        }

        fn contains(&self, key: &str) -> bool {
            self.secrets.lock().unwrap().contains_key(key)
        }
    }

    impl CredentialStore for FailOnceCredentialStore {
        fn set(&self, key: &str, value: &str) -> Result<()> {
            self.secrets
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }

        fn get(&self, key: &str) -> Result<Option<String>> {
            Ok(self.secrets.lock().unwrap().get(key).cloned())
        }

        fn delete(&self, key: &str) -> Result<()> {
            if self.fail_next_delete.swap(false, Ordering::SeqCst) {
                bail!("injected credential deletion failure")
            }
            self.secrets.lock().unwrap().remove(key);
            Ok(())
        }
    }

    fn test_store() -> (TempDir, SkillStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SkillStore::new(dir.path().join("skills.db"));
        store.ensure_schema().unwrap();
        (dir, store)
    }

    #[test]
    fn startup_cleanup_with_empty_queue_does_not_touch_credentials() {
        let (_dir, store) = test_store();
        let credentials = NoTouchCredentialStore {
            calls: AtomicUsize::new(0),
        };

        retry_device_sync_credential_cleanup_at_startup(&store, &credentials).unwrap();

        assert_eq!(credentials.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn startup_cleanup_retries_failures_without_deleting_an_active_key() {
        let (_dir, store) = test_store();
        let credentials = FailOnceCredentialStore::new([
            ("active-key", "active-secret"),
            ("orphan-key", "orphan-secret"),
        ]);
        store
            .save_device_sync_config(&DeviceSyncConfig {
                credential_key: Some("active-key".to_string()),
                ..DeviceSyncConfig::default()
            })
            .unwrap();
        store
            .set_setting(CLEANUP_QUEUE_SETTING, r#"["active-key","orphan-key"]"#)
            .unwrap();

        let first = retry_device_sync_credential_cleanup_at_startup(&store, &credentials);
        assert!(first.is_err());
        assert!(credentials.contains("active-key"));
        assert!(credentials.contains("orphan-key"));
        let queued: Vec<String> =
            serde_json::from_str(&store.get_setting(CLEANUP_QUEUE_SETTING).unwrap().unwrap())
                .unwrap();
        assert_eq!(queued, vec!["active-key", "orphan-key"]);

        retry_device_sync_credential_cleanup_at_startup(&store, &credentials).unwrap();
        assert!(credentials.contains("active-key"));
        assert!(!credentials.contains("orphan-key"));
        let queued: Vec<String> =
            serde_json::from_str(&store.get_setting(CLEANUP_QUEUE_SETTING).unwrap().unwrap())
                .unwrap();
        assert_eq!(queued, vec!["active-key"]);

        store.clear_device_sync_config().unwrap();
        retry_device_sync_credential_cleanup_at_startup(&store, &credentials).unwrap();
        assert!(!credentials.contains("active-key"));
        assert_eq!(store.get_setting(CLEANUP_QUEUE_SETTING).unwrap(), None);
    }
}
