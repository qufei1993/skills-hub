use std::ffi::{OsStr, OsString};
#[cfg(windows)]
use std::fs;
use std::fs::File;
use std::io;
use std::path::{Component, Path, PathBuf};

#[cfg(any(windows, test))]
#[path = "directory/windows_guard.rs"]
mod windows_guard;

fn unsafe_path() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "CLI_BRIDGE_UNSAFE_DIRECTORY",
    )
}

/// File operations remain attached to the directory verified during preparation.
pub(super) struct BridgeDirectory {
    path: PathBuf,
    #[cfg(unix)]
    file: File,
    #[cfg(windows)]
    ancestors: Vec<WindowsAncestor>,
}

#[cfg(windows)]
struct WindowsAncestor {
    handle: File,
    snapshot: windows_guard::Snapshot,
}

#[cfg(windows)]
fn windows_snapshot(handle: &File) -> io::Result<windows_guard::Snapshot> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileAttributeTagInfo, FileIdInfo, GetFileInformationByHandleEx, FILE_ATTRIBUTE_TAG_INFO,
        FILE_ID_INFO,
    };
    let mut attributes = FILE_ATTRIBUTE_TAG_INFO::default();
    let mut identity = FILE_ID_INFO::default();
    // Both initialized output buffers match the selected Windows information classes.
    let attributes_ok = unsafe {
        GetFileInformationByHandleEx(
            handle.as_raw_handle(),
            FileAttributeTagInfo,
            (&mut attributes as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
            std::mem::size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    };
    if attributes_ok == 0 {
        return Err(io::Error::last_os_error());
    }
    let identity_ok = unsafe {
        GetFileInformationByHandleEx(
            handle.as_raw_handle(),
            FileIdInfo,
            (&mut identity as *mut FILE_ID_INFO).cast(),
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    };
    if identity_ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(windows_guard::Snapshot {
        attributes: attributes.FileAttributes,
        reparse_tag: attributes.ReparseTag,
        volume: identity.VolumeSerialNumber,
        file_id: identity.FileId.Identifier,
    })
}

impl BridgeDirectory {
    pub(super) fn open(path: &Path, create: bool) -> io::Result<Self> {
        if !path.is_absolute()
            || path
                .components()
                .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        {
            return Err(unsafe_path());
        }
        Self::open_platform(path, create)
    }

    #[cfg(unix)]
    fn open_platform(path: &Path, create: bool) -> io::Result<Self> {
        use rustix::fs::{mkdirat, open, openat, unlinkat, AtFlags, Mode, OFlags};
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let mut current = File::from(open("/", flags, Mode::empty())?);
        let mut created: Vec<(File, OsString)> = Vec::new();
        let result: io::Result<Self> = (|| {
            for component in path.components() {
                let Component::Normal(name) = component else {
                    continue;
                };
                let next = match openat(&current, name, flags, Mode::empty()) {
                    Ok(fd) => fd,
                    Err(rustix::io::Errno::NOENT) if create => {
                        // Every existing ancestor was opened with O_NOFOLLOW before the first mkdir.
                        let parent = current.try_clone()?;
                        match mkdirat(&parent, name, Mode::from_raw_mode(0o700)) {
                            Ok(()) => created.push((parent, name.to_os_string())),
                            Err(rustix::io::Errno::EXIST) => {}
                            Err(error) => return Err(error.into()),
                        }
                        openat(&current, name, flags, Mode::empty())?
                    }
                    Err(error) => return Err(error.into()),
                };
                current = File::from(next);
            }
            let directory = Self {
                path: path.to_path_buf(),
                file: current,
            };
            if create {
                directory.verify()?;
            }
            Ok(directory)
        })();
        if result.is_err() {
            for (parent, name) in created.into_iter().rev() {
                let _ = unlinkat(&parent, name, AtFlags::REMOVEDIR);
            }
        }
        result
    }

    #[cfg(windows)]
    fn open_platform(path: &Path, create: bool) -> io::Result<Self> {
        use std::os::windows::fs::OpenOptionsExt;
        let mut current = PathBuf::new();
        let mut ancestors: Vec<WindowsAncestor> = Vec::new();
        let mut created = Vec::new();
        let result: io::Result<()> = (|| {
            for component in path.components() {
                current.push(component.as_os_str());
                if matches!(component, Component::Prefix(_)) {
                    continue;
                }
                for ancestor in &ancestors {
                    windows_guard::verify(Some(ancestor.snapshot), || {
                        windows_snapshot(&ancestor.handle)
                    })?;
                }
                // Read sharing only denies both existing and future write/reparse/delete handles.
                let open = || {
                    fs::OpenOptions::new()
                        .read(true)
                        .share_mode(windows_guard::share_mode())
                        .custom_flags(0x00200000 | 0x02000000)
                        .open(&current)
                };
                let handle = match open() {
                    Ok(handle) => handle,
                    Err(error) if create && error.kind() == io::ErrorKind::NotFound => {
                        fs::create_dir(&current)?;
                        created.push((current.clone(), ancestors.len()));
                        open()?
                    }
                    Err(error) => return Err(error),
                };
                let snapshot = windows_guard::verify(None, || windows_snapshot(&handle))?;
                ancestors.push(WindowsAncestor { handle, snapshot });
            }
            Ok(())
        })();
        if let Err(error) = result {
            // Drop only handles for newly created directories; existing ancestors stay pinned.
            while let Some((directory, retained_handles)) = created.pop() {
                ancestors.truncate(retained_handles);
                let _ = fs::remove_dir(directory);
            }
            return Err(error);
        }
        Ok(Self {
            path: path.to_path_buf(),
            ancestors,
        })
    }

    pub(super) fn verify(&self) -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let current = Self::open(&self.path, false)?;
            let expected = self.file.metadata()?;
            let actual = current.file.metadata()?;
            if actual.dev() != expected.dev() || actual.ino() != expected.ino() {
                return Err(unsafe_path());
            }
        }
        #[cfg(windows)]
        for ancestor in &self.ancestors {
            windows_guard::verify(Some(ancestor.snapshot), || {
                windows_snapshot(&ancestor.handle)
            })?;
        }
        Ok(())
    }

    pub(super) fn open_file(
        &self,
        name: &OsStr,
        create: bool,
        exclusive: bool,
    ) -> io::Result<File> {
        self.verify()?;
        #[cfg(unix)]
        let file = {
            use rustix::fs::{openat, Mode, OFlags};
            let mut flags = OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
            flags |= if create {
                OFlags::RDWR | OFlags::CREATE
            } else {
                OFlags::RDONLY
            };
            if exclusive {
                flags |= OFlags::EXCL;
            }
            File::from(openat(&self.file, name, flags, Mode::from_raw_mode(0o600))?)
        };
        #[cfg(windows)]
        let file = {
            use std::os::windows::fs::OpenOptionsExt;
            fs::OpenOptions::new()
                .read(true)
                .write(create)
                .create(create)
                .create_new(exclusive)
                .custom_flags(0x00200000)
                .open(self.path.join(name))?
        };
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(unsafe_path());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(unsafe_path());
            }
        }
        Ok(file)
    }

    pub(super) fn remove(&self, name: &OsStr) -> io::Result<()> {
        self.verify()?;
        self.remove_bound(name)
    }

    fn remove_bound(&self, name: &OsStr) -> io::Result<()> {
        #[cfg(unix)]
        {
            rustix::fs::unlinkat(&self.file, name, rustix::fs::AtFlags::empty()).map_err(Into::into)
        }
        #[cfg(windows)]
        {
            fs::remove_file(self.path.join(name))
        }
    }

    fn replace(&self, from: &OsStr, to: &OsStr) -> io::Result<()> {
        self.verify()?;
        #[cfg(unix)]
        {
            rustix::fs::renameat(&self.file, from, &self.file, to)?;
            self.file.sync_all()?;
        }
        #[cfg(windows)]
        {
            fs::rename(self.path.join(from), self.path.join(to))?;
        }
        Ok(())
    }

    pub(super) fn temp(&self) -> io::Result<BridgeTemp<'_>> {
        let name = OsString::from(format!(".skillshub-cli-{}.tmp", uuid::Uuid::new_v4()));
        let file = self.open_file(&name, true, true)?;
        Ok(BridgeTemp {
            directory: self,
            name,
            file,
            persisted: false,
        })
    }
}

pub(super) struct BridgeTemp<'a> {
    directory: &'a BridgeDirectory,
    name: OsString,
    pub(super) file: File,
    persisted: bool,
}

impl BridgeTemp<'_> {
    pub(super) fn persist(mut self, name: &str) -> io::Result<()> {
        self.directory.replace(&self.name, OsStr::new(name))?;
        self.persisted = true;
        Ok(())
    }
}

impl Drop for BridgeTemp<'_> {
    fn drop(&mut self) {
        if !self.persisted {
            let _ = self.directory.remove_bound(&self.name);
        }
    }
}
