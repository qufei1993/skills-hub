use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

use super::{git_cmd, git_cmd_for_remote};
use crate::core::git_fetcher::{clone_or_pull, clone_or_pull_sparse, clone_or_pull_via_libgit2};

fn commit_file(repo: &git2::Repository, path: &str, content: &[u8], msg: &str) -> git2::Oid {
    let workdir = repo.workdir().expect("workdir");
    let file_path = workdir.join(path);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&file_path, content).unwrap();

    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new(path)).unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();

    let sig = git2::Signature::now("t", "t@example.com").unwrap();
    let parents = match repo.head() {
        Ok(head) => vec![repo.find_commit(head.target().unwrap()).unwrap()],
        Err(_) => vec![],
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, parent_refs.as_slice())
        .unwrap()
}

#[test]
fn clone_then_pull_updates_head() {
    let origin_dir = tempfile::tempdir().unwrap();
    let origin = git2::Repository::init(origin_dir.path()).unwrap();
    let _c1 = commit_file(&origin, "a.txt", b"v1", "c1");
    let c2 = commit_file(&origin, "a.txt", b"v2", "c2");

    let dest_dir = tempfile::tempdir().unwrap();
    let dest = dest_dir.path().join("clone");

    let h1 = clone_or_pull(
        origin_dir.path().to_string_lossy().as_ref(),
        &dest,
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(h1, c2.to_string(), "首次 clone 应指向最新提交");

    let c3 = commit_file(&origin, "b.txt", b"v3", "c3");
    let h2 = clone_or_pull(
        origin_dir.path().to_string_lossy().as_ref(),
        &dest,
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(h2, c3.to_string(), "再次调用应更新到最新提交");
}

#[test]
fn sparse_clone_only_materializes_requested_subpath() {
    let origin_dir = tempfile::tempdir().unwrap();
    let origin = git2::Repository::init(origin_dir.path()).unwrap();
    let _ = commit_file(&origin, "skills/a/SKILL.md", b"---\nname: A\n---\n", "c1");
    let _ = commit_file(&origin, "skills/b/SKILL.md", b"---\nname: B\n---\n", "c2");

    let dest_dir = tempfile::tempdir().unwrap();
    let dest = dest_dir.path().join("clone");

    let head = match clone_or_pull_sparse(
        origin_dir.path().to_string_lossy().as_ref(),
        &dest,
        None,
        "skills/a",
        None,
        None,
    ) {
        Ok(head) => head,
        Err(err) if format!("{:#}", err).contains("system git is required") => return,
        Err(err) => panic!("sparse clone failed: {:#}", err),
    };

    assert!(!head.is_empty());
    assert!(dest.join("skills/a/SKILL.md").exists());
    assert!(
        !dest.join("skills/b/SKILL.md").exists(),
        "未请求的子目录不应被检出到工作区"
    );
}

#[test]
fn git_command_injects_configured_proxy() {
    let cmd = git_cmd(Some("http://127.0.0.1:7890"));
    let args = cmd
        .get_args()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    assert!(args.contains(&"http.proxy=http://127.0.0.1:7890".to_string()));
    assert!(args.contains(&"https.proxy=http://127.0.0.1:7890".to_string()));
    assert_eq!(
        cmd.get_envs()
            .find(|(key, _)| key.to_string_lossy() == "https_proxy")
            .and_then(|(_, value)| value)
            .map(|value| value.to_string_lossy().to_string()),
        Some("http://127.0.0.1:7890".to_string())
    );
}

#[test]
fn git_command_clears_inherited_proxy_when_application_proxy_is_disabled() {
    let cmd = git_cmd(None);
    let args = cmd
        .get_args()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    assert!(args.contains(&"http.proxy=".to_string()));
    assert!(args.contains(&"https.proxy=".to_string()));
    for name in [
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
    ] {
        assert!(
            cmd.get_envs()
                .any(|(key, value)| key == name && value.is_none()),
            "{name} should be removed"
        );
    }
}

#[test]
fn application_proxy_overrides_url_specific_global_git_proxy() {
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().join("gitconfig");
    std::fs::write(
        &config_path,
        "[http \"https://github.com\"]\n\tproxy = http://127.0.0.1:4567\n",
    )
    .unwrap();
    let remote_url = "https://github.com/example/repo.git";

    for expected in [Some("http://127.0.0.1:7890"), None] {
        let mut cmd = git_cmd_for_remote(expected, Some(remote_url));
        let output = cmd
            .env("GIT_CONFIG_GLOBAL", &config_path)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args(["config", "--get-urlmatch", "http.proxy", remote_url])
            .output()
            .unwrap();

        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            expected.unwrap_or_default()
        );
    }
}

#[test]
fn libgit2_fallback_uses_the_explicit_application_proxy() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (request_tx, request_rx) = mpsc::channel();
    let proxy = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut buffer = [0_u8; 4096];
        let read = stream.read(&mut buffer).unwrap();
        request_tx
            .send(String::from_utf8_lossy(&buffer[..read]).into_owned())
            .unwrap();
        stream
            .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
            .unwrap();
    });
    let destination = tempfile::tempdir().unwrap().path().join("clone");

    let result = clone_or_pull_via_libgit2(
        "https://127.0.0.1:9/example/repo.git",
        &destination,
        None,
        &format!("http://{address}"),
    );

    assert!(result.is_err());
    let request = request_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    proxy.join().unwrap();
    assert!(request.starts_with("CONNECT 127.0.0.1:9 HTTP/1.1"));
}

