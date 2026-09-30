use anyhow::{bail, Context, Result};
use std::path::Path;

fn path_conflicts_with_managed(path: &std::ffi::OsStr, managed: &Path) -> bool {
    let mut earlier_cli = false;
    let managed = managed
        .canonicalize()
        .unwrap_or_else(|_| managed.to_path_buf());
    for directory in std::env::split_paths(path) {
        let same = directory
            .canonicalize()
            .unwrap_or_else(|_| directory.clone())
            == managed;
        if same {
            return earlier_cli;
        }
        let names: &[&str] = if cfg!(windows) {
            &[
                "skillshub-cli.exe",
                "skillshub-cli.cmd",
                "skillshub-cli.bat",
                "skillshub-cli.com",
            ]
        } else {
            &["skillshub-cli"]
        };
        earlier_cli |= names.iter().map(|name| directory.join(name)).any(|binary| {
            if !binary.is_file() {
                return false;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                binary
                    .metadata()
                    .is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
            }
            #[cfg(windows)]
            {
                true
            }
        });
    }
    false
}

pub fn current_path_conflict(managed: &Path) -> bool {
    std::env::var_os("PATH").is_some_and(|path| path_conflicts_with_managed(&path, managed))
}

#[cfg(unix)]
fn profiles(home: &Path) -> Result<Vec<std::path::PathBuf>> {
    let shell = std::env::var("SHELL").unwrap_or_default();
    match Path::new(&shell).file_name().and_then(|name| name.to_str()) {
        Some("zsh") => Ok(vec![std::env::var_os("ZDOTDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| home.to_path_buf())
            .join(".zshrc")]),
        Some("bash") => {
            let login = [".bash_profile", ".bash_login", ".profile"]
                .into_iter()
                .map(|name| home.join(name))
                .find(|path| path.exists())
                .unwrap_or_else(|| home.join(".profile"));
            Ok(vec![home.join(".bashrc"), login])
        }
        _ => bail!("unsupported terminal shell"),
    }
}

#[cfg(unix)]
fn path_line(directory: &Path) -> Result<String> {
    let path = directory.to_str().context("invalid terminal path")?;
    if path.contains(['\n', '\r', ':']) {
        bail!("unsupported terminal path");
    }
    Ok(format!(
        "export PATH='{}':\"$PATH\"",
        path.replace('\'', "'\\''")
    ))
}

#[cfg(unix)]
fn update_profile(profile: &Path, line: &str) -> Result<()> {
    use std::io::Write;
    let destination = if profile.is_symlink() {
        profile
            .canonicalize()
            .context("invalid shell profile link")?
    } else {
        profile.to_path_buf()
    };
    let existing = match std::fs::read_to_string(&destination) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    if existing.lines().any(|entry| entry == line) {
        return Ok(());
    }
    let parent = destination
        .parent()
        .context("missing shell profile parent")?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    if let Ok(metadata) = std::fs::metadata(&destination) {
        staged.as_file().set_permissions(metadata.permissions())?;
    }
    write!(staged, "{existing}\n# Skills Hub CLI\n{line}\n")?;
    staged.as_file().sync_all()?;
    staged.persist(&destination)?;
    Ok(())
}

pub fn configured(home: &Path, directory: &Path) -> bool {
    #[cfg(unix)]
    {
        let Ok(profiles) = profiles(home) else {
            return false;
        };
        let Ok(line) = path_line(directory) else {
            return false;
        };
        profiles.iter().all(|profile| {
            std::fs::read_to_string(profile)
                .is_ok_and(|text| text.lines().any(|entry| entry == line))
        })
    }
    #[cfg(windows)]
    {
        let _ = home;
        windows::read_path("Environment")
            .is_ok_and(|path| contains_directory(&path, &directory.to_string_lossy()))
    }
}

pub fn configure(home: &Path, directory: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let line = path_line(directory)?;
        for profile in profiles(home)? {
            update_profile(&profile, &line)?;
        }
    }
    #[cfg(windows)]
    {
        let _ = home;
        windows::configure_path("Environment", directory)?;
    }
    Ok(())
}

