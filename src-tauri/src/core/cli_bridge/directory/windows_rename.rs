use std::io;

fn encoded_name(name: &str) -> io::Result<Vec<u16>> {
    let encoded: Vec<u16> = name.encode_utf16().collect();
    if encoded.is_empty()
        || encoded.len() > 255
        || name == "."
        || name == ".."
        || name.ends_with(['.', ' '])
        || name.chars().any(|c| c < ' ' || "\\/:*?\"<>|".contains(c))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid bridge file name",
        ));
    }
    Ok(encoded)
}

#[cfg(windows)]
pub(super) fn rename(file: &std::fs::File, name: &str) -> io::Result<()> {
    use std::{ffi::c_void, os::windows::io::AsRawHandle};

    #[repr(C)]
    struct RenameInformation {
        replace_if_exists: u32,
        root_directory: *mut c_void,
        name_length: u32,
        name: [u16; 256],
    }
    #[repr(C)]
    struct IoStatusBlock {
        status_or_pointer: usize,
        information: usize,
    }
    #[link(name = "ntdll")]
    extern "system" {
        fn NtSetInformationFile(
            file: *mut c_void,
            status: *mut IoStatusBlock,
            information: *const RenameInformation,
            length: u32,
            class: i32,
        ) -> i32;
        fn RtlNtStatusToDosError(status: i32) -> u32;
    }

    let name = encoded_name(name)?;
    let mut information = RenameInformation {
        replace_if_exists: 1,
        root_directory: std::ptr::null_mut(),
        name_length: (name.len() * 2) as u32,
        name: [0; 256],
    };
    information.name[..name.len()].copy_from_slice(&name);
    let mut status = IoStatusBlock {
        status_or_pointer: 0,
        information: 0,
    };
    // FileRenameInformation (10), NULL root and a single name rename within the
    // source directory without reopening our write-protected parent by path.
    // The synchronous file has DELETE access; repr(C) buffers remain alive and aligned.
    let result = unsafe {
        NtSetInformationFile(
            file.as_raw_handle(),
            &mut status,
            &information,
            std::mem::size_of::<RenameInformation>() as u32,
            10,
        )
    };
    if result < 0 {
        // NT calls return NTSTATUS instead of setting the Win32 last-error value.
        return Err(io::Error::from_raw_os_error(
            unsafe { RtlNtStatusToDosError(result) } as i32,
        ));
    }
    Ok(())
}

#[cfg(windows)]
pub(super) fn delete(file: &std::fs::File) -> io::Result<()> {
    use std::{ffi::c_void, os::windows::io::AsRawHandle};
    #[link(name = "kernel32")]
    extern "system" {
        fn SetFileInformationByHandle(
            file: *mut c_void,
            class: i32,
            information: *const u8,
            length: u32,
        ) -> i32;
    }
    // FileDispositionInfo uses a BOOLEAN and marks this DELETE-capable handle for
    // deletion on close, without reopening a path blocked by our sharing mode.
    let delete_file: u8 = 1;
    let result = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            4,
            &delete_file,
            std::mem::size_of::<u8>() as u32,
        )
    };
    if result == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_bridge_windows_rename_accepts_only_single_file_names() {
        for name in [
            "",
            ".",
            "..",
            "../outside",
            "sub/file",
            "sub\\file",
            "C:stream",
            "x\0y",
            "x.",
            "x ",
            "*",
        ] {
            assert!(encoded_name(name).is_err(), "{name:?}");
        }
        assert!(encoded_name(&"x".repeat(256)).is_err());
        assert_eq!(
            encoded_name("skillshub-cli.exe").unwrap(),
            "skillshub-cli.exe".encode_utf16().collect::<Vec<_>>()
        );
    }
}