#[test]
fn pinned_commit_install_uses_requested_revision_instead_of_branch_head() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let repo = git2::Repository::init(&source).unwrap();
    let first = commit_file(&repo, "SKILL.md", b"first", "first");
    commit_file(&repo, "SKILL.md", b"second", "second");
    let dest = temp.path().join("cache");
    let revision = first.to_string();
    let result =
        clone_or_pull(source.to_str().unwrap(), &dest, Some(&revision), None, None).unwrap();
    assert_eq!(result, revision);
    assert_eq!(fs::read_to_string(dest.join("SKILL.md")).unwrap(), "first");
    assert!(clone_or_pull(
        source.to_str().unwrap(),
        &dest,
        Some(&"b".repeat(40)),
        None,
        None
    )
    .is_err());
}

#[test]
fn pinned_install_without_system_git_does_not_use_the_default_head() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let repo = git2::Repository::init(&source).unwrap();
    let first = commit_file(&repo, "SKILL.md", b"first", "first");
    commit_file(&repo, "SKILL.md", b"second", "second");
    let dest = temp.path().join("cache");
    assert_eq!(
        super::clone_pinned_commit_via_libgit2(
            source.to_str().unwrap(),
            &dest,
            &first.to_string(),
            ""
        )
        .unwrap(),
        first.to_string()
    );
    assert_eq!(fs::read_to_string(dest.join("SKILL.md")).unwrap(), "first");
}

#[test]
fn issue_182_sparse_checkout_accepts_windows_paths_on_clone_and_update() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let repo = git2::Repository::init(&source).unwrap();
    commit_file(&repo, "skills/a/SKILL.md", b"first", "first");
    commit_file(&repo, "skills/b/SKILL.md", b"other", "other");
    let dest = temp.path().join("cache");
    for content in [b"first".as_slice(), b"second".as_slice()] {
        let expected = commit_file(&repo, "skills/a/SKILL.md", content, "update");
        let head = clone_or_pull_sparse(
            source.to_str().unwrap(),
            &dest,
            None,
            r"skills\a",
            None,
            None,
        )
        .unwrap();
        assert_eq!(head, expected.to_string());
        assert_eq!(fs::read(dest.join("skills/a/SKILL.md")).unwrap(), content);
        assert!(!dest.join("skills/b/SKILL.md").exists());
    }
}
