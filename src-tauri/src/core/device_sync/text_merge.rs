use anyhow::{Context, Result};
use git2::{FileFavor, MergeOptions, Repository, Tree};
use sha2::{Digest, Sha256};

pub(super) const MAX_TEXT_BYTES: usize = 1024 * 1024;

pub(super) fn verify_snapshot(
    bytes: Vec<u8>,
    expected: &str,
    allow_git_eol: bool,
) -> Result<Vec<u8>> {
    let matches = |value: &[u8]| hex::encode(Sha256::digest(value)) == expected;
    if matches(&bytes) {
        return Ok(bytes);
    }
    if allow_git_eol && !bytes.contains(&0) {
        if let Ok(text) = std::str::from_utf8(&bytes) {
            let lf = text.replace("\r\n", "\n");
            if matches(lf.as_bytes()) {
                return Ok(lf.into_bytes());
            }
            let crlf = lf.replace('\n', "\r\n");
            if matches(crlf.as_bytes()) {
                return Ok(crlf.into_bytes());
            }
        }
    }
    anyhow::bail!("text merge snapshot hash mismatch")
}

pub(super) fn merge_text(base: &[u8], local: &[u8], remote: &[u8]) -> Result<Option<Vec<u8>>> {
    if [base, local, remote].iter().any(|bytes| {
        bytes.len() > MAX_TEXT_BYTES || bytes.contains(&0) || std::str::from_utf8(bytes).is_err()
    }) {
        return Ok(None);
    }
    let texts = [base, local, remote].map(|bytes| String::from_utf8_lossy(bytes));
    if texts
        .iter()
        .any(|text| text.contains("\r\n") && text.replace("\r\n", "").contains('\n'))
    {
        return Ok(None);
    }
    let local_crlf = texts[1].contains("\r\n");
    let normalized = texts.map(|text| text.replace("\r\n", "\n"));
    let directory = tempfile::tempdir().context("create isolated text merge directory")?;
    let repo =
        Repository::init_bare(directory.path()).context("initialize text merge repository")?;
    // Highest-precedence attributes prevent user/global union drivers from hiding conflicts.
    std::fs::create_dir_all(repo.path().join("info"))?;
    std::fs::write(repo.path().join("info/attributes"), b"content merge=text\n")?;
    let ancestor = text_tree(&repo, normalized[0].as_bytes())?;
    let ours = text_tree(&repo, normalized[1].as_bytes())?;
    let theirs = text_tree(&repo, normalized[2].as_bytes())?;
    let mut options = MergeOptions::new();
    options.find_renames(false).file_favor(FileFavor::Normal);
    let index = repo
        .merge_trees(&ancestor, &ours, &theirs, Some(&options))
        .context("merge Skill text")?;
    if index.has_conflicts() {
        return Ok(None);
    }
    let entry = index
        .get_path(std::path::Path::new("content"), 0)
        .context("merged text missing from index")?;
    let blob = repo.find_blob(entry.id)?;
    let text = std::str::from_utf8(blob.content()).context("invalid merged UTF-8 text")?;
    Ok(Some(if local_crlf {
        text.replace('\n', "\r\n").into_bytes()
    } else {
        blob.content().to_vec()
    }))
}

