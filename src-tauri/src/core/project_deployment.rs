use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use rustix::fs::{
    mkdirat, openat, readlinkat, renameat_with, statat, symlinkat, unlinkat, AtFlags, Dir,
    FileType, Mode, OFlags, RenameFlags,
};
use uuid::Uuid;

use super::{DeploymentParentIdentity, DeploymentParentSnapshot, SyncMode};
use crate::core::content_hash::{hash_open_dir, hash_open_dir_with_root_link};

// Display paths are never mutation capabilities: every project write stays relative
// to an opened, identity-checked directory, including rollback after path redirection.
fn open_child(parent: &File, name: &OsStr) -> Result<File> {
    Ok(openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?
    .into())
}

fn names(directory: &File) -> Result<Vec<OsString>> {
    Ok(Dir::read_from(directory)?
        .map(|entry| entry.map(|entry| OsString::from_vec(entry.file_name().to_bytes().to_vec())))
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|name| name != "." && name != "..")
        .collect())
}

fn kind(parent: &File, name: &OsStr) -> Result<Option<FileType>> {
    match statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(metadata) => Ok(Some(FileType::from_raw_mode(metadata.st_mode))),
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn fingerprint(parent: &File, name: &OsStr) -> Result<Option<String>> {
    match kind(parent, name)? {
        None => Ok(None),
        Some(FileType::Directory) => Ok(Some(format!(
            "dir:{}",
            hash_open_dir(&open_child(parent, name)?, false)?
        ))),
        Some(FileType::Symlink) => Ok(Some(format!(
            "link:{}",
            Path::new(OsStr::from_bytes(
                readlinkat(parent, name, Vec::new())?.as_bytes()
            ))
            .display()
        ))),
        _ => anyhow::bail!("target is not a directory"),
    }
}

fn rename(parent: &File, old: &OsStr, new: &OsStr) -> Result<()> {
    renameat_with(parent, old, parent, new, RenameFlags::NOREPLACE).context("PLAN_STALE")?;
    Ok(())
}

fn remove(parent: &File, name: &OsStr) -> Result<()> {
    match kind(parent, name)? {
        Some(FileType::Directory) => {
            let child = open_child(parent, name)?;
            for entry in names(&child)? {
                remove(&child, &entry)?;
            }
            unlinkat(parent, name, AtFlags::REMOVEDIR)?;
        }
        Some(_) => unlinkat(parent, name, AtFlags::empty())?,
        None => {}
    }
    Ok(())
}

fn copy_contents(source: &Path, target: &File) -> Result<()> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            mkdirat(target, name.as_os_str(), Mode::from_raw_mode(0o755))?;
            copy_contents(&entry.path(), &open_child(target, &name)?)?;
        } else if file_type.is_file() {
            let mut input = File::open(entry.path())?;
            let mut output = File::from(openat(
                target,
                name.as_os_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )?);
            std::io::copy(&mut input, &mut output)?;
            output.set_permissions(input.metadata()?.permissions())?;
        }
    }
    Ok(())
}

struct CreatedDirectory {
    parent: File,
    name: OsString,
    identity: (u64, u64),
}

fn clean_created(created: &mut Vec<CreatedDirectory>) {
    for entry in created.iter().rev() {
        if let Ok(metadata) = statat(
            &entry.parent,
            entry.name.as_os_str(),
            AtFlags::SYMLINK_NOFOLLOW,
        ) {
            #[allow(clippy::unnecessary_cast)] // dev_t differs between macOS and Linux.
            let identity = (metadata.st_dev as u64, metadata.st_ino as u64);
            if identity == entry.identity {
                let _ = unlinkat(&entry.parent, entry.name.as_os_str(), AtFlags::REMOVEDIR);
            }
        }
    }
    created.clear();
}