#[cfg(any(windows, test))]
fn contains_directory(path: &str, directory: &str) -> bool {
    path.split(';').any(|entry| {
        entry
            .trim()
            .trim_matches('"')
            .trim_end_matches('\\')
            .eq_ignore_ascii_case(directory.trim_end_matches('\\'))
    })
}

#[cfg(windows)]
mod windows {
    use super::*;
    use windows_sys::Win32::System::Registry::{
        RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_EXPAND_SZ, RRF_NOEXPAND,
        RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
    };
    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    pub(super) fn read_path(key: &str) -> Result<String> {
        read_path_value(key).map(|(value, _)| value)
    }

    pub(super) fn read_path_value(key: &str) -> Result<(String, u32)> {
        let key = wide(key);
        let value = wide("Path");
        let mut size = 0;
        let mut kind = REG_EXPAND_SZ;
        let flags = RRF_NOEXPAND | RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ;
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                flags,
                &mut kind,
                std::ptr::null_mut(),
                &mut size,
            )
        };
        if status == 2 {
            return Ok((String::new(), REG_EXPAND_SZ));
        }
        if status != 0 {
            bail!("cannot read user PATH: {status}");
        }
        let mut buffer = vec![0u16; size as usize / 2 + 1];
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                flags,
                &mut kind,
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != 0 {
            bail!("cannot read user PATH: {status}");
        }
        let end = buffer
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(buffer.len());
        Ok((String::from_utf16(&buffer[..end])?, kind))
    }

    pub(super) fn configure_path(key: &str, directory: &Path) -> Result<()> {
        let directory = directory.to_str().context("invalid terminal path")?;
        if directory.contains([';', '\0']) {
            bail!("unsupported terminal path");
        }
        let (current, kind) = read_path_value(key)?;
        if !contains_directory(&current, directory) {
            let path = wide(&format!("{directory};{current}"));
            let status = unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    wide(key).as_ptr(),
                    wide("Path").as_ptr(),
                    kind,
                    path.as_ptr().cast(),
                    (path.len() * 2) as u32,
                )
            };
            if status != 0 {
                bail!("cannot update user PATH: {status}");
            }
        }
        if key == "Environment" {
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
            };
            unsafe {
                SendMessageTimeoutW(
                    HWND_BROADCAST,
                    WM_SETTINGCHANGE,
                    0,
                    wide("Environment").as_ptr() as isize,
                    SMTO_ABORTIFHUNG,
                    1000,
                    std::ptr::null_mut(),
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reports_when_an_older_cli_precedes_the_managed_directory() {
        let root = tempfile::tempdir().unwrap();
        let older = root.path().join("older");
        let managed = root.path().join("managed");
        std::fs::create_dir_all(&older).unwrap();
        std::fs::create_dir_all(&managed).unwrap();
        let binary = if cfg!(windows) {
            "skillshub-cli.exe"
        } else {
            "skillshub-cli"
        };
        std::fs::write(older.join(binary), "old").unwrap();
        std::fs::write(managed.join(binary), "managed").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(older.join(binary), std::fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        let path = std::env::join_paths([&older, &managed]).unwrap();
        assert!(path_conflicts_with_managed(&path, &managed));
        let reversed = std::env::join_paths([&managed, &older]).unwrap();
        assert!(!path_conflicts_with_managed(&reversed, &managed));
    }

    #[cfg(windows)]
    #[test]
    fn detects_older_windows_command_shims() {
        let root = tempfile::tempdir().unwrap();
        let older = root.path().join("older");
        let managed = root.path().join("managed");
        std::fs::create_dir_all(&older).unwrap();
        std::fs::create_dir_all(&managed).unwrap();
        std::fs::write(managed.join("skillshub-cli.exe"), "managed").unwrap();
        let path = std::env::join_paths([&older, &managed]).unwrap();
        for extension in ["cmd", "bat"] {
            let shim = older.join(format!("skillshub-cli.{extension}"));
            std::fs::write(&shim, "old").unwrap();
            assert!(path_conflicts_with_managed(&path, &managed));
            std::fs::remove_file(shim).unwrap();
        }
    }
    #[test]
    fn windows_path_matching_preserves_other_entries() {
        assert!(contains_directory("C:\\Other;\"c:\\Tools\\\"", "C:\\tools"));
        assert!(!contains_directory("C:\\Tools-old", "C:\\Tools"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_user_path_setup_preserves_values_and_does_not_duplicate() {
        use windows_sys::Win32::System::Registry::{
            RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, HKEY_CURRENT_USER, KEY_ALL_ACCESS,
        };
        let key = format!(
            "Software\\SkillsHubTerminalTest-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let key_wide: Vec<u16> = key.encode_utf16().chain(Some(0)).collect();
        let mut handle = std::ptr::null_mut();
        assert_eq!(
            unsafe {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    key_wide.as_ptr(),
                    0,
                    std::ptr::null(),
                    0,
                    KEY_ALL_ACCESS,
                    std::ptr::null(),
                    &mut handle,
                    std::ptr::null_mut(),
                )
            },
            0
        );
        unsafe {
            RegCloseKey(handle);
        }
        struct Cleanup(Vec<u16>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                unsafe {
                    RegDeleteTreeW(HKEY_CURRENT_USER, self.0.as_ptr());
                }
            }
        }
        let _cleanup = Cleanup(key_wide);
        windows::configure_path(&key, Path::new("C:\\existing")).unwrap();
        windows::configure_path(&key, Path::new("C:\\Users\\test user\\.skills-hub\\bin")).unwrap();
        let first = windows::read_path(&key).unwrap();
        windows::configure_path(&key, Path::new("C:\\Users\\test user\\.skills-hub\\bin")).unwrap();
        assert_eq!(windows::read_path(&key).unwrap(), first);
        assert!(first.contains("C:\\existing"));
        assert!(first.starts_with("C:\\Users\\test user\\.skills-hub\\bin;"));
    }

    #[cfg(unix)]
    #[test]
    fn shell_setup_preserves_content_quotes_paths_and_is_idempotent() {
        let root = tempfile::tempdir().unwrap();
        let profile = root.path().join(".zshrc");
        let directory = root.path().join("user's tools");
        std::fs::write(&profile, "# keep my settings\n").unwrap();
        std::fs::create_dir(&directory).unwrap();
        let binary = directory.join("skillshub-cli");
        std::fs::write(&binary, "#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        let line = path_line(&directory).unwrap();
        update_profile(&profile, &line).unwrap();
        let content = std::fs::read_to_string(&profile).unwrap();
        update_profile(&profile, &line).unwrap();
        assert_eq!(std::fs::read_to_string(&profile).unwrap(), content);
        assert!(content.starts_with("# keep my settings\n"));
        let output = std::process::Command::new("bash")
            .args([
                "-c",
                "source \"$1\"; command -v skillshub-cli; skillshub-cli",
                "test",
            ])
            .arg(&profile)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            binary.to_str().unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn terminal_status_never_creates_profile_files() {
        let root = tempfile::tempdir().unwrap();
        assert!(!configured(root.path(), &root.path().join("bin")));
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn shell_setup_preserves_profile_symlinks_and_fails_without_overwriting_invalid_content() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("dotfile");
        let profile = root.path().join(".zshrc");
        std::fs::write(&target, [255]).unwrap();
        std::os::unix::fs::symlink(&target, &profile).unwrap();
        assert!(update_profile(&profile, "export PATH='/cli':\"$PATH\"").is_err());
        assert_eq!(std::fs::read(&target).unwrap(), [255]);
        std::fs::write(&target, "# settings\n").unwrap();
        update_profile(&profile, "export PATH='/cli':\"$PATH\"").unwrap();
        assert!(profile.is_symlink());
        assert!(std::fs::read_to_string(&target)
            .unwrap()
            .contains("export PATH="));
    }
}
