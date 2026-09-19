fn main() {
    prepare_cli_bridge_metadata();
    // 确保替换图标后，`tauri dev` 的构建会重新触发（否则 Cargo 可能不重跑 build.rs，Dock 仍显示旧图标）。
    println!("cargo:rerun-if-changed=icons/icon.png");
    println!("cargo:rerun-if-changed=icons/icon.icns");
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=tauri.conf.json");
    tauri_build::build()
}

fn prepare_cli_bridge_metadata() {
    use sha2::{Digest, Sha256};
    use std::{env, fs, path::PathBuf};

    println!("cargo:rerun-if-env-changed=SKILLS_HUB_PREPARE_CLI_SIDECAR");
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
