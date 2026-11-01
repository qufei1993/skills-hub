use crate::services::skills_hub::SkillsHubService;
use crate::services::source_conflict::{
    SourceConflictChoice, SourceConflictOutcome, SourceConflictPreview,
};
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
        .map_err(|_| "INTERNAL_ERROR".to_string())?
        .map_err(|error| serde_json::to_string(&error).unwrap_or_else(|_| "INTERNAL_ERROR".into()))
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn source_conflict_preview(
    service: State<'_, SkillsHubService>,
    sourcePath: String,
) -> Result<SourceConflictPreview, String> {
    run(&service, move |service| {
        service.preview_source_conflict(&sourcePath)
    })
    .await
}

#[tauri::command]
pub async fn source_conflict_execute(
    service: State<'_, SkillsHubService>,
    id: String,
    choice: SourceConflictChoice,
) -> Result<SourceConflictOutcome, String> {
    run(&service, move |service| {
        service.resolve_source_conflict(&id, choice)
    })
    .await
}
