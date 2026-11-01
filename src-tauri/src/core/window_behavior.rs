pub fn should_hide_on_close(tray_created: bool, host_available: bool) -> bool {
    tray_created && host_available
}

pub fn uses_desktop_lifecycle(background_update: bool) -> bool {
    !background_update
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_usable_tray_keeps_running_without_opt_in() {
        assert!(should_hide_on_close(true, true));
    }

    #[test]
    fn missing_tray_or_host_falls_back_to_exiting() {
        assert!(!should_hide_on_close(false, false));
        assert!(!should_hide_on_close(false, true));
        assert!(!should_hide_on_close(true, false));
    }

    #[test]
    fn background_update_workers_skip_the_desktop_lifecycle() {
        assert!(uses_desktop_lifecycle(false));
        assert!(!uses_desktop_lifecycle(true));
    }
}