fn open_parent(
    snapshot: &mut DeploymentParentSnapshot,
    created: &mut Vec<CreatedDirectory>,
) -> Result<File> {
    let mut directory = open_child(&File::open("/")?, OsStr::new("."))?;
    let mut path = PathBuf::from("/");
    for component in snapshot.physical_parent.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        path.push(name);
        let expected = snapshot
            .identities
            .iter()
            .find(|expected| expected.canonical == path)
            .map(|expected| expected.identity);
        let next = match open_child(&directory, name) {
            Ok(next) => next,
            Err(error)
                if error.downcast_ref::<rustix::io::Errno>() == Some(&rustix::io::Errno::NOENT) =>
            {
                anyhow::ensure!(expected.is_none(), "PLAN_STALE");
                mkdirat(&directory, name, Mode::from_raw_mode(0o755))?;
                let child = open_child(&directory, name)?;
                let metadata = child.metadata()?;
                created.push(CreatedDirectory {
                    parent: directory.try_clone()?,
                    name: name.to_os_string(),
                    identity: (metadata.dev(), metadata.ino()),
                });
                child
            }
            Err(error) => return Err(error.context("PLAN_STALE")),
        };
        let metadata = next.metadata()?;
        let identity = (metadata.dev(), metadata.ino());
        if let Some(expected) = expected {
            anyhow::ensure!(identity == expected, "PLAN_STALE");
        } else {
            snapshot.identities.push(DeploymentParentIdentity {
                path: path.clone(),
                canonical: path.clone(),
                identity,
                symlink: false,
            });
        }
        directory = next;
    }
    Ok(directory)
}

pub(super) struct ProjectDeployment {
    parent: File,
    target: OsString,
    target_path: PathBuf,
    snapshot: DeploymentParentSnapshot,
    staging: Option<OsString>,
    backup: Option<OsString>,
    backup_path: Option<PathBuf>,
    expected: Option<String>,
    prepared: Option<String>,
    content_directory: Option<File>,
    content_root_link: Option<PathBuf>,
    activated: bool,
    committed: bool,
    created: Vec<CreatedDirectory>,
    pub(super) mode: SyncMode,
}

impl ProjectDeployment {
    pub(super) fn prepare(
        source: Option<&Path>,
        target: &Path,
        mode: SyncMode,
        expected: Option<String>,
        mut snapshot: DeploymentParentSnapshot,
    ) -> Result<Self> {
        snapshot.validate()?;
        let mut created = Vec::new();
        let parent = match open_parent(&mut snapshot, &mut created) {
            Ok(parent) => parent,
            Err(error) => {
                clean_created(&mut created);
                return Err(error);
            }
        };
        let mut value = Self {
            parent,
            target: target
                .file_name()
                .context("target has no name")?
                .to_os_string(),
            target_path: target.to_path_buf(),
            snapshot,
            staging: None,
            backup: None,
            backup_path: None,
            expected,
            prepared: None,
            content_directory: None,
            content_root_link: None,
            activated: false,
            committed: false,
            created,
            mode,
        };
        value.snapshot.validate()?;
        if let Some(source) = source {
            let staging = OsString::from(format!(".skills-hub-deploy-{}", Uuid::new_v4()));
            value.staging = Some(staging.clone());
            #[cfg(test)]
            super::run_deployment_race_hook(super::DeploymentRacePoint::StagingWrite);
            match mode {
                SyncMode::Auto | SyncMode::Symlink => {
                    match symlinkat(source, &value.parent, staging.as_os_str()) {
                        Ok(()) => value.mode = SyncMode::Symlink,
                        Err(_) if mode == SyncMode::Auto => value.mode = SyncMode::Copy,
                        Err(error) => return Err(error.into()),
                    }
                }
                SyncMode::Copy => {}
                SyncMode::Junction => anyhow::bail!("junction not supported on this platform"),
            }
            if value.mode == SyncMode::Copy {
                mkdirat(
                    &value.parent,
                    staging.as_os_str(),
                    Mode::from_raw_mode(0o755),
                )?;
                copy_contents(source, &open_child(&value.parent, &staging)?)?;
            }
            // Keep the materialized directory (or our link's destination) open across
            // activation; persisted baselines must never reopen the display path.
            value.content_directory = Some(if value.mode == SyncMode::Copy {
                open_child(&value.parent, &staging)?
            } else {
                value.content_root_link = Some(source.to_path_buf());
                File::from(openat(
                    &value.parent,
                    staging.as_os_str(),
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                    Mode::empty(),
                )?)
            });
            value.prepared = fingerprint(&value.parent, &staging)?;
            value.snapshot.validate()?;
        }
        Ok(value)
    }

