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
fn sparse_many_materializes_the_union_and_drops_paths_from_the_next_round() {
    let origin_dir = tempfile::tempdir().unwrap();
    let origin = git2::Repository::init(origin_dir.path()).unwrap();
    commit_file(&origin, "skills/a/SKILL.md", b"a", "a");
    commit_file(&origin, "skills/b/SKILL.md", b"b", "b");
    let revision = commit_file(&origin, "unrelated/root.asset", b"unrelated", "unrelated");
    let root = tempfile::tempdir().unwrap();
    let dest = root.path().join("checkout");
    let reference = origin_dir.path().to_str().unwrap();
    let head = super::clone_or_pull_sparse_many(
        reference,
        &dest,
        None,
        &["skills/a".into(), "skills/b".into()],
        None,
        None,
    )
    .unwrap();
    assert_eq!(head, revision.to_string());
    assert!(dest.join("skills/a/SKILL.md").is_file());
    assert!(dest.join("skills/b/SKILL.md").is_file());
    assert!(!dest.join("unrelated").exists());
    super::clone_or_pull_sparse_many(reference, &dest, None, &["skills/a".into()], None, None)
        .unwrap();
    assert!(dest.join("skills/a/SKILL.md").is_file());
    assert!(!dest.join("skills/b/SKILL.md").exists());
    let pinned = root.path().join("pinned");
    super::clone_or_pull_sparse_many(
        reference,
        &pinned,
        Some(&revision.to_string()),
        &["skills/b".into()],
        None,
        None,
    )
    .unwrap();
    assert!(pinned.join("skills/b/SKILL.md").is_file());
    assert!(!pinned.join("skills/a").exists());
    assert!(!pinned.join("unrelated").exists());
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

#[test]
fn default_revision_resolution_reads_current_remote_head() {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    let first = commit_file(&repo, "SKILL.md", b"first", "first");
    let reference = dir.path().to_str().unwrap();
    assert_eq!(
        super::resolve_default_revision(reference, None).unwrap(),
        first.to_string()
    );
    let second = commit_file(&repo, "SKILL.md", b"second", "second");
    assert_eq!(
        super::resolve_default_revision(reference, None).unwrap(),
        second.to_string()
    );
}

const PROCESS_FIXTURE_TEST: &str = "core::git_fetcher::tests::git_command_process_fixture";

fn process_fixture_command(root: &std::path::Path, role: &str) -> std::process::Command {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", PROCESS_FIXTURE_TEST, "--nocapture"])
        .env("SKILLS_HUB_GIT_PROCESS_FIXTURE_ROOT", root)
        .env("SKILLS_HUB_GIT_PROCESS_FIXTURE_ROLE", role);
    command
}

#[test]
fn git_command_process_fixture() {
    let Some(root) = std::env::var_os("SKILLS_HUB_GIT_PROCESS_FIXTURE_ROOT") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    let role = std::env::var("SKILLS_HUB_GIT_PROCESS_FIXTURE_ROLE").unwrap();
    if role == "output" {
        // More than a pipe buffer: a command runner must drain output while the
        // command is alive, or use a capture that cannot block the writer.
        std::io::stdout()
            .write_all(&vec![b'o'; 256 * 1024])
            .unwrap();
        std::io::stderr()
            .write_all(&vec![b'e'; 256 * 1024])
            .unwrap();
        return;
    }
    let lock = fs::File::create(root.join(format!("{role}.lock"))).unwrap();
    fs2::FileExt::lock_exclusive(&lock).unwrap();
    if role == "parent" {
        let mut child = process_fixture_command(&root, "descendant")
            .spawn()
            .unwrap();
        // The test parent owns a disposable process. A finite fixture lifetime
        // also bounds the old buggy implementation and cleans up on assertion
        // failure without affecting any other process.
        child.wait().unwrap();
    } else {
        std::io::stderr().write_all(b"descendant-ready\n").unwrap();
        fs::write(root.join("descendant.ready"), b"ready").unwrap();
        std::thread::sleep(Duration::from_secs(6));
    }
}

fn assert_fixture_stopped(root: &std::path::Path) {
    for role in ["parent", "descendant"] {
        let file = fs::File::open(root.join(format!("{role}.lock"))).unwrap();
        // The OS releases the fixture's lock only after its process exits.
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if fs2::FileExt::try_lock_exclusive(&file).is_ok() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "{role} survived termination"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[test]
fn git_timeout_terminates_descendants_holding_output_and_returns_within_deadline() {
    let root = tempfile::tempdir().unwrap();
    let started = std::time::Instant::now();
    let error = super::run_cmd_with_timeout(
        process_fixture_command(root.path(), "parent"),
        Duration::from_secs(2),
        "process-tree timeout fixture".into(),
        None,
    )
    .unwrap_err();
    assert!(
        root.path().join("descendant.ready").exists(),
        "fixture was not ready"
    );
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "timeout waited for a descendant's output handle"
    );
    assert!(error.to_string().contains("git 操作超时（2s）"));
    assert!(error.to_string().contains("descendant-ready"));
    assert_fixture_stopped(root.path());
}

#[test]
fn git_cancellation_terminates_descendants_and_releases_resources() {
    let root = tempfile::tempdir().unwrap();
    let token = crate::core::cancel_token::CancelToken::new();
    std::thread::scope(|scope| {
        let token_ref = &token;
        let root_ref = root.path();
        let ready = scope.spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(4);
            while !root_ref.join("descendant.ready").exists() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "fixture was not ready"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            token_ref.cancel();
            std::time::Instant::now()
        });
        let error = super::run_cmd_with_timeout(
            process_fixture_command(root.path(), "parent"),
            Duration::from_secs(5),
            "process-tree cancellation fixture".into(),
            Some(&token),
        )
        .unwrap_err();
        let cancelled_at = ready.join().unwrap();
        assert!(cancelled_at.elapsed() < Duration::from_secs(2));
        assert!(error.to_string().starts_with("CANCELLED|"));
    });
    assert_fixture_stopped(root.path());
}

#[test]
fn git_command_captures_output_larger_than_pipe_buffers() {
    let root = tempfile::tempdir().unwrap();
    let output = super::run_cmd_with_timeout(
        process_fixture_command(root.path(), "output"),
        Duration::from_secs(3),
        "large command output fixture".into(),
        None,
    )
    .unwrap();
    assert!(output.status.success());
    assert!(output
        .stdout
        .windows(256 * 1024)
        .any(|bytes| bytes.iter().all(|byte| *byte == b'o')));
    assert!(output
        .stderr
        .windows(256 * 1024)
        .any(|bytes| bytes.iter().all(|byte| *byte == b'e')));
}
