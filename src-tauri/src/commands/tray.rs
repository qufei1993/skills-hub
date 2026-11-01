use crate::core::skill_store::SkillStore;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn set_tray_language(
    app: AppHandle,
    store: State<'_, SkillStore>,
    language: String,
) -> Result<(), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::window_lifecycle::set_language(&app, &store, &language)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| {
        log::warn!("tray language update failed: {error:#}");
        "TRAY_LANGUAGE_FAILED".into()
    })
}