    pub(super) fn activate(&mut self) -> Result<()> {
        self.snapshot.validate()?;
        anyhow::ensure!(
            fingerprint(&self.parent, &self.target)? == self.expected,
            "PLAN_STALE"
        );
        if self.expected.is_some() {
            let backup = OsString::from(format!(".skills-hub-backup-{}", Uuid::new_v4()));
            rename(&self.parent, &self.target, &backup)?;
            self.backup_path = Some(self.snapshot.parent.join(&backup));
            self.backup = Some(backup);
            self.verify_backup()?;
        }
        if let Some(staging) = &self.staging {
            self.snapshot.validate()?;
            #[cfg(test)]
            super::run_deployment_race_hook(super::DeploymentRacePoint::ActivationRename);
            rename(&self.parent, staging, &self.target)?;
            self.staging = None;
        }
        self.activated = true;
        self.snapshot.validate()
    }

    pub(super) fn verify_backup(&self) -> Result<()> {
        self.snapshot.validate()?;
        if let Some(backup) = &self.backup {
            anyhow::ensure!(
                fingerprint(&self.parent, backup)? == self.expected,
                "PLAN_STALE"
            );
        }
        Ok(())
    }

    pub(super) fn verify_unchanged(&self) -> Result<()> {
        self.verify_backup()?;
        if self.activated {
            anyhow::ensure!(
                fingerprint(&self.parent, &self.target)? == self.prepared,
                "PLAN_STALE"
            );
        }
        Ok(())
    }

    pub(super) fn content_baseline(&self) -> Result<String> {
        anyhow::ensure!(self.activated, "deployment is not activated");
        hash_open_dir_with_root_link(
            self.content_directory
                .as_ref()
                .context("deployment has no content")?,
            true,
            self.content_root_link.as_deref(),
        )
    }

    pub(super) fn rollback(&mut self) -> Result<()> {
        if self.committed {
            return Ok(());
        }
        if self.activated {
            if kind(&self.parent, &self.target)?.is_some() {
                let recovery = OsString::from(format!(".skills-hub-recovery-{}", Uuid::new_v4()));
                rename(&self.parent, &self.target, &recovery)?;
                if fingerprint(&self.parent, &recovery)? != self.prepared {
                    rename(&self.parent, &recovery, &self.target)?;
                    anyhow::bail!("ROLLBACK_CONFLICT|{}", self.target_path.display());
                }
                remove(&self.parent, &recovery)?;
            }
            self.activated = false;
        }
        if let Some(backup) = &self.backup {
            rename(&self.parent, backup, &self.target)?;
            self.backup = None;
            self.backup_path = None;
        }
        if let Some(staging) = &self.staging {
            remove(&self.parent, staging)?;
            self.staging = None;
        }
        clean_created(&mut self.created);
        Ok(())
    }

    pub(super) fn commit(&mut self) -> Result<()> {
        self.commit_with_recycler(recycle)
    }

    fn commit_with_recycler(
        &mut self,
        recycle: impl FnOnce(&File, &OsStr, &Path) -> Result<()>,
    ) -> Result<()> {
        self.committed = true;
        if self.prepared.is_none() && self.expected.is_none() {
            clean_created(&mut self.created);
        }
        self.created.clear();
        if let Some(backup) = self.backup.as_ref() {
            anyhow::ensure!(
                fingerprint(&self.parent, backup)? == self.expected,
                "backup changed; retained for recovery"
            );
            if kind(&self.parent, backup)? == Some(FileType::Symlink) {
                unlinkat(&self.parent, backup.as_os_str(), AtFlags::empty())?;
            } else {
                recycle(&self.parent, backup, &self.target_path)?;
            }
            self.backup = None;
            self.backup_path = None;
        }
        Ok(())
    }

