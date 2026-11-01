//! Complete staging copies for explicit source-conflict resolution.
use std::path::Path;

use anyhow::Result;

/// Unlike distribution copies, a staging snapshot includes .git and symbolic
/// links. Never follow links, silently omit entries, or discard unsupported data.
pub(crate) fn copy_snapshot(source: &Path, destination: &Path) -> Result<()> {
    super::sync_engine::ensure_paths_do_not_overlap(source, destination)?;
    anyhow::ensure!(!destination.exists(), "snapshot destination already exists");
    let mut directories = vec![];
    for entry in walkdir::WalkDir::new(source).follow_links(false) {
        let entry = entry?;
        let target = destination.join(entry.path().strip_prefix(source)?);
        let kind = entry.file_type();
        if kind.is_dir() {
            std::fs::create_dir_all(&target)?;
            directories.push((target, std::fs::metadata(entry.path())?.permissions()));
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &target)?;
        } else if kind.is_symlink() {
            let link = std::fs::read_link(entry.path())?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(link, target)?;
            #[cfg(windows)]
            {
                use std::os::windows::fs::{symlink_dir, symlink_file, FileTypeExt};
                if kind.is_symlink_dir() {
                    if let Err(error) = symlink_dir(&link, &target) {
                        if !link.is_absolute() {
                            return Err(error.into());
                        }
                        junction::create(&link, &target)?;
                    }
                } else {
                    symlink_file(link, target)?;
                }
            }
        } else {
            anyhow::bail!("cannot snapshot special filesystem entry");
        }
    }
    for (path, permissions) in directories.into_iter().rev() {
        std::fs::set_permissions(path, permissions)?;
    }
    anyhow::ensure!(
        super::content_hash::hash_dir_strict(source)?
            == super::content_hash::hash_dir_strict(destination)?,
        "PLAN_STALE"
    );
    Ok(())
}
