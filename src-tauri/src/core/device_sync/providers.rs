use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;
use serde_json::json;

use super::types::{ProviderAccount, ProviderId, RemoteRepository};

pub trait GitProvider: Send + Sync {
    fn validate_token(&self, token: &str) -> Result<ProviderAccount>;
    fn list_repositories(&self, token: &str) -> Result<Vec<RemoteRepository>>;
    fn create_private_repository(&self, token: &str, name: &str) -> Result<RemoteRepository>;
}

pub fn provider(id: ProviderId) -> Box<dyn GitProvider> {
    match id {
        ProviderId::Github => Box::new(ApiProvider::github()),
        ProviderId::Gitlab => Box::new(ApiProvider::gitlab()),
        ProviderId::Gitee => Box::new(ApiProvider::gitee()),
    }
}

#[derive(Clone, Debug)]
pub struct ApiProvider {
    id: ProviderId,
    base_url: String,
    client: Client,
}

impl ApiProvider {
    pub fn github() -> Self {
        Self::new(ProviderId::Github, "https://api.github.com")
    }

    pub fn gitlab() -> Self {
        Self::new(ProviderId::Gitlab, "https://gitlab.com/api/v4")
    }

    pub fn gitee() -> Self {
        Self::new(ProviderId::Gitee, "https://gitee.com/api/v5")
    }

    #[cfg(test)]
    pub fn with_base_url(id: ProviderId, base_url: impl Into<String>) -> Self {
        Self::new(id, &base_url.into())
    }

    fn new(id: ProviderId, base_url: &str) -> Self {
        Self {
            id,
            base_url: base_url.trim_end_matches('/').to_string(),
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .expect("build provider HTTP client"),
        }
    }

    fn headers(&self, token: &str) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("Skills-Hub"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        match self.id {
            ProviderId::Github => {
                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {token}"))?,
                );
            }
            ProviderId::Gitlab => {
                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {token}"))?,
                );
            }
            ProviderId::Gitee => {
                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {token}"))?,
                );
            }
        }
        Ok(headers)
    }

    fn checked(response: reqwest::blocking::Response) -> Result<reqwest::blocking::Response> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let body = response.text().unwrap_or_default();
        bail!(
            "provider API returned {}: {}",
            status,
            sanitize_message(&body)
        );
    }
}

#[derive(Deserialize)]
struct AccountResponse {
    login: Option<String>,
    username: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct RepositoryResponse {
    name: String,
    html_url: Option<String>,
    web_url: Option<String>,
    clone_url: Option<String>,
    http_url_to_repo: Option<String>,
    ssh_url: Option<String>,
    ssh_url_to_repo: Option<String>,
    private: Option<bool>,
    visibility: Option<String>,
}

impl GitProvider for ApiProvider {
    fn validate_token(&self, token: &str) -> Result<ProviderAccount> {
        if token.trim().is_empty() {
            bail!("token is empty");
        }
        let response = Self::checked(
            self.client
                .get(format!("{}/user", self.base_url))
                .headers(self.headers(token)?)
                .send()
                .context("validate provider token")?,
        )?;
        let account: AccountResponse = response.json().context("decode provider account")?;
        let login = account
            .login
            .or(account.username)
            .context("provider account response missing login")?;
        Ok(ProviderAccount {
            login,
            display_name: account.name,
        })
    }

    fn list_repositories(&self, token: &str) -> Result<Vec<RemoteRepository>> {
        let path = match self.id {
            ProviderId::Github => {
                "/user/repos?visibility=all&affiliation=owner&per_page=100&sort=updated"
            }
            ProviderId::Gitlab => {
                "/projects?membership=true&simple=true&per_page=100&order_by=last_activity_at"
            }
            ProviderId::Gitee => "/user/repos?type=all&per_page=100&sort=updated",
        };
        let response = Self::checked(
            self.client
                .get(format!("{}{}", self.base_url, path))
                .headers(self.headers(token)?)
                .send()
                .context("list provider repositories")?,
        )?;
        let repositories: Vec<RepositoryResponse> =
            response.json().context("decode repository list")?;
        repositories.into_iter().map(normalize_repository).collect()
    }