    pub(super) fn backup_path(&self) -> Option<&Path> {
        self.backup_path.as_deref()
    }
}

impl Drop for ProjectDeployment {
    fn drop(&mut self) {
        if let Err(error) = self.rollback() {
            log::error!(
                "project deployment recovery retained at {}: {error:#}",
                self.target_path.display()
            );
        }
    }
}

fn open_absolute(path: &Path, create: bool) -> Result<File> {
    anyhow::ensure!(path.is_absolute(), "trash directory must be absolute");
    let mut directory = File::open("/")?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                if create && kind(&directory, name)?.is_none() {
                    mkdirat(&directory, name, Mode::from_raw_mode(0o700))?;
                }
                directory = open_child(&directory, name)?;
            }
            _ => anyhow::bail!("invalid trash directory"),
        }
    }
    Ok(directory)
}

#[cfg(test)]
fn recycle(parent: &File, name: &OsStr, _original: &Path) -> Result<()> {
    let trash = tempfile::tempdir()?;
    let directory = open_absolute(&std::fs::canonicalize(trash.path())?, false)?;
    renameat_with(parent, name, &directory, name, RenameFlags::NOREPLACE)?;
    Ok(())
}

#[cfg(all(not(test), target_os = "macos"))]
fn recycle(parent: &File, name: &OsStr, original: &Path) -> Result<()> {
    use trash::macos::{DeleteMethod, TrashContextExtMacos};

    recycle_via_staging(parent, name, original, |path| {
        let mut context = trash::TrashContext::default();
        context.set_delete_method(DeleteMethod::NsFileManager);
        context.delete(path).map_err(anyhow::Error::from)
    })
}

