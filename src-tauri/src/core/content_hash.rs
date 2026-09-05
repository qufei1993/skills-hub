use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use walkdir::{DirEntry, WalkDir};

fn is_ignored(entry: &DirEntry) -> bool {
    entry.file_name() == OsStr::new(".git")
}

fn os_str_bytes(value: &OsStr) -> Vec<u8> {
    if let Some(value) = value.to_str() {
        let mut bytes = Vec::with_capacity(value.len() + 1);
        bytes.push(b'u');
        bytes.extend_from_slice(value.as_bytes());
        return bytes;
    }
    non_unicode_os_str_bytes(value)
}

#[cfg(unix)]
fn non_unicode_os_str_bytes(value: &OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    let mut bytes = vec![b'x'];
    bytes.extend_from_slice(value.as_bytes());
    bytes
}

#[cfg(windows)]
fn non_unicode_os_str_bytes(value: &OsStr) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    let mut bytes = vec![b'x'];
    bytes.extend(value.encode_wide().flat_map(u16::to_le_bytes));
    bytes
}

#[cfg(not(any(unix, windows)))]
fn non_unicode_os_str_bytes(value: &OsStr) -> Vec<u8> {
    let mut bytes = vec![b'x'];
    bytes.extend_from_slice(value.to_string_lossy().as_bytes());
    bytes
}

fn path_bytes(path: &Path) -> Vec<u8> {
    let mut encoded = Vec::new();
    for component in path.components() {
        let bytes = os_str_bytes(component.as_os_str());
        encoded.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        encoded.extend_from_slice(&bytes);
    }
    encoded
}

fn update_record_header(hasher: &mut Sha256, kind: u8, relative: &Path) {
    let path = path_bytes(relative);
    hasher.update([kind]);
    hasher.update((path.len() as u64).to_be_bytes());
    hasher.update(path);
}

#[cfg(unix)]
fn update_file_attributes(hasher: &mut Sha256, metadata: &std::fs::Metadata) {
    use std::os::unix::fs::PermissionsExt;
    hasher.update((metadata.permissions().mode() & 0o7777).to_be_bytes());
}

#[cfg(not(unix))]
fn update_file_attributes(hasher: &mut Sha256, metadata: &std::fs::Metadata) {
    hasher.update([u8::from(metadata.permissions().readonly())]);
}

fn hash_dir_with_mode(path: &Path, strict: bool) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut entries: Vec<(PathBuf, DirEntry)> = Vec::new();

    for entry in WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| strict || !is_ignored(entry))
    {
        let entry = entry?;
        if !strict && is_ignored(&entry) {
            continue;
        }

        if !strict && !entry.file_type().is_dir() && !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(path)
            .with_context(|| format!("strip prefix {:?}", entry.path()))?
            .to_path_buf();
        entries.push((relative, entry));
    }

    entries.sort_by(|left, right| path_bytes(&left.0).cmp(&path_bytes(&right.0)));

    for (relative, entry) in entries {
        if entry.file_type().is_dir() {
            update_record_header(&mut hasher, b'd', &relative);
        } else if entry.file_type().is_file() {
            update_record_header(&mut hasher, b'f', &relative);
            let metadata = entry
                .metadata()
                .with_context(|| format!("read metadata {:?}", entry.path()))?;
            update_file_attributes(&mut hasher, &metadata);

            let bytes = std::fs::read(entry.path())
                .with_context(|| format!("read file {:?}", entry.path()))?;
            hasher.update((bytes.len() as u64).to_be_bytes());
            hasher.update(bytes);
        } else if entry.file_type().is_symlink() {
            update_record_header(&mut hasher, b'l', &relative);
            let destination = std::fs::read_link(entry.path())
                .with_context(|| format!("read link {:?}", entry.path()))?;
            let destination = path_bytes(&destination);
            hasher.update((destination.len() as u64).to_be_bytes());
            hasher.update(destination);
        } else {
            update_record_header(&mut hasher, b's', &relative);
        }
    }

    let digest = hasher.finalize();
    Ok(hex::encode(digest))
}

pub fn hash_dir(path: &Path) -> Result<String> {
    hash_dir_with_mode(path, false)
}

pub fn hash_dir_strict(path: &Path) -> Result<String> {
    hash_dir_with_mode(path, true)
}

#[cfg(test)]
#[path = "tests/content_hash.rs"]
mod tests;
