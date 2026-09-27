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
        if source.exists() {
            probe_handle_rename(&source, &target, &guard);
        }
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

#[cfg(windows)]
fn probe_handle_rename(source: &std::path::Path, target: &std::path::Path, guard: &std::fs::File) {
    use std::{
        ffi::c_void,
        fs::OpenOptions,
        os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle},
    };
    #[repr(C)]
    struct RenameInfo {
        flags: u32,
        root: *mut c_void,
        length: u32,
        name: [u16; 2048],
    }
    #[repr(C)]
    struct IoStatus {
        status: usize,
        information: usize,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn SetFileInformationByHandle(
            file: *mut c_void,
            class: i32,
            info: *const c_void,
            size: u32,
        ) -> i32;
    }
    #[link(name = "ntdll")]
    extern "system" {
        fn NtSetInformationFile(
            file: *mut c_void,
            status: *mut IoStatus,
            info: *const c_void,
            size: u32,
            class: i32,
        ) -> i32;
    }
    let file = OpenOptions::new()
        .access_mode(0x10000)
        .share_mode(7)
        .custom_flags(0x00200000)
        .open(source)
        .unwrap();
    let mut info = RenameInfo {
        flags: 1,
        root: std::ptr::null_mut(),
        length: 0,
        name: [0; 2048],
    };
    let name: Vec<u16> = target.as_os_str().encode_wide().collect();
    info.name[..name.len()].copy_from_slice(&name);
    info.length = (name.len() * 2) as u32;
    let offset = info.name.as_ptr() as usize - &info as *const RenameInfo as usize;
    let result = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            3,
            (&info as *const RenameInfo).cast(),
            (offset + name.len() * 2) as u32,
        )
    };
    println!(
        "HANDLE_RENAME win32 result={result} error={:?}",
        std::io::Error::last_os_error()
    );
    if source.exists() {
        let name: Vec<u16> = target.file_name().unwrap().encode_wide().collect();
        info.name.fill(0);
        info.name[..name.len()].copy_from_slice(&name);
        info.length = (name.len() * 2) as u32;
        info.root = std::ptr::null_mut();
        let mut status = IoStatus {
            status: 0,
            information: 0,
        };
        let result = unsafe {
            NtSetInformationFile(
                file.as_raw_handle(),
                &mut status,
                (&info as *const RenameInfo).cast(),
                (offset + name.len() * 2) as u32,
                10,
            )
        };
        println!("HANDLE_RENAME nt_same_directory result={result:x}");
        if result < 0 {
            info.root = guard.as_raw_handle();
            let result = unsafe {
                NtSetInformationFile(
                    file.as_raw_handle(),
                    &mut status,
                    (&info as *const RenameInfo).cast(),
                    (offset + name.len() * 2) as u32,
                    10,
                )
            };
            println!("HANDLE_RENAME nt_relative result={result:x}");
        }
    }
}
