const PREFIX: &str = "DEVICE_SYNC_FAILURE_";
const KINDS: &[&str] = &[
    "network",
    "tls",
    "auth",
    "credential",
    "visibility",
    "publicUpload",
    "privateKey",
    "integrity",
    "disk",
    "permission",
    "push",
    "fetch",
    "storage",
    "targetModified",
    "unknown",
];

// Only fixed diagnostic codes may cross the history/IPC boundary; never raw error text.
pub(crate) fn safe_message(raw: &str) -> String {
    if raw
        .strip_prefix(PREFIX)
        .is_some_and(|kind| KINDS.contains(&kind))
    {
        return raw.to_owned();
    }
    if raw.starts_with(PREFIX) {
        return format!("{PREFIX}unknown");
    }
    let lower = raw.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|needle| lower.contains(needle));
    let kind = if has(&["target_modified|", "central_modified|"]) {
        "targetModified"
    } else if has(&["device_sync_public_upload_confirmation"]) {
        "publicUpload"
    } else if has(&["device_sync_visibility_unknown"]) {
        "visibility"
    } else if has(&["private key", "potential private key"]) {
        "privateKey"
    } else if has(&[
        "hash mismatch",
        "unsafe text merge",
        "unsafe local text merge",
        "unsafe sync path",
        "snapshot file missing",
        "decode device sync manifest",
    ]) {
        "integrity"
    } else if has(&[
        "keyring",
        "keychain",
        "credential",
        "device_sync_read_credential_required",
    ]) {
        "credential"
    } else if has(&[
        "authentication",
        "unauthorized",
        "401",
        "403",
        "permission denied (publickey)",
    ]) {
        "auth"
    } else if has(&["certificate", "securetransport", "ssl", "tls"]) {
        "tls"
    } else if has(&[
        "timed out",
        "timeout",
        "resolve host",
        "connection",
        "network",
        "dns",
    ]) {
        "network"
    } else if has(&["no space left", "disk full"]) {
        "disk"
    } else if has(&["permission denied", "access is denied", "read-only file"]) {
        "permission"
    } else if has(&[
        "push device sync",
        "remote rejected",
        "non-fast-forward",
        "not fast-forward",
    ]) {
        "push"
    } else if has(&[
        "fetch device sync",
        "clone device sync",
        "read device sync repository",
        "device_sync_public_read_failed",
    ]) {
        "fetch"
    } else if has(&["database", "sqlite", "replace directory", "copy ", "write "]) {
        "storage"
    } else {
        "unknown"
    };
    format!("{PREFIX}{kind}")
}

pub(crate) fn format_error(error: anyhow::Error) -> String {
    let first = error.to_string();
    if error.chain().count() == 1
        && [
            "DEVICE_SYNC_PUBLIC_UPLOAD_CONFIRMATION",
            "DEVICE_SYNC_VISIBILITY_UNKNOWN",
            "DEVICE_SYNC_READ_CREDENTIAL_REQUIRED",
        ]
        .contains(&first.as_str())
    {
        return first;
    }
    safe_message(&format!("{error:#}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_codes_keep_root_cause_without_exposing_any_input() {
        for (cause, kind) in [
            ("connection timed out", "network"),
            ("certificate verify failed", "tls"),
            ("authentication failed", "auth"),
            ("keyring locked", "credential"),
            ("TARGET_MODIFIED|private/local/path", "targetModified"),
            ("text merge snapshot hash mismatch", "integrity"),
            ("no space left on device", "disk"),
            ("permission denied", "permission"),
            ("remote rejected device sync branch update", "push"),
            ("unrecognized failure", "unknown"),
        ] {
            let error = anyhow::anyhow!(
                "{cause}: https://user:secret@example.org?token=secret\nBearer secret\n<secret>"
            )
            .context("synchronize library");
            assert_eq!(format_error(error), format!("{PREFIX}{kind}"));
        }
        for kind in KINDS {
            let code = format!("{PREFIX}{kind}");
            assert_eq!(safe_message(&code), code);
        }
        assert_eq!(
            safe_message("DEVICE_SYNC_FAILURE_network secret"),
            "DEVICE_SYNC_FAILURE_unknown"
        );
    }
}
