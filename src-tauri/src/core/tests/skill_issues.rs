use super::safe_code;

#[test]
fn a_missing_central_folder_has_its_own_issue_kind() {
    // The central folder is what a Skill actually is, so a missing central folder and a
    // missing external source must stay distinguishable in the UI.
    assert_eq!(
        safe_code("central path not found: \"C:\\\\Users\\\\me\\\\.skillshub\\\\skill\""),
        "centralMissing"
    );
    assert_eq!(
        safe_code("source path not found: \"/tmp/skill\""),
        "sourceMissing"
    );
}

#[test]
fn issue_kinds_never_carry_raw_details() {
    for raw in [
        "central path not found: https://token:secret@example.com",
        "git clone failed for https://user:super-secret@example.com/repo.git",
        "network error contacting https://example.com?access_token=super-secret",
    ] {
        let code = safe_code(raw);
        for forbidden in ["secret", "token", "http", "/", "\\", ":"] {
            assert!(
                !code.contains(forbidden),
                "issue kind {code} must be a category without raw details"
            );
        }
    }
}
