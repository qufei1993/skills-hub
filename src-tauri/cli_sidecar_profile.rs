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

#[cfg(test)]
mod manifest_tests {
    #[test]
    fn release_metadata_is_valid_without_a_binary_and_rejects_identity_changes() {
        let commit = "a".repeat(40);
        let metadata = serde_json::json!({"version":"0.11.0","sourceCommit":commit,"target":"aarch64-apple-darwin","profile":"release","assetName":"skillshub-cli-0.11.0-darwin-arm64","size":3,"sha256":"a".repeat(64)});
        assert!(super::validate_manifest(
            &metadata,
            "0.11.0",
            "aarch64-apple-darwin",
            "release",
            &commit
        )
        .is_ok());
        for (field, value) in [
            ("version", serde_json::json!("0.10.1")),
            ("sourceCommit", serde_json::json!("b".repeat(40))),
            ("target", serde_json::json!("x86_64-apple-darwin")),
            ("profile", serde_json::json!("debug")),
            ("size", serde_json::json!(0)),
            ("assetName", serde_json::json!("../cli")),
            ("sha256", serde_json::json!("z".repeat(64))),
        ] {
            let mut altered = metadata.clone();
            altered[field] = value;
            assert!(
                super::validate_manifest(
                    &altered,
                    "0.11.0",
                    "aarch64-apple-darwin",
                    "release",
                    &commit
                )
                .is_err(),
                "{field}"
            );
        }
    }
}

pub fn validate_manifest(
    metadata: &serde_json::Value,
    version: &str,
    target: &str,
    profile: &str,
    commit: &str,
) -> Result<(), &'static str> {
    let platform = match target {
        "aarch64-apple-darwin" => "darwin-arm64",
        "x86_64-apple-darwin" => "darwin-x64",
        "x86_64-pc-windows-msvc" => "windows-x64",
        "aarch64-unknown-linux-gnu" => "linux-arm64",
        "x86_64-unknown-linux-gnu" => "linux-x64",
        _ => return Err("CLI_MANIFEST_INVALID"),
    };
    let hex = |text: &str, length| {
        text.len() == length
            && text
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    };
    let asset = format!(
        "skillshub-cli-{version}-{platform}{}",
        if target.contains("windows") {
            ".exe"
        } else {
            ""
        }
    );
    if metadata["version"].as_str() != Some(version)
        || metadata["target"].as_str() != Some(target)
        || metadata["profile"].as_str() != Some(profile)
        || metadata["sourceCommit"].as_str() != Some(commit)
        || !hex(commit, 40)
        || !hex(metadata["sha256"].as_str().unwrap_or(""), 64)
        || metadata["size"].as_u64().unwrap_or(0) == 0
        || metadata["assetName"].as_str() != Some(&asset)
    {
        return Err("CLI_MANIFEST_MISMATCH");
    }
    Ok(())
}

pub fn validate_local_test_config(config: &serde_json::Value) -> Result<(), &'static str> {
    if config["productName"] != "Skills Hub Local Test"
        || config["mainBinaryName"] != "skills-hub-local-test"
        || config["identifier"] != "com.qufei1993.skillshub.local-test"
        || config["bundle"]["createUpdaterArtifacts"] != false
        || config["bundle"]["externalBin"] != serde_json::json!([])
        || config["plugins"]["updater"]["endpoints"] != serde_json::json!([])
    {
        return Err("LOCAL_TEST_BUILD_CONFIG_REQUIRED");
    }
    Ok(())
}

#[cfg(test)]
mod local_test_config_tests {
    #[test]
    fn local_test_requires_distinct_identity_binary_and_disabled_release_updater() {
        let config = serde_json::json!({
            "productName": "Skills Hub Local Test", "mainBinaryName": "skills-hub-local-test",
            "identifier": "com.qufei1993.skillshub.local-test",
            "bundle": { "createUpdaterArtifacts": false, "externalBin": [] },
            "plugins": { "updater": { "endpoints": [] } }
        });
        assert!(super::validate_local_test_config(&config).is_ok());
        for field in [
            "productName",
            "mainBinaryName",
            "identifier",
            "bundle",
            "plugins",
        ] {
            let mut altered = config.clone();
            altered.as_object_mut().unwrap().remove(field);
            assert_eq!(
                super::validate_local_test_config(&altered),
                Err("LOCAL_TEST_BUILD_CONFIG_REQUIRED")
            );
        }
        let mut altered = config.clone();
        altered["plugins"]["updater"]["endpoints"] =
            serde_json::json!(["https://example.com/updater.json"]);
        assert!(super::validate_local_test_config(&altered).is_err());
        let mut altered = config;
        altered["bundle"]["createUpdaterArtifacts"] = serde_json::json!(true);
        assert!(super::validate_local_test_config(&altered).is_err());
    }
}
