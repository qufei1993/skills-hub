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
        // CLI bootstrap builds have no desktop download metadata.
        let mut config: serde_json::Value = env::var("TAURI_CONFIG")
            .map(|value| serde_json::from_str(&value).expect("invalid TAURI_CONFIG"))
            .unwrap_or_else(|_| serde_json::json!({}));
        config["bundle"]["externalBin"] = serde_json::json!([]);
        env::set_var("TAURI_CONFIG", config.to_string());
        println!("cargo:rustc-env=TAURI_CONFIG={config}");
        return;
    }
    println!("cargo:rerun-if-env-changed=SKILLS_HUB_CLI_MANIFEST_PATH");
    let target = env::var("TARGET").expect("missing build target");
    let base = PathBuf::from("binaries").join(format!("skillshub-cli-{target}"));
    let metadata_path = env::var_os("SKILLS_HUB_CLI_MANIFEST_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            assert_eq!(profile, "debug", "CLI_MANIFEST_REQUIRED");
            base.with_extension("json")
        });
    println!("cargo:rerun-if-changed={}", metadata_path.display());
    let metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(&metadata_path).expect("CLI_MANIFEST_REQUIRED"))
            .expect("CLI_MANIFEST_INVALID");
    let version = env::var("CARGO_PKG_VERSION").expect("missing package version");
    cli_sidecar_profile::validate_profile(
        metadata["profile"].as_str().unwrap_or(""),
        &profile,
        output_profile,
    )
    .unwrap_or_else(|reason| panic!("{reason}"));
    let revision = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("CLI_MANIFEST_SOURCE_UNAVAILABLE");
    assert!(revision.status.success(), "CLI_MANIFEST_SOURCE_UNAVAILABLE");
    let commit = String::from_utf8(revision.stdout).expect("CLI_MANIFEST_SOURCE_UNAVAILABLE");
    cli_sidecar_profile::validate_manifest(&metadata, &version, &target, &profile, commit.trim())
        .unwrap_or_else(|reason| panic!("{reason}"));
    if let Ok(head) = std::process::Command::new("git")
        .args(["rev-parse", "--git-path", "HEAD"])
        .output()
    {
        println!(
            "cargo:rerun-if-changed={}",
            String::from_utf8_lossy(&head.stdout).trim()
        );
    }
    if let Ok(reference) = std::process::Command::new("git")
        .args(["symbolic-ref", "-q", "HEAD"])
        .output()
    {
        if reference.status.success() {
            let name = String::from_utf8_lossy(&reference.stdout);
            let filename = std::process::Command::new("git")
                .args(["rev-parse", "--git-path", name.trim()])
                .output()
                .unwrap();
            println!(
                "cargo:rerun-if-changed={}",
                String::from_utf8_lossy(&filename.stdout).trim()
            );
        }
    }
    if profile == "debug" {
        let binary_path = if target.contains("windows") {
            base.with_extension("exe")
        } else {
            base
        };
        println!("cargo:rerun-if-changed={}", binary_path.display());
        let binary =
            fs::read(&binary_path).expect("prepare local debug CLI before building desktop");
        assert_eq!(
            metadata["size"].as_u64(),
            Some(binary.len() as u64),
            "CLI_MANIFEST_SIZE_MISMATCH"
        );
        let hash = format!("{:x}", Sha256::digest(binary));
        assert_eq!(
            metadata["sha256"].as_str(),
            Some(hash.as_str()),
            "CLI_MANIFEST_HASH_MISMATCH"
        );
        println!(
            "cargo:rustc-env=SKILLS_HUB_BUNDLED_CLI_SOURCE_PATH={}",
            env::current_dir().unwrap().join(binary_path).display()
        );
    }
    println!(
        "cargo:rustc-env=SKILLS_HUB_BUNDLED_CLI_SHA256={}",
        metadata["sha256"].as_str().unwrap()
    );
    println!("cargo:rustc-env=SKILLS_HUB_CLI_MANIFEST={metadata}");
}
mod cli_sidecar_profile;
