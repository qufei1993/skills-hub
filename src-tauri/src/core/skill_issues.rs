// Persist diagnostic categories only, never raw URLs, tokens or command output.
pub fn safe_code(raw: &str) -> &'static str {
    let text = raw.to_lowercase();
    if text.contains("source path not found") {
        "sourceMissing"
    } else if text.contains("path not found in repo") {
        "repoPathMissing"
    } else if text.contains("target_modified") || text.contains("central_modified") {
        "modified"
    } else if text.contains("unsafe") || text.contains("overlap") {
        "unsafeTarget"
    } else if text.contains("no space") || text.contains("disk full") {
        "disk"
    } else if text.contains("permission denied") || text.contains("read-only") {
        "permission"
    } else if text.contains("authentication") || text.contains("unauthorized") {
        "auth"
    } else if text.contains("network")
        || text.contains("resolve host")
        || text.contains("timed out")
    {
        "network"
    } else {
        "unknown"
    }
}
