fn main() {
    prepare_cli_bridge_metadata();
    // 确保替换图标后，`tauri dev` 的构建会重新触发（否则 Cargo 可能不重跑 build.rs，Dock 仍显示旧图标）。
    println!("cargo:rerun-if-changed=icons/icon.png");
    println!("cargo:rerun-if-changed=icons/icon.icns");
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=tauri.conf.json");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap();
    if target_os == "windows" && target_env == "msvc" {
        // Library test executables also need Common Controls v6 to load Tauri.
        let manifest = std::env::current_dir()
            .unwrap()
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        tauri_build::try_build(
            tauri_build::Attributes::new()
                .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest()),
        )
        .expect("failed to run tauri-build");
    } else {
        tauri_build::build();
    }
}

fn prepare_cli_bridge_metadata() {
    use sha2::{Digest, Sha256};
    use std::{env, fs, path::PathBuf};

    println!("cargo:rerun-if-env-changed=SKILLS_HUB_PREPARE_CLI_SIDECAR");
    println!("cargo:rerun-if-changed=cli_sidecar_profile.rs");
    let profile = env::var("PROFILE").expect("CLI_BRIDGE_UNSUPPORTED_PROFILE");
    let output = PathBuf::from(env::var("OUT_DIR").expect("CLI_BRIDGE_UNSUPPORTED_PROFILE"));
    let output_profile = output
        .ancestors()
        .nth(3)
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .expect("CLI_BRIDGE_UNSUPPORTED_PROFILE");
    cli_sidecar_profile::validate_profile(&profile, &profile, output_profile)
        .unwrap_or_else(|reason| panic!("{reason}"));
    println!(
        "cargo:rustc-env=SKILLS_HUB_EXPECT_DEBUG_ASSERTIONS={}",
        if profile == "debug" { "1" } else { "0" }
    );
    if env::var("SKILLS_HUB_PREPARE_CLI_SIDECAR").as_deref() == Ok("1") {
        // The CLI must be built before Tauri can copy it as an external binary.
        let mut config: serde_json::Value = env::var("TAURI_CONFIG")
            .map(|value| serde_json::from_str(&value).expect("invalid TAURI_CONFIG"))
            .unwrap_or_else(|_| serde_json::json!({}));
        config["bundle"]["externalBin"] = serde_json::json!([]);
        env::set_var("TAURI_CONFIG", config.to_string());
        println!("cargo:rustc-env=TAURI_CONFIG={config}");
        return;
    }
    let target = env::var("TARGET").expect("missing build target");
    let base = PathBuf::from("binaries").join(format!("skillshub-cli-{target}"));
    let metadata_path = base.with_extension("json");
    let binary_path = if target.contains("windows") {
        base.with_extension("exe")
    } else {
        base
    };
    println!("cargo:rerun-if-changed={}", metadata_path.display());
    println!("cargo:rerun-if-changed={}", binary_path.display());
    let metadata: serde_json::Value = serde_json::from_slice(
        &fs::read(metadata_path).expect("prepare CLI sidecar metadata before building desktop"),
    )
    .expect("invalid CLI sidecar metadata");
    let version = env::var("CARGO_PKG_VERSION").expect("missing package version");
    cli_sidecar_profile::validate_profile(
        metadata["profile"].as_str().unwrap_or(""),
        &profile,
        output_profile,
    )
    .unwrap_or_else(|reason| panic!("{reason}"));
    assert_eq!(
        metadata["version"].as_str(),
        Some(version.as_str()),
        "CLI sidecar version mismatch"
    );
    assert_eq!(
        metadata["target"].as_str(),
        Some(target.as_str()),
        "CLI sidecar target mismatch"
    );
    let binary = fs::read(binary_path).expect("prepare CLI sidecar binary before building desktop");
    let hash = format!("{:x}", Sha256::digest(binary));
    assert_eq!(
        metadata["sha256"].as_str(),
        Some(hash.as_str()),
        "CLI sidecar hash mismatch"
    );
    println!("cargo:rustc-env=SKILLS_HUB_BUNDLED_CLI_SHA256={hash}");
}
mod cli_sidecar_profile;
