mod commands;
mod core;

use std::sync::Arc;

use core::cancel_token::CancelToken;
use core::central_repo::migrate_legacy_central_repo_if_needed;
use core::skill_store::{default_db_path, migrate_legacy_db_if_needed, SkillStore};
use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
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

            let db_path = default_db_path(app.handle()).map_err(tauri::Error::from)?;
            migrate_legacy_db_if_needed(&db_path).map_err(tauri::Error::from)?;
            let store = SkillStore::new(db_path);
            store.ensure_schema().map_err(tauri::Error::from)?;
            migrate_legacy_central_repo_if_needed(&store).map_err(tauri::Error::from)?;
            app.manage(store.clone());
            app.manage(Arc::new(CancelToken::new()));

            // Backfill description for skills that were installed before V2 schema.
            core::installer::backfill_skill_descriptions(&store);

            // Best-effort cleanup of our own old git temp directories.
            // Safety:
            // - Only deletes directories that match prefix `skills-syncer-git-*` (or legacy `skills-hub-git-*`)
            // - And contain our marker file `.skills-syncer-git-temp` (or legacy `.skills-hub-git-temp`)
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
            commands::set_central_repo_path,
            commands::get_recent_projects,
            commands::save_recent_project,
            commands::get_tool_status,
            commands::get_git_cache_cleanup_days,
            commands::get_git_cache_ttl_secs,
            commands::set_git_cache_cleanup_days,
            commands::set_git_cache_ttl_secs,
            commands::clear_git_cache_now,
            commands::get_onboarding_plan,
            commands::install_local,
            commands::list_local_skills_cmd,
            commands::install_local_selection,
            commands::install_git,
            commands::list_git_skills_cmd,
            commands::install_git_selection,
            commands::sync_skill_dir,
            commands::sync_skill_to_tool,
            commands::unsync_skill_from_tool,
            commands::update_managed_skill,
            commands::search_github,
            commands::get_github_token,
            commands::set_github_token,
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
