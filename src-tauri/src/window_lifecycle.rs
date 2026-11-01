use crate::core::{skill_store::SkillStore, window_behavior::should_hide_on_close};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "linux")]
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, Runtime,
};

pub const TRAY_ID: &str = "skills-hub-tray";
const SHOW: &str = "tray-show";
const QUIT: &str = "tray-quit";

pub struct WindowLifecycle {
    host_available: AtomicBool,
    #[cfg(target_os = "linux")]
    stopping: Arc<AtomicBool>,
    #[cfg(target_os = "linux")]
    monitor: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl WindowLifecycle {
    pub fn new() -> Self {
        Self {
            host_available: AtomicBool::new(!cfg!(target_os = "linux")),
            #[cfg(target_os = "linux")]
            stopping: Arc::new(AtomicBool::new(false)),
            #[cfg(target_os = "linux")]
            monitor: Mutex::new(None),
        }
    }
    pub fn should_hide<R: Runtime>(&self, app: &tauri::AppHandle<R>) -> bool {
        should_hide_on_close(
            app.tray_by_id(TRAY_ID).is_some(),
            self.host_available.load(Ordering::SeqCst),
        )
    }
    #[cfg(any(target_os = "linux", test))]
    fn update_host_availability(&self, available: bool) -> bool {
        self.host_available.swap(available, Ordering::SeqCst) && !available
    }

    pub fn stop(&self) {
        #[cfg(target_os = "linux")]
        {
            self.stopping.store(true, Ordering::SeqCst);
            if let Ok(mut monitor) = self.monitor.lock() {
                if let Some(thread) = monitor.take() {
                    thread.thread().unpark();
                    if thread.join().is_err() {
                        log::warn!("tray host monitor stopped unexpectedly");
                    }
                }
            }
        }
    }
}

pub fn show_main_window<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().context("show main window")?;
        window.unminimize().context("unminimize main window")?;
        window.set_focus().context("focus main window")?;
    }
    Ok(())
}

#[derive(Clone, Deserialize)]
struct TrayLabels {
    show: String,
    quit: String,
}
fn labels(language: &str) -> Result<TrayLabels> {
    let all: std::collections::HashMap<String, TrayLabels> =
        serde_json::from_str(include_str!("../../src/i18n/trayMenu.json"))?;
    all.get(language)
        .cloned()
        .context("unsupported tray language")
}
fn menu<R: Runtime>(app: &tauri::AppHandle<R>, language: &str) -> Result<Menu<R>> {
    let labels = labels(language)?;
    let show = MenuItem::with_id(app, SHOW, labels.show, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT, labels.quit, true, None::<&str>)?;
    Ok(Menu::with_items(app, &[&show, &quit])?)
}
pub fn set_language<R: Runtime>(
    app: &tauri::AppHandle<R>,
    store: &SkillStore,
    language: &str,
) -> Result<()> {
    let menu = menu(app, language)?;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(menu))?;
    }
    store
        .set_setting("tray_language", language)
        .context("save tray language")
}

pub fn setup<R: Runtime>(app: &tauri::AppHandle<R>, store: &SkillStore) {
    app.manage(WindowLifecycle::new());
    let language = store
        .get_setting("tray_language")
        .ok()
        .flatten()
        .filter(|lang| matches!(lang.as_str(), "en" | "zh" | "ko"))
        .unwrap_or_else(|| "en".into());
    let create = (|| -> Result<()> {
        let icon = app
            .default_window_icon()
            .context("application icon unavailable")?
            .clone();
        TrayIconBuilder::with_id(TRAY_ID)
            .icon(icon)
            .menu(&menu(app, &language)?)
            .tooltip("Skills Hub")
            .show_menu_on_left_click(false)
            .on_menu_event(|app, event| match event.id().as_ref() {
                SHOW => {
                    if let Err(error) = show_main_window(app) {
                        log::warn!("window restore failed: {error:#}");
                    }
                }
                QUIT => app.exit(0),
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                if matches!(
                    event,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    }
                ) {
                    if let Err(error) = show_main_window(tray.app_handle()) {
                        log::warn!("window restore failed: {error:#}");
                    }
                }
            })
            .build(app)?;
        Ok(())
    })();
    if let Err(error) = create {
        log::warn!("tray unavailable; closing exits: {error:#}");
    }
    #[cfg(target_os = "linux")]
    start_linux_host_monitor(app);
}

#[cfg(target_os = "linux")]
fn start_linux_host_monitor<R: Runtime>(app: &tauri::AppHandle<R>) {
    use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
    let state = app.state::<WindowLifecycle>();
    let stop = state.stopping.clone();
    let handle = app.clone();
    let thread = std::thread::spawn(move || {
        let mut connection = None;
        while !stop.load(Ordering::SeqCst) {
            if connection.is_none() {
                connection = dbus::blocking::Connection::new_session().ok();
            }
            let available = connection.as_ref().is_some_and(|bus| {
                [
                    "org.kde.StatusNotifierWatcher",
                    "org.freedesktop.StatusNotifierWatcher",
                ]
                .iter()
                .any(|service| {
                    let proxy = bus.with_proxy(
                        *service,
                        "/StatusNotifierWatcher",
                        std::time::Duration::from_millis(300),
                    );
                    proxy
                        .get::<bool>(*service, "IsStatusNotifierHostRegistered")
                        .unwrap_or(false)
                })
            });
            let state = handle.state::<WindowLifecycle>();
            let host_lost = state.update_host_availability(available);
            if !available {
                connection = None;
            }
            if host_lost {
                let restore = handle.clone();
                let _ = handle.run_on_main_thread(move || {
                    if let Some(window) = restore.get_webview_window("main") {
                        if window.is_visible().is_ok_and(|visible| !visible) {
                            if let Err(error) = show_main_window(&restore) {
                                log::warn!("tray host lost; window restore failed: {error:#}");
                            }
                        }
                    }
                });
            }
            std::thread::park_timeout(std::time::Duration::from_secs(1));
        }
    });
    *state.monitor.lock().expect("new monitor lock") = Some(thread);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tray_menu_has_localized_show_and_explicit_quit_in_all_languages() {
        for language in ["en", "zh", "ko"] {
            let labels = labels(language).unwrap();
            assert!(!labels.show.is_empty());
            assert!(!labels.quit.is_empty());
            assert_ne!(labels.show, labels.quit);
        }
        assert!(labels("unsupported").is_err());
    }
    #[test]
    fn no_native_tray_never_hides_the_window() {
        let app = tauri::test::mock_app();
        let state = WindowLifecycle::new();
        assert!(!state.should_hide(app.handle()));
    }
    #[test]
    fn losing_the_tray_host_triggers_one_recovery_until_it_returns() {
        let state = WindowLifecycle::new();
        state.host_available.store(false, Ordering::SeqCst);
        assert!(!state.update_host_availability(false));
        assert!(!state.update_host_availability(true));
        assert!(state.update_host_availability(false));
        assert!(!state.update_host_availability(false));
        assert!(!state.update_host_availability(true));
        assert!(state.update_host_availability(false));
    }
}
