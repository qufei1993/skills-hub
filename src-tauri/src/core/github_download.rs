//! Download a GitHub directory via the Contents API, bypassing git clone entirely.
//! This is much faster than cloning large repos when only a subdirectory is needed.

use std::path::Path;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;

use super::cancel_token::CancelToken;
use super::network_proxy::github_http_client_no_redirects;

const GITHUB_API_ORIGIN: &str = "https://api.github.com";

#[derive(Debug, Deserialize)]
struct GithubContent {
    name: String,
    #[serde(rename = "type")]
    content_type: String,
    path: String,
}

pub struct GithubDownloadOptions<'a> {
    pub cancel: Option<&'a CancelToken>,
    pub token: Option<&'a str>,
    pub proxy_url: &'a str,
}

/// Download a directory from a GitHub repo using the Contents API.
///
/// `owner`/`repo`: repository coordinates
/// `branch`: branch or ref (e.g. "main")
/// `path`: directory path within the repo (e.g. "skills/user/foo")
/// `dest`: local directory to write files into (will be created)
pub fn download_github_directory(
    owner: &str,
    repo: &str,
    branch: &str,
    path: &str,
    dest: &Path,
    options: GithubDownloadOptions<'_>,
) -> Result<()> {
    download_github_directory_from_api(
        GITHUB_API_ORIGIN,
        false,
        owner,
        repo,
        branch,
        path,
        dest,
        options,
    )
}

#[allow(clippy::too_many_arguments)]
fn download_github_directory_from_api(
    api_origin: &str,
    allow_http: bool,
    owner: &str,
    repo: &str,
    branch: &str,
    path: &str,
    dest: &Path,
    options: GithubDownloadOptions<'_>,
) -> Result<()> {
    let api_origin = validate_api_origin(api_origin, allow_http)?;
    let client = github_http_client_no_redirects(options.proxy_url, Some(30))?;

    std::fs::create_dir_all(dest).with_context(|| format!("create directory {:?}", dest))?;

    download_dir_recursive(
        &client,
        &api_origin,
        owner,
        repo,
        branch,
        path,
        dest,
        options.cancel,
        options.token,
    )
}

#[allow(clippy::too_many_arguments)]
fn download_dir_recursive(
    client: &Client,
    api_origin: &reqwest::Url,
    owner: &str,
    repo: &str,
    branch: &str,
    path: &str,
    dest: &Path,
    cancel: Option<&CancelToken>,
    token: Option<&str>,
) -> Result<()> {
    if cancel.is_some_and(|c| c.is_cancelled()) {
        anyhow::bail!("CANCELLED|操作已被用户取消。");
    }

    let url = github_contents_url(api_origin, owner, repo, path, branch)?;

    let mut req = client
        .get(url.clone())
        .header("User-Agent", "skills-hub")
        .header("Accept", "application/vnd.github.v3+json");
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }
    let resp = req
        .send()
        .with_context(|| format!("request GitHub contents: {}", url))?;
    let resp = check_github_response(resp, url.as_str())?;

    let items: Vec<GithubContent> = resp
        .json()
        .with_context(|| format!("parse GitHub contents response: {}", url))?;

    for item in items {
        if cancel.is_some_and(|c| c.is_cancelled()) {
            anyhow::bail!("CANCELLED|操作已被用户取消。");
        }

        let local_path = dest.join(&item.name);

        match item.content_type.as_str() {
            "file" => {
                if let Some(parent) = local_path.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("create parent dir {:?}", parent))?;
                }
                let file_url = github_contents_url(api_origin, owner, repo, &item.path, branch)?;
                let mut file_req = client
                    .get(file_url)
                    .header("User-Agent", "skills-hub")
                    .header("Accept", "application/vnd.github.raw+json");
                if let Some(t) = token {
                    file_req = file_req.header("Authorization", format!("Bearer {}", t));
                }
                let file_resp = file_req
                    .send()
                    .with_context(|| format!("download file: {}", item.path))?;
                let file_resp = check_github_response(file_resp, &item.path)?;
                let bytes = file_resp
                    .bytes()
                    .with_context(|| format!("read file bytes: {}", item.path))?;

                std::fs::write(&local_path, &bytes)
                    .with_context(|| format!("write file {:?}", local_path))?;
            }
            "dir" => {
                download_dir_recursive(
                    client,
                    api_origin,
                    owner,
                    repo,
                    branch,
                    &item.path,
                    &local_path,
                    cancel,
                    token,
                )?;
            }
            _ => {
                // Skip symlinks, submodules, etc.
            }
        }
    }

    Ok(())
}

