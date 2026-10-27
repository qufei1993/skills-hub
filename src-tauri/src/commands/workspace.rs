use crate::services::skills_hub::SkillsHubService;
use crate::services::workspace::{ApplicationRecord, ApplicationRequest, Preset, UndoItem};
use tauri::State;

async fn run<T: Send + 'static>(
    service: &SkillsHubService,
    work: impl FnOnce(SkillsHubService) -> Result<T, crate::services::error::ServiceError>
        + Send
        + 'static,
) -> Result<T, String> {
    let service = service.clone();
    tauri::async_runtime::spawn_blocking(move || work(service))
        .await
        .map_err(|e| e.to_string())?
        .map_err(super::format_service_error)
}
#[tauri::command]
pub async fn workspace_presets(
    service: State<'_, SkillsHubService>,
) -> Result<Vec<Preset>, String> {
    run(&service, |s| s.list_presets()).await
}
#[tauri::command]
pub async fn workspace_save_preset(
    service: State<'_, SkillsHubService>,
    preset: Preset,
) -> Result<Preset, String> {
    run(&service, move |s| s.save_preset(preset)).await
}
#[tauri::command]
pub async fn workspace_delete_preset(
    service: State<'_, SkillsHubService>,
    id: String,
    revision: u64,
) -> Result<(), String> {
    run(&service, move |s| s.delete_preset(&id, revision)).await
}
#[tauri::command]
pub async fn workspace_preview(
    service: State<'_, SkillsHubService>,
    request: ApplicationRequest,
) -> Result<ApplicationRecord, String> {
    run(&service, move |s| s.preview_application(request)).await
}
#[tauri::command]
pub async fn workspace_execute(
    service: State<'_, SkillsHubService>,
    id: String,
) -> Result<ApplicationRecord, String> {
    run(&service, move |s| s.execute_application(&id)).await
}
#[tauri::command]
pub async fn workspace_discard(
    service: State<'_, SkillsHubService>,
    id: String,
) -> Result<(), String> {
    run(&service, move |s| s.discard_application(&id)).await
}
#[tauri::command]
pub async fn workspace_history(
    service: State<'_, SkillsHubService>,
) -> Result<Vec<ApplicationRecord>, String> {
    run(&service, |s| s.application_history()).await
}
#[tauri::command]
pub async fn workspace_preview_undo(
    service: State<'_, SkillsHubService>,
    id: String,
) -> Result<Vec<UndoItem>, String> {
    run(&service, move |s| s.preview_application_undo(&id)).await
}
#[tauri::command]
pub async fn workspace_undo(
    service: State<'_, SkillsHubService>,
    id: String,
) -> Result<Vec<UndoItem>, String> {
    run(&service, move |s| s.undo_application(&id)).await
}

#[tauri::command]
pub async fn workspace_recheck(
    service: State<'_, SkillsHubService>,
    id: String,
) -> Result<ApplicationRecord, String> {
    run(&service, move |s| s.recheck_application(&id)).await
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn workspace_content_preview(
    service: State<'_, SkillsHubService>,
    skillId: String,
    restore: bool,
) -> Result<crate::core::content_recovery::ContentPreview, String> {
    run(&service, move |s| s.preview_content(&skillId, restore)).await
}
#[tauri::command]
pub async fn workspace_content_execute(
    service: State<'_, SkillsHubService>,
    id: String,
) -> Result<serde_json::Value, String> {
    run(&service, move |s| s.execute_content(&id)).await
}
#[tauri::command]
pub async fn workspace_recoveries(
    service: State<'_, SkillsHubService>,
) -> Result<Vec<String>, String> {
    run(&service, move |s| s.recoverable_skills()).await
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn workspace_source_preview(
    service: State<'_, SkillsHubService>,
    skillId: String,
    kind: String,
    reference: String,
    subpath: Option<String>,
) -> Result<crate::core::content_recovery::SourceRepairPlan, String> {
    run(&service, move |s| {
        s.preview_source_repair(&skillId, &kind, &reference, subpath)
    })
    .await
}
#[tauri::command]
pub async fn workspace_source_execute(
    service: State<'_, SkillsHubService>,
    id: String,
) -> Result<(), String> {
    run(&service, move |s| s.apply_source_repair(&id)).await
}