    fn create_private_repository(&self, token: &str, name: &str) -> Result<RemoteRepository> {
        let name = name.trim();
        if name.is_empty() {
            bail!("repository name is empty");
        }
        let (path, payload) = match self.id {
            ProviderId::Github => (
                "/user/repos",
                json!({"name": name, "private": true, "auto_init": true}),
            ),
            ProviderId::Gitlab => (
                "/projects",
                json!({"name": name, "visibility": "private", "initialize_with_readme": true}),
            ),
            ProviderId::Gitee => (
                "/user/repos",
                json!({"name": name, "private": true, "auto_init": true}),
            ),
        };
        let response = Self::checked(
            self.client
                .post(format!("{}{}", self.base_url, path))
                .headers(self.headers(token)?)
                .json(&payload)
                .send()
                .context("create private sync repository")?,
        )?;
        normalize_repository(response.json().context("decode repository response")?)
    }
}

fn normalize_repository(repo: RepositoryResponse) -> Result<RemoteRepository> {
    Ok(RemoteRepository {
        name: repo.name,
        web_url: repo.html_url.or(repo.web_url).unwrap_or_default(),
        clone_url: repo
            .clone_url
            .or(repo.http_url_to_repo)
            .context("provider response missing HTTPS clone URL")?,
        ssh_url: repo.ssh_url.or(repo.ssh_url_to_repo),
        private: repo.private.unwrap_or_else(|| {
            repo.visibility
                .as_deref()
                .is_some_and(|value| value == "private")
        }),
    })
}

fn sanitize_message(value: &str) -> String {
    let compact = value.replace(['\n', '\r'], " ");
    compact.chars().take(300).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Matcher;

    #[test]
    fn github_validates_account_and_creates_private_repository() {
        let mut server = mockito::Server::new();
        let account = server
            .mock("GET", "/user")
            .match_header("authorization", "Bearer token")
            .with_status(200)
            .with_body(r#"{"login":"may","name":"May"}"#)
            .create();
        let repository = server
            .mock("POST", "/user/repos")
            .match_header("authorization", "Bearer token")
            .match_body(Matcher::PartialJson(json!({"name":"skills-hub-sync","private":true})))
            .with_status(201)
            .with_body(r#"{"name":"skills-hub-sync","html_url":"https://example/repo","clone_url":"https://example/repo.git","ssh_url":"git@example:repo.git","private":true}"#)
            .create();
        let provider = ApiProvider::with_base_url(ProviderId::Github, server.url());
        assert_eq!(provider.validate_token("token").unwrap().login, "may");
        assert!(
            provider
                .create_private_repository("token", "skills-hub-sync")
                .unwrap()
                .private
        );
        account.assert();
        repository.assert();
    }

    #[test]
    fn gitlab_uses_bearer_token_and_normalizes_response() {
        let mut server = mockito::Server::new();
        let repository = server
            .mock("POST", "/projects")
            .match_header("authorization", "Bearer token")
            .with_status(201)
            .with_body(r#"{"name":"sync","web_url":"https://gitlab/repo","http_url_to_repo":"https://gitlab/repo.git","ssh_url_to_repo":"git@gitlab:repo.git","visibility":"private"}"#)
            .create();
        let provider = ApiProvider::with_base_url(ProviderId::Gitlab, server.url());
        let result = provider.create_private_repository("token", "sync").unwrap();
        assert_eq!(result.clone_url, "https://gitlab/repo.git");
        assert!(result.private);
        repository.assert();
    }

    #[test]
    fn lists_and_normalizes_repositories() {
        let mut server = mockito::Server::new();
        let repositories = server
            .mock(
                "GET",
                "/user/repos?visibility=all&affiliation=owner&per_page=100&sort=updated",
            )
            .match_header("authorization", "Bearer token")
            .with_status(200)
            .with_body(r#"[{"name":"sync","html_url":"https://example/repo","clone_url":"https://example/repo.git","private":true}]"#)
            .create();
        let provider = ApiProvider::with_base_url(ProviderId::Github, server.url());
        let result = provider.list_repositories("token").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].clone_url, "https://example/repo.git");
        repositories.assert();
    }

    #[test]
    fn provider_errors_do_not_include_multiline_payloads() {
        let mut server = mockito::Server::new();
        let request = server
            .mock("GET", "/user")
            .with_status(401)
            .with_body("bad\ncredential")
            .create();
        let provider = ApiProvider::with_base_url(ProviderId::Gitee, server.url());
        let error = provider.validate_token("token").unwrap_err().to_string();
        assert!(!error.contains('\n'));
        request.assert();
    }

    #[test]
    fn gitee_creates_private_repository() {
        let mut server = mockito::Server::new();
        let repository = server
            .mock("POST", "/user/repos")
            .match_header("authorization", "Bearer token")
            .with_status(201)
            .with_body(r#"{"name":"sync","html_url":"https://gitee/repo","clone_url":"https://gitee/repo.git","ssh_url":"git@gitee:repo.git","private":true}"#)
            .create();
        let provider = ApiProvider::with_base_url(ProviderId::Gitee, server.url());
        let result = provider.create_private_repository("token", "sync").unwrap();
        assert_eq!(result.clone_url, "https://gitee/repo.git");
        assert!(result.private);
        repository.assert();
    }
}
