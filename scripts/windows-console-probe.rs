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

    let output = process::background_command(std::env::current_exe().unwrap())
        .arg("--child")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("CONSOLE_WINDOW=false"));
}

#[cfg(not(windows))]
fn main() {}