#[cfg(any(test, target_os = "macos"))]
fn recycle_via_staging(
    parent: &File,
    name: &OsStr,
    original: &Path,
    recycle: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    // macOS protects direct access to ~/.Trash. Transfer ownership through the
    // opened project directory before passing a private path to the system API.
    let temporary = tempfile::Builder::new()
        .prefix("skills-hub-recycle-")
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir()?;
    let staging_path = std::fs::canonicalize(temporary.path())?;
    let staging = open_absolute(&staging_path, false)?;
    let destination = format!(
        "{}-{}",
        original.file_name().unwrap_or_default().to_string_lossy(),
        Uuid::new_v4()
    );
    // A failed recycle or rollback must never recursively delete this backup.
    let _retained = temporary.keep();
    if let Err(error) = renameat_with(parent, name, &staging, &destination, RenameFlags::NOREPLACE)
    {
        let _ = std::fs::remove_dir(&staging_path);
        return Err(error.into());
    }
    let staged_backup = staging_path.join(&destination);
    let result = recycle(&staged_backup).and_then(|()| {
        anyhow::ensure!(
            kind(&staging, OsStr::new(&destination))?.is_none(),
            "system recycler left the staged backup in place"
        );
        Ok(())
    });
    match result {
        Ok(()) => {
            let _ = std::fs::remove_dir(&staging_path);
            Ok(())
        }
        Err(error) => {
            if kind(&staging, OsStr::new(&destination))?.is_none() {
                let _ = std::fs::remove_dir(&staging_path);
                return Err(error.context(
                    "system recycler reported failure after moving the backup; outcome unknown",
                ));
            }
            match renameat_with(&staging, &destination, parent, name, RenameFlags::NOREPLACE) {
                Ok(()) => {
                    let _ = std::fs::remove_dir(&staging_path);
                    Err(error.context("system recycle failed; backup restored"))
                }
                Err(restore_error) => Err(error.context(format!(
                    "system recycle failed; backup recovery retained at {} ({restore_error})",
                    staged_backup.display()
                ))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_recycling_uses_the_open_parent_after_project_path_redirection() {
        let project = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let trash = tempfile::tempdir().unwrap();
        let parent_path = project.path().join("skills");
        std::fs::create_dir(&parent_path).unwrap();
        let backup = OsStr::new(".skills-hub-backup-owned");
        std::fs::create_dir(parent_path.join(backup)).unwrap();
        std::fs::write(parent_path.join(backup).join("original.txt"), "recoverable").unwrap();
        let parent = File::open(&parent_path).unwrap();
        let moved = project.path().join("moved-skills");
        std::fs::rename(&parent_path, &moved).unwrap();
        std::os::unix::fs::symlink(outside.path(), &parent_path).unwrap();
        std::fs::create_dir(outside.path().join(backup)).unwrap();
        std::fs::write(outside.path().join(backup).join("trap"), "untouched").unwrap();
        let mut staged_path = PathBuf::new();
        recycle_via_staging(&parent, backup, &parent_path.join("demo"), |path| {
            staged_path = path.to_path_buf();
            assert!(!path.starts_with(project.path()));
            assert_eq!(
                std::fs::metadata(path.parent().unwrap()).unwrap().mode() & 0o777,
                0o700
            );
            std::fs::rename(path, trash.path().join("recycled"))?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(trash.path().join("recycled/original.txt")).unwrap(),
            "recoverable"
        );
        assert!(!moved.join(backup).exists());
        assert!(!staged_path.parent().unwrap().exists());
        assert_eq!(
            std::fs::read_to_string(outside.path().join(backup).join("trap")).unwrap(),
            "untouched"
        );
    }

    #[test]
    fn failed_staged_recycling_restores_backup_or_retains_recovery_without_deleting_content() {
        for block_restore in [false, true] {
            let project = tempfile::tempdir().unwrap();
            let backup = OsStr::new(".skills-hub-backup-owned");
            let original_backup = project.path().join(backup);
            std::fs::create_dir(&original_backup).unwrap();
            std::fs::write(original_backup.join("original.txt"), "recoverable").unwrap();
            let parent = File::open(project.path()).unwrap();
            let mut staged_path = PathBuf::new();
            let error =
                recycle_via_staging(&parent, backup, &project.path().join("demo"), |path| {
                    staged_path = path.to_path_buf();
                    if block_restore {
                        std::fs::create_dir(&original_backup)?;
                        std::fs::write(original_backup.join("trap"), "new content")?;
                    }
                    anyhow::bail!("simulated system recycler failure")
                })
                .unwrap_err();
            assert!(format!("{error:#}").contains("simulated system recycler failure"));
            if block_restore {
                assert!(error
                    .to_string()
                    .contains(&staged_path.display().to_string()));
                assert_eq!(
                    std::fs::read_to_string(staged_path.join("original.txt")).unwrap(),
                    "recoverable"
                );
                assert_eq!(
                    std::fs::read_to_string(original_backup.join("trap")).unwrap(),
                    "new content"
                );
                // Only this test owns the deliberately retained recovery directory.
                std::fs::remove_dir_all(staged_path.parent().unwrap()).unwrap();
            } else {
                assert!(error.to_string().contains("backup restored"));
                assert_eq!(
                    std::fs::read_to_string(original_backup.join("original.txt")).unwrap(),
                    "recoverable"
                );
                assert!(!staged_path.parent().unwrap().exists());
            }
        }
    }

    #[test]
    fn missing_undeploy_does_not_leave_created_parent_directories() {
        let project = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(project.path()).unwrap();
        let target = root.join(".agents/skills/demo");
        let snapshot = DeploymentParentSnapshot::capture(&target, Some(&root)).unwrap();
        let mut staged =
            ProjectDeployment::prepare(None, &target, SyncMode::Copy, None, snapshot).unwrap();
        staged.activate().unwrap();
        staged.commit().unwrap();
        assert!(!root.join(".agents").exists());
    }

    #[test]
    fn redirected_project_parent_cannot_redirect_rollback_or_recycling() {
        for commit in [false, true] {
            for undeploy in [false, true] {
                let project = tempfile::tempdir().unwrap();
                let outside = tempfile::tempdir().unwrap();
                let trash = tempfile::tempdir().unwrap();
                let root = std::fs::canonicalize(project.path()).unwrap();
                let parent = root.join(".agents/skills");
                let target = parent.join("demo");
                let source = root.join("source");
                std::fs::create_dir_all(&target).unwrap();
                std::fs::create_dir(&source).unwrap();
                std::fs::write(target.join("original.txt"), "recoverable").unwrap();
                std::fs::write(source.join("SKILL.md"), "replacement").unwrap();
                let snapshot = DeploymentParentSnapshot::capture(&target, Some(&root)).unwrap();
                let expected = super::super::deployment_fingerprint(&target).unwrap();
                let mut staged = ProjectDeployment::prepare(
                    (!undeploy).then_some(source.as_path()),
                    &target,
                    SyncMode::Copy,
                    expected,
                    snapshot,
                )
                .unwrap();
                staged.activate().unwrap();
                let backup_name = staged.backup.as_ref().unwrap().clone();
                let original = root.join("original-parent");
                std::fs::rename(&parent, &original).unwrap();
                std::os::unix::fs::symlink(outside.path(), &parent).unwrap();
                std::fs::create_dir(outside.path().join(&backup_name)).unwrap();
                std::fs::write(
                    outside.path().join(&backup_name).join("trap"),
                    "backup trap",
                )
                .unwrap();
                std::fs::create_dir(outside.path().join("demo")).unwrap();
                std::fs::write(outside.path().join("demo/trap"), "target trap").unwrap();
                assert!(staged
                    .verify_unchanged()
                    .unwrap_err()
                    .to_string()
                    .contains("PLAN_STALE"));
                if commit {
                    let trash_directory = File::open(trash.path()).unwrap();
                    staged
                        .commit_with_recycler(|parent, name, _| {
                            renameat_with(
                                parent,
                                name,
                                &trash_directory,
                                "recycled",
                                RenameFlags::NOREPLACE,
                            )?;
                            Ok(())
                        })
                        .unwrap();
                    assert_eq!(
                        std::fs::read_to_string(trash.path().join("recycled/original.txt"))
                            .unwrap(),
                        "recoverable"
                    );
                    assert_eq!(original.join("demo/SKILL.md").exists(), !undeploy);
                } else {
                    staged.rollback().unwrap();
                    assert_eq!(
                        std::fs::read_to_string(original.join("demo/original.txt")).unwrap(),
                        "recoverable"
                    );
                }
                drop(staged);
                assert!(!original.join(&backup_name).exists());
                assert_eq!(
                    std::fs::read_to_string(outside.path().join(&backup_name).join("trap"))
                        .unwrap(),
                    "backup trap"
                );
                assert_eq!(
                    std::fs::read_to_string(outside.path().join("demo/trap")).unwrap(),
                    "target trap"
                );
                assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 2);
            }
        }
    }
}

#[cfg(all(not(test), target_os = "linux"))]
fn recycle(parent: &File, name: &OsStr, original: &Path) -> Result<()> {
    use std::io::Write;
    let root = dirs::data_local_dir()
        .context("trash data directory unavailable")?
        .join("Trash");
    let files = open_absolute(&root.join("files"), true)?;
    let info = open_absolute(&root.join("info"), true)?;
    let destination = format!(
        "{}-{}",
        original.file_name().unwrap_or_default().to_string_lossy(),
        Uuid::new_v4()
    );
    let info_name = format!("{destination}.trashinfo");
    let mut metadata = File::from(openat(
        &info,
        info_name.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )?);
    write!(
        metadata,
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        urlencoding::encode(&original.to_string_lossy()),
        chrono::Local::now().format("%Y-%m-%dT%H:%M:%S")
    )?;
    if let Err(error) = renameat_with(parent, name, &files, destination, RenameFlags::NOREPLACE) {
        let _ = unlinkat(&info, info_name, AtFlags::empty());
        return Err(error.into());
    }
    Ok(())
}