fn text_tree<'a>(repo: &'a Repository, bytes: &[u8]) -> Result<Tree<'a>> {
    let blob = repo.blob(bytes)?;
    let mut builder = repo.treebuilder(None)?;
    builder.insert("content", blob, 0o100644)?;
    Ok(repo.find_tree(builder.write()?)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_merge_preserves_disjoint_insertions_deletions_and_shared_edits() {
        for (base, local, remote, expected) in [
            (
                "a\nb\nc\nd\ne\n",
                "a\ninserted\nb\nc\nd\ne\n",
                "a\nb\nc\nd\nE\n",
                "a\ninserted\nb\nc\nd\nE\n",
            ),
            (
                "a\nb\nc\nd\ne\n",
                "b\nc\nd\ne\n",
                "a\nb\nc\nd\nE\n",
                "b\nc\nd\nE\n",
            ),
            (
                "a\nb\nc\nd\ne\nf\ng\n",
                "A\nb\nc\nD\ne\nf\ng\n",
                "a\nb\nc\nD\ne\nf\nG\n",
                "A\nb\nc\nD\ne\nf\nG\n",
            ),
            (
                "a\nb\nc\nd\ne\n",
                "\tb  \nb\nc\nd\ne\n",
                "a\nb\nc\nd\nE\n",
                "\tb  \nb\nc\nd\nE\n",
            ),
            ("", "", "", ""),
        ] {
            for (ours, theirs) in [(local, remote), (remote, local)] {
                assert_eq!(
                    merge_text(base.as_bytes(), ours.as_bytes(), theirs.as_bytes())
                        .unwrap()
                        .unwrap(),
                    expected.as_bytes()
                );
            }
        }
    }

    #[test]
    fn text_merge_accepts_size_boundary_and_rejects_conflicts_on_either_side() {
        let boundary = vec![b'a'; MAX_TEXT_BYTES];
        assert_eq!(
            merge_text(&boundary, &boundary, &boundary)
                .unwrap()
                .unwrap(),
            boundary
        );
        for (base, local, remote) in [
            ("a\nb\nc\n", "a\nB\nc\n", "a\nOTHER\nc\n"),
            ("a\nb\nc\n", "a\nc\n", "a\nB\nc\n"),
            ("", "first\n", "second\n"),
        ] {
            for (ours, theirs) in [(local, remote), (remote, local)] {
                assert!(
                    merge_text(base.as_bytes(), ours.as_bytes(), theirs.as_bytes())
                        .unwrap()
                        .is_none()
                );
            }
        }
    }

    #[test]
    fn text_merge_restores_git_eol_only_when_exact_manifest_hash_matches() {
        let crlf = b"one\r\ntwo\r\n";
        let lf = b"one\ntwo\n";
        let crlf_hash = hex::encode(Sha256::digest(crlf));
        let lf_hash = hex::encode(Sha256::digest(lf));
        assert_eq!(
            verify_snapshot(lf.to_vec(), &crlf_hash, true).unwrap(),
            crlf
        );
        assert_eq!(verify_snapshot(crlf.to_vec(), &lf_hash, true).unwrap(), lf);
        assert!(verify_snapshot(lf.to_vec(), &crlf_hash, false).is_err());
        assert!(verify_snapshot(b"changed\ntwo\n".to_vec(), &crlf_hash, true).is_err());
        let mixed_hash = hex::encode(Sha256::digest(b"one\r\ntwo\n"));
        assert!(verify_snapshot(lf.to_vec(), &mixed_hash, true).is_err());
    }

    #[test]
    fn text_merge_uses_local_eol_and_leaves_mixed_eol_for_manual_resolution() {
        let base = b"a\r\nb\r\nc\r\nd\r\ne\r\n";
        let local = b"A\nb\nc\nd\ne\n";
        let remote = b"a\r\nb\r\nc\r\nd\r\nE\r\n";
        assert_eq!(
            merge_text(base, local, remote).unwrap().unwrap(),
            b"A\nb\nc\nd\nE\n"
        );
        assert_eq!(
            merge_text(base, remote, local).unwrap().unwrap(),
            b"A\r\nb\r\nc\r\nd\r\nE\r\n"
        );
        assert!(merge_text(b"a\r\nb\nc\n", b"A\r\nb\nc\n", b"a\r\nb\nC\n")
            .unwrap()
            .is_none());
    }

    #[test]
    fn text_merge_preserves_disjoint_edits_in_utf8_crlf_and_unterminated_text() {
        for (base, local, remote, expected) in [
            (
                "a\nb\nc\nd\ne\n",
                "A\nb\nc\nd\ne\n",
                "a\nb\nc\nd\nE\n",
                "A\nb\nc\nd\nE\n",
            ),
            (
                "甲\n乙\n丙\n丁\n戊\n",
                "本机\n乙\n丙\n丁\n戊\n",
                "甲\n乙\n丙\n丁\n仓库\n",
                "本机\n乙\n丙\n丁\n仓库\n",
            ),
            (
                "a\r\nb\r\nc\r\nd\r\ne\r\n",
                "A\r\nb\r\nc\r\nd\r\ne\r\n",
                "a\r\nb\r\nc\r\nd\r\nE\r\n",
                "A\r\nb\r\nc\r\nd\r\nE\r\n",
            ),
            (
                "a\nb\nc\nd\ne",
                "A\nb\nc\nd\ne",
                "a\nb\nc\nd\nE",
                "A\nb\nc\nd\nE",
            ),
            ("", "added\n", "", "added\n"),
            ("old\n", "same\n", "same\n", "same\n"),
        ] {
            assert_eq!(
                merge_text(base.as_bytes(), local.as_bytes(), remote.as_bytes())
                    .unwrap()
                    .unwrap(),
                expected.as_bytes()
            );
        }
    }

    #[test]
    fn text_merge_reports_overlapping_edits_and_insertions_without_conflict_output() {
        for (base, local, remote) in [
            ("a\nb\nc\n", "a\nLOCAL\nc\n", "a\nREMOTE\nc\n"),
            ("a\nb\n", "a\nLOCAL\nb\n", "a\nREMOTE\nb\n"),
            ("a\nb\nc\n", "a\nc\n", "a\nREMOTE\nc\n"),
        ] {
            assert!(
                merge_text(base.as_bytes(), local.as_bytes(), remote.as_bytes())
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn text_merge_refuses_binary_invalid_utf8_and_large_input_on_every_side() {
        for input in [vec![0, 1], vec![255, 254], vec![b'a'; MAX_TEXT_BYTES + 1]] {
            for index in 0..3 {
                let mut versions: [&[u8]; 3] = [b"a\nb\nc\n", b"A\nb\nc\n", b"a\nb\nC\n"];
                versions[index] = &input;
                assert!(merge_text(versions[0], versions[1], versions[2])
                    .unwrap()
                    .is_none());
            }
        }
    }
}
