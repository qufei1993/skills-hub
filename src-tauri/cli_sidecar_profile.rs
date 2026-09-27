pub fn validate_profile(sidecar: &str, cargo: &str, output: &str) -> Result<(), &'static str> {
    if ![sidecar, cargo, output]
        .iter()
        .all(|profile| matches!(*profile, "debug" | "release"))
    {
        return Err("CLI_BRIDGE_UNSUPPORTED_PROFILE");
    }
    if sidecar != cargo || output != cargo {
        return Err("CLI_BRIDGE_PROFILE_MISMATCH");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_profile;

    #[test]
    fn cli_bridge_profile_rejects_both_mismatched_directions() {
        for (sidecar, desktop) in [("debug", "release"), ("release", "debug")] {
            assert_eq!(
                validate_profile(sidecar, desktop, desktop),
                Err("CLI_BRIDGE_PROFILE_MISMATCH")
            );
        }
        assert!(validate_profile("debug", "debug", "debug").is_ok());
        assert!(validate_profile("release", "release", "release").is_ok());
    }

    #[test]
    fn cli_bridge_profile_rejects_unapproved_custom_profiles() {
        assert_eq!(
            validate_profile("release", "release", "custom"),
            Err("CLI_BRIDGE_UNSUPPORTED_PROFILE")
        );
        assert_eq!(
            validate_profile("custom", "custom", "custom"),
            Err("CLI_BRIDGE_UNSUPPORTED_PROFILE")
        );
    }
}