fn validate_api_origin(value: &str, allow_http: bool) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(value).context("parse GitHub API origin")?;
    if (url.scheme() != "https" && !(allow_http && url.scheme() == "http"))
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        anyhow::bail!("GitHub API endpoint must be a trusted origin");
    }
    Ok(url)
}

fn github_contents_url(
    api_origin: &reqwest::Url,
    owner: &str,
    repo: &str,
    path: &str,
    branch: &str,
) -> Result<reqwest::Url> {
    let mut url = api_origin.clone();
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| anyhow::anyhow!("GitHub API origin cannot be a base URL"))?;
        segments.clear().extend(["repos", owner, repo, "contents"]);
        segments.extend(path.split('/').filter(|segment| !segment.is_empty()));
    }
    url.query_pairs_mut().append_pair("ref", branch);
    Ok(url)
}

/// Check a GitHub API response for rate-limit errors and surface a helpful message.
fn check_github_response(
    resp: reqwest::blocking::Response,
    context: &str,
) -> Result<reqwest::blocking::Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    if status.as_u16() == 403 {
        let reset_hint = resp
            .headers()
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok())
            .map(|ts| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                let wait_mins = ((ts - now).max(0) + 59) / 60; // round up
                format!("RATE_LIMITED|{}", wait_mins)
            })
            .unwrap_or_else(|| "403 Forbidden".to_string());
        anyhow::bail!("{}", reset_hint);
    }
    // For other errors, use the standard error_for_status logic.
    Err(anyhow::anyhow!(
        "GitHub API error {} for: {}",
        status,
        context
    ))
}

