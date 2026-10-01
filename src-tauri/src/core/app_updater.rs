pub fn builder() -> tauri_plugin_updater::Builder {
    #[allow(unused_mut)]
    let mut builder = tauri_plugin_updater::Builder::new();
    #[cfg(target_os = "linux")]
    if let Ok(executable) = tauri::utils::platform::current_exe() {
        if let Some(target) = linux_target(
            std::env::consts::ARCH,
            &executable,
            std::path::Path::new("/var/lib/dpkg").exists(),
            std::path::Path::new("/etc/apt").exists(),
            || {
                super::process::background_command("dpkg")
                    .args(["-S", &executable.to_string_lossy()])
                    .output()
                    .is_ok_and(|output| output.status.success())
            },
        ) {
            builder = builder.target(target);
        }
    }
    builder
}

#[cfg(any(target_os = "linux", test))]
fn linux_target(
    arch: &str,
    executable: &std::path::Path,
    dpkg_exists: bool,
    apt_exists: bool,
    owns_executable: impl FnOnce() -> bool,
) -> Option<String> {
    // Match the updater plugin's Debian installer detection before selecting its bytes.
    if executable
        .to_str()
        .is_some_and(|path| path.starts_with("/usr"))
        && dpkg_exists
        && apt_exists
        && owns_executable()
    {
        Some(format!("linux-{arch}-deb"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::linux_target;
    use std::path::Path;

    #[test]
    fn deb_updates_match_the_installed_package_on_both_architectures() {
        for arch in ["x86_64", "aarch64"] {
            assert_eq!(
                linux_target(arch, Path::new("/usr/bin/app"), true, true, || true),
                Some(format!("linux-{arch}-deb"))
            );
        }
    }

    #[test]
    fn appimages_and_unmanaged_binaries_keep_the_default_update_target() {
        for (path, dpkg, apt, owned) in [
            ("/tmp/.mount/app", true, true, false),
            ("/home/user/Skills Hub.AppImage", true, true, false),
            ("/usr/local/bin/app", true, true, false),
            ("/usr/bin/app", false, true, true),
            ("/usr/bin/app", true, false, true),
        ] {
            assert_eq!(
                linux_target("x86_64", Path::new(path), dpkg, apt, || owned),
                None
            );
        }
    }

    #[test]
    fn non_system_paths_do_not_query_package_ownership() {
        assert_eq!(
            linux_target("aarch64", Path::new("/tmp/app"), true, true, || panic!(
                "unexpected dpkg query"
            )),
            None
        );
    }
}
