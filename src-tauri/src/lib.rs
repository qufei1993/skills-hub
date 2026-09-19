mod commands;
mod core;

use std::sync::Arc;

use core::cancel_token::CancelToken;
use core::skill_store::{default_db_path, migrate_legacy_db_if_needed, SkillStore};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind};

fn runtime_context() -> tauri::Context<tauri::Wry> {
    let context = tauri::generate_context!();
    #[cfg(debug_assertions)]
    let context = {
        let mut context: tauri::Context<tauri::Wry> = context;
        if !context.config().identifier.ends_with(".dev") {
            context.config_mut().identifier.push_str(".dev");
        }
        context
    };
    context
}

const TRAY_ID: &str = "skills-hub-tray";
const TRAY_MENU_SHOW: &str = "tray-show";
const TRAY_MENU_QUIT: &str = "tray-quit";

/// The tray menu is native window-manager text, so it cannot come from the web i18n
/// bundle. The frontend pushes the active interface language once it has loaded.
fn tray_labels(language: &str) -> (&'static str, &'static str) {
    match language {
        "zh" => ("显示主窗口", "退出 Skills Hub"),
        "ko" => ("Skills Hub 열기", "Skills Hub 종료"),
        _ => ("Show Skills Hub", "Quit Skills Hub"),
    }
}

fn build_tray_menu<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    language: &str,
) -> tauri::Result<Menu<R>> {
    let (show, quit) = tray_labels(language);
    let show_item = MenuItem::with_id(app, TRAY_MENU_SHOW, show, true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, TRAY_MENU_QUIT, quit, true, None::<&str>)?;
    Menu::with_items(app, &[&show_item, &quit_item])
}

fn show_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// The tray must exist before the close button can hide the window, otherwise the app
/// would become unreachable. Callers therefore treat a missing tray as "not hidden".
fn setup_tray<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    let menu = build_tray_menu(app, "en")?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Skills Hub")
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_MENU_SHOW => show_main_window(app),
            TRAY_MENU_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

/// Re-labels the tray menu for the interface language the web app is currently using.
pub(crate) fn apply_tray_language<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    language: &str,
) -> Result<(), String> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    let menu = build_tray_menu(app, language).map_err(|err| err.to_string())?;
    tray.set_menu(Some(menu)).map_err(|err| err.to_string())
}

fn init_store<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> anyhow::Result<SkillStore> {
    let db_path = default_db_path(app)?;
    migrate_legacy_db_if_needed(&db_path)?;
    let store = SkillStore::new(db_path);
    store.ensure_schema()?;
    store.migrate_device_sync_startup_credential_consent()?;
    Ok(store)
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

            let sync_workspace = app.handle().path().app_data_dir()?.join("device-sync");
            let sync_central = core::central_repo::resolve_central_repo_path(app.handle(), &store)?;
            let sync_credentials = core::device_sync::credentials::SystemCredentialStore;
            match core::device_sync::DeviceSyncService::new(
                &store,
                &sync_credentials,
                sync_workspace,
                sync_central,
            )
            .repair_legacy_same_device_conflicts()
            {
                Ok(true) => {
                    log::info!("recovered same-device sync baseline from repository history")
                }
                Ok(false) => {}
                Err(err) => log::warn!("same-device sync baseline recovery skipped: {err:#}"),
            }

            if let Ok(Some(config)) = store.get_device_sync_config() {
                if config.auto_check
                    && !config.needs_visibility_confirmation()
                    && !(config.auto_sync && config.auto_sync_schedule.is_some())
                {
                    let handle = app.handle().clone();
                    let store_for_device_sync = store.clone();
                    tauri::async_runtime::spawn(async move {
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
                            service.check().map(|_| ())
                        })
                        .await;
                        if let Err(err) = result.and_then(|inner| inner.map_err(Into::into)) {
                            log::warn!("automatic device sync check failed: {err:#}");
                        }
                    });
                }
            }

            core::device_sync::scheduler::start(app.handle().clone(), store.clone());

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

            let recycle_handle = app.handle().clone();
            let recycle_store = store.clone();
            std::thread::spawn(move || loop {
                let root = match recycle_handle.path().app_data_dir() {
                    Ok(path) => path.join("recycle-bin"),
                    Err(error) => {
                        log::warn!("resolve recycle bin directory failed: {error:#}");
                        std::thread::sleep(std::time::Duration::from_secs(24 * 60 * 60));
                        continue;
                    }
                };
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                match core::recycle_bin::RecycleBinService::new(&recycle_store, root)
                    .cleanup_expired(now)
                {
                    Ok(removed) if removed > 0 => {
                        log::info!("cleaned up {removed} expired recycle bin items");
                    }
                    Ok(_) => {}
                    Err(error) => log::warn!("recycle bin cleanup failed: {error:#}"),
                }
                std::thread::sleep(std::time::Duration::from_secs(24 * 60 * 60));
            });

            if let Err(error) = setup_tray(app.handle()) {
                // Without a tray the window must not hide on close, or the app would be
                // unreachable. `on_window_event` already falls back to quitting.
                log::warn!("tray icon setup failed: {error:#}");
            }

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
            commands::get_auto_update_runtime,
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
            commands::get_recycle_bin_items,
            commands::get_recycle_bin_locations,
            commands::restore_recycle_bin_item,
            commands::delete_recycle_bin_item,
            commands::clear_recycle_bin,
            commands::resolve_device_sync_conflict,
            commands::restore_device_sync_trash,
            commands::disconnect_device_sync,
            commands::cancel_current_operation,
            commands::set_tray_language
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Closing keeps the app running in the notification area so scheduled
                // updates and device sync stay alive. Quit from the tray menu instead.
                if window.app_handle().tray_by_id(TRAY_ID).is_some() {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    window.app_handle().exit(0);
                }
            }
        })
        .build(runtime_context())
        .expect("error while running tauri application")
        .run(|_app, _event| {});
}

#[cfg(test)]
mod environment_tests {
    #[test]
    fn development_data_is_separate_from_packaged_data() {
        let packaged: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let identifier = packaged["identifier"].as_str().unwrap();
        let runtime = super::runtime_context();
        if cfg!(debug_assertions) {
            assert_ne!(runtime.config().identifier, identifier);
            assert_eq!(runtime.config().identifier, format!("{identifier}.dev"));
        } else {
            assert_eq!(runtime.config().identifier, identifier);
        }
    }
}
