//! Per-request cancellation for content discovery, independent of installation.
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tauri::{ipc::Channel, State};

use crate::core::cancel_token::CancelToken;
use crate::services::content_check::{ContentCheckProgress, ContentCheckRun};
use crate::services::skills_hub::SkillsHubService;

const RECENT_LIMIT: usize = 128;

#[derive(Default)]
struct RegistryState {
    active: Option<(String, Arc<CancelToken>)>,
    // A cancellation may reach the backend before its start invocation. Retain
    // it, and retired IDs, without allowing an unbounded request-ID history.
    recent: VecDeque<(String, bool)>,
}

#[derive(Clone, Default)]
pub struct ContentCheckRegistry(Arc<Mutex<RegistryState>>);

struct CheckGuard {
    registry: ContentCheckRegistry,
    check_id: String,
    token: Arc<CancelToken>,
}

impl ContentCheckRegistry {
    fn begin(&self, id: &str) -> Result<CheckGuard, String> {
        validate_id(id)?;
        let mut state = self
            .0
            .lock()
            .map_err(|_| "content check state unavailable")?;
        if state.active.is_some() {
            return Err("OPERATION_BUSY|another update check is running".into());
        }
        let token = Arc::new(CancelToken::new());
        if let Some(index) = state.recent.iter().position(|(key, _)| key == id) {
            let (_, cancelled_before_start) = state.recent.remove(index).unwrap();
            if !cancelled_before_start {
                return Err("INVALID_ARGUMENT|checkId has already completed".into());
            }
            token.cancel();
        }
        state.active = Some((id.into(), token.clone()));
        Ok(CheckGuard {
            registry: self.clone(),
            check_id: id.into(),
            token,
        })
    }

    fn cancel(&self, id: &str) -> Result<(), String> {
        validate_id(id)?;
        let mut state = self
            .0
            .lock()
            .map_err(|_| "content check state unavailable")?;
        if let Some((active_id, token)) = state.active.as_ref() {
            if active_id == id {
                token.cancel();
                return Ok(());
            }
        }
        if !state.recent.iter().any(|(key, _)| key == id) {
            remember(&mut state, id.into(), true);
        }
        Ok(())
    }
}

fn validate_id(id: &str) -> Result<(), String> {
    uuid::Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| "INVALID_ARGUMENT|checkId must be a UUID".into())
}

fn remember(state: &mut RegistryState, id: String, cancelled_before_start: bool) {
    state.recent.push_back((id, cancelled_before_start));
    while state.recent.len() > RECENT_LIMIT {
        state.recent.pop_front();
    }
}

impl Drop for CheckGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.registry.0.lock() {
            if state
                .active
                .as_ref()
                .is_some_and(|(id, _)| id == &self.check_id)
            {
                state.active = None;
                remember(&mut state, self.check_id.clone(), false);
            }
        }
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn workspace_content_check(
    service: State<'_, SkillsHubService>,
    registry: State<'_, ContentCheckRegistry>,
    checkId: String,
    skillIds: Vec<String>,
    onProgress: Channel<ContentCheckProgress>,
) -> Result<ContentCheckRun, String> {
    // Register before spawning: cancel never needs an operation/file lock and
    // cannot reset or cancel the token of an unrelated install/preview.
    let guard = registry.begin(&checkId)?;
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = service.check_content_sources(&checkId, &skillIds, &guard.token, |event| {
            // The final invoke result is authoritative if a channel is closed.
            let _ = onProgress.send(event);
        });
        drop(guard);
        result
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(super::format_service_error)
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn workspace_content_check_cancel(
    registry: State<'_, ContentCheckRegistry>,
    checkId: String,
) -> Result<(), String> {
    registry.cancel(&checkId)
}

#[tauri::command]
pub async fn workspace_content_check_cached(
    service: State<'_, SkillsHubService>,
) -> Result<Option<ContentCheckRun>, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.cached_content_checks())
        .await
        .map_err(|e| e.to_string())?
        .map_err(super::format_service_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_before_start_is_observed_without_resetting_another_operation() {
        let registry = ContentCheckRegistry::default();
        let id = uuid::Uuid::new_v4().to_string();
        let unrelated = CancelToken::new();
        registry.cancel(&id).unwrap();
        let run = registry.begin(&id).unwrap();
        assert!(run.token.is_cancelled());
        assert!(!unrelated.is_cancelled());
        drop(run);
        assert!(registry.begin(&id).is_err());
    }

    #[test]
    fn stale_cancel_cannot_cancel_the_current_run_and_guard_releases_busy_state() {
        let registry = ContentCheckRegistry::default();
        let first_id = uuid::Uuid::new_v4().to_string();
        let next_id = uuid::Uuid::new_v4().to_string();
        let first = registry.begin(&first_id).unwrap();
        assert!(registry
            .begin(&next_id)
            .err()
            .unwrap()
            .starts_with("OPERATION_BUSY"));
        drop(first);
        let next = registry.begin(&next_id).unwrap();
        registry.cancel(&first_id).unwrap();
        assert!(!next.token.is_cancelled());
        registry.cancel(&next_id).unwrap();
        assert!(next.token.is_cancelled());
        drop(next);
        assert!(registry.begin(&uuid::Uuid::new_v4().to_string()).is_ok());
    }

    #[test]
    fn cancellation_registry_rejects_invalid_ids_and_bounds_tombstones() {
        let registry = ContentCheckRegistry::default();
        assert!(registry.cancel("not-a-check").is_err());
        for _ in 0..(RECENT_LIMIT + 2) {
            registry.cancel(&uuid::Uuid::new_v4().to_string()).unwrap();
        }
        assert_eq!(registry.0.lock().unwrap().recent.len(), RECENT_LIMIT);
    }
}