/// Check if a GitHub URL with subpath can use the fast API download path.
/// Returns Some((owner, repo, branch, subpath)) if applicable.
pub fn parse_github_api_params(
    clone_url: &str,
    branch: Option<&str>,
    subpath: Option<&str>,
) -> Option<(String, String, String, String)> {
    // Only for GitHub URLs with a subpath
    let subpath = subpath?;
    if subpath.is_empty() || subpath == "." {
        return None;
    }

    // Extract owner/repo from clone_url like https://github.com/owner/repo.git
    let url = clone_url.trim_end_matches('/').trim_end_matches(".git");
    let prefix = "https://github.com/";
    if !url.starts_with(prefix) {
        return None;
    }
    let rest = &url[prefix.len()..];
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() < 2 {
        return None;
    }

    Some((
        parts[0].to_string(),
        parts[1].to_string(),
        branch.unwrap_or("main").to_string(),
        subpath.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_github_api_params_extracts_correctly() {
        let result = parse_github_api_params(
            "https://github.com/openclaw/skills.git",
            Some("main"),
            Some("skills/user/foo"),
        );
        assert_eq!(
            result,
            Some((
                "openclaw".to_string(),
                "skills".to_string(),
                "main".to_string(),
                "skills/user/foo".to_string(),
            ))
        );
    }

    #[test]
    fn parse_github_api_params_returns_none_without_subpath() {
        let result =
            parse_github_api_params("https://github.com/openclaw/skills.git", Some("main"), None);
        assert_eq!(result, None);
    }

    #[test]
    fn parse_github_api_params_returns_none_for_root_subpath() {
        let result = parse_github_api_params(
            "https://github.com/openclaw/skills.git",
            Some("main"),
            Some("."),
        );
        assert_eq!(result, None);
    }

    #[test]
    fn parse_github_api_params_returns_none_for_non_github() {
        let result = parse_github_api_params(
            "https://gitlab.com/user/repo.git",
            Some("main"),
            Some("path"),
        );
        assert_eq!(result, None);
    }

    #[test]
    fn authenticated_file_download_ignores_response_download_url_and_uses_api_endpoint() {
        let mut api_server = mockito::Server::new();
        let mut attacker_server = mockito::Server::new();
        let attacker_request = attacker_server.mock("GET", "/stolen").expect(0).create();
        let list_request = api_server
            .mock("GET", "/repos/acme/private/contents/skills/example")
            .match_query(mockito::Matcher::UrlEncoded("ref".into(), "main".into()))
            .match_header("authorization", "Bearer private-pat")
            .match_header("accept", "application/vnd.github.v3+json")
            .with_status(200)
            .with_body(format!(
                r#"[{{"name":"SKILL.md","type":"file","download_url":"{}/stolen","path":"skills/example/SKILL.md"}}]"#,
                attacker_server.url()
            ))
            .create();
        let raw_request = api_server
            .mock(
                "GET",
                "/repos/acme/private/contents/skills/example/SKILL.md",
            )
            .match_query(mockito::Matcher::UrlEncoded("ref".into(), "main".into()))
            .match_header("authorization", "Bearer private-pat")
            .match_header("accept", "application/vnd.github.raw+json")
            .with_status(200)
            .with_body("private skill contents")
            .create();
        let destination = tempfile::tempdir().unwrap();

        download_github_directory_from_api(
            &api_server.url(),
            true,
            "acme",
            "private",
            "main",
            "skills/example",
            destination.path(),
            GithubDownloadOptions {
                cancel: None,
                token: Some("private-pat"),
                proxy_url: "",
            },
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(destination.path().join("SKILL.md")).unwrap(),
            "private skill contents"
        );
        list_request.assert();
        raw_request.assert();
        attacker_request.assert();
    }

    #[test]
    fn authenticated_file_download_does_not_follow_api_redirects() {
        for status in [307, 308] {
            let mut api_server = mockito::Server::new();
            let mut attacker_server = mockito::Server::new();
            let list_request = api_server
                .mock("GET", "/repos/acme/private/contents/skills/example")
                .match_query(mockito::Matcher::UrlEncoded("ref".into(), "main".into()))
                .match_header("authorization", "Bearer private-pat")
                .with_status(200)
                .with_body(
                    r#"[{"name":"SKILL.md","type":"file","path":"skills/example/SKILL.md"}]"#,
                )
                .create();
            let redirect_target = format!("{}/stolen", attacker_server.url());
            let raw_request = api_server
                .mock(
                    "GET",
                    "/repos/acme/private/contents/skills/example/SKILL.md",
                )
                .match_query(mockito::Matcher::UrlEncoded("ref".into(), "main".into()))
                .match_header("authorization", "Bearer private-pat")
                .with_status(status)
                .with_header("location", &redirect_target)
                .create();
            let attacker_request = attacker_server.mock("GET", "/stolen").expect(0).create();
            let destination = tempfile::tempdir().unwrap();

            let result = download_github_directory_from_api(
                &api_server.url(),
                true,
                "acme",
                "private",
                "main",
                "skills/example",
                destination.path(),
                GithubDownloadOptions {
                    cancel: None,
                    token: Some("private-pat"),
                    proxy_url: "",
                },
            );

            assert!(result.is_err(), "accepted HTTP {status} redirect");
            list_request.assert();
            raw_request.assert();
            attacker_request.assert();
        }
    }

    #[test]
    fn check_github_response_passes_success() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/ok")
            .with_status(200)
            .with_body("ok")
            .create();
        let client = Client::new();
        let resp = client.get(format!("{}/ok", server.url())).send().unwrap();
        assert!(check_github_response(resp, "test").is_ok());
    }

    #[test]
    fn check_github_response_extracts_rate_limit_reset() {
        let mut server = mockito::Server::new();
        let reset_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 600; // 10 minutes from now
        let _m = server
            .mock("GET", "/limited")
            .with_status(403)
            .with_header("x-ratelimit-reset", &reset_ts.to_string())
            .with_body("rate limited")
            .create();
        let client = Client::new();
        let resp = client
            .get(format!("{}/limited", server.url()))
            .send()
            .unwrap();
        let err = check_github_response(resp, "test").unwrap_err();
        let msg = format!("{:#}", err);
        assert!(msg.contains("RATE_LIMITED|"), "got: {}", msg);
        // Should contain a number of minutes (around 10)
        let mins: i64 = msg
            .strip_prefix("RATE_LIMITED|")
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!((9..=11).contains(&mins), "expected ~10 mins, got {}", mins);
    }

    #[test]
    fn check_github_response_handles_403_without_reset_header() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/forbidden")
            .with_status(403)
            .with_body("forbidden")
            .create();
        let client = Client::new();
        let resp = client
            .get(format!("{}/forbidden", server.url()))
            .send()
            .unwrap();
        let err = check_github_response(resp, "test").unwrap_err();
        let msg = format!("{:#}", err);
        assert!(msg.contains("403"), "got: {}", msg);
    }

    #[test]
    fn check_github_response_handles_other_errors() {
        let mut server = mockito::Server::new();
        let _m = server
            .mock("GET", "/notfound")
            .with_status(404)
            .with_body("not found")
            .create();
        let client = Client::new();
        let resp = client
            .get(format!("{}/notfound", server.url()))
            .send()
            .unwrap();
        let err = check_github_response(resp, "test").unwrap_err();
        let msg = format!("{:#}", err);
        assert!(msg.contains("404"), "got: {}", msg);
    }
}
