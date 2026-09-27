#[path = "../src-tauri/src/core/process.rs"]
mod process;

#[cfg(windows)]
fn main() {
    if std::env::args().any(|arg| arg == "--child") {
        #[link(name = "Kernel32")]
        extern "system" {
            fn GetConsoleWindow() -> *mut std::ffi::c_void;
        }

        let has_console = unsafe { !GetConsoleWindow().is_null() };
        println!("CONSOLE_WINDOW={has_console}");
        return;
    }

    probe_bridge_rename();

    let output = process::background_command(std::env::current_exe().unwrap())
        .arg("--child")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("CONSOLE_WINDOW=false"));
}

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn probe_bridge_rename() {
    use std::{fs, fs::OpenOptions, os::windows::fs::OpenOptionsExt};
    for access in [0x80000000, 0x80] {
        let root =
            std::env::temp_dir().join(format!("skills-hub-rename-{}-{access}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let guard = OpenOptions::new()
            .access_mode(access)
            .share_mode(1)
            .custom_flags(0x00200000 | 0x02000000)
            .open(&root)
            .unwrap();
        let source = root.join("source");
        let target = root.join("target");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .custom_flags(0x00200000)
            .open(&source)
            .unwrap();
        let result = fs::rename(&source, &target);
        println!("RENAME access={access:x} source_open=true guard_open=true result={result:?}");
        drop(file);
        if source.exists() {
            let result = fs::rename(&source, &target);
            println!(
                "RENAME access={access:x} source_open=false guard_open=true result={result:?}"
            );
        }
        drop(guard);
        if source.exists() {
            let result = fs::rename(&source, &target);
            println!(
                "RENAME access={access:x} source_open=false guard_open=false result={result:?}"
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}
