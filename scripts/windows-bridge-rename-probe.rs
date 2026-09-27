#[cfg(windows)]
#[path = "../src-tauri/src/core/cli_bridge/directory/windows_rename.rs"]
mod windows_rename;

#[cfg(windows)]
#[test]
fn guarded_directory_supports_native_rename_and_replacement() {
    use std::{fs, fs::OpenOptions, io::Write, os::windows::fs::OpenOptionsExt};
    let root = std::env::temp_dir().join(format!("skills-hub-rename-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let guard = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(0x00200000 | 0x02000000)
        .open(&root)
        .unwrap();
    for value in [b"first".as_slice(), b"replacement".as_slice()] {
        let source = root.join("source");
        let mut file = OpenOptions::new()
            .access_mode(0x80000000 | 0x40000000 | 0x10000)
            .share_mode(1)
            .create_new(true)
            .custom_flags(0x00200000)
            .open(&source)
            .unwrap();
        file.write_all(value).unwrap();
        assert_eq!(
            OpenOptions::new()
                .write(true)
                .open(&source)
                .unwrap_err()
                .raw_os_error(),
            Some(32)
        );
        windows_rename::rename(&file, "target").unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read(root.join("target")).unwrap(), value);
        assert_eq!(
            OpenOptions::new()
                .write(true)
                .share_mode(7)
                .custom_flags(0x00200000 | 0x02000000)
                .open(&root)
                .unwrap_err()
                .raw_os_error(),
            Some(32)
        );
    }
    let source = root.join("failed");
    let file = OpenOptions::new()
        .access_mode(0x80000000 | 0x40000000 | 0x10000)
        .share_mode(1)
        .create_new(true)
        .open(&source)
        .unwrap();
    let held = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(root.join("target"))
        .unwrap();
    assert!(windows_rename::rename(&file, "target").is_err());
    windows_rename::delete(&file).unwrap();
    drop(file);
    assert!(!source.exists());
    assert_eq!(fs::read(root.join("target")).unwrap(), b"replacement");
    drop(held);
    drop(guard);
    fs::remove_dir_all(root).unwrap();
}
