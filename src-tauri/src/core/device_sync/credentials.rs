use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::types::{CredentialUsage, ProviderId};

#[cfg(debug_assertions)]
pub(crate) const DEVICE_SYNC_KEYRING_SERVICE: &str = "com.skills-hub.device-sync.dev";
#[cfg(not(debug_assertions))]
pub(crate) const DEVICE_SYNC_KEYRING_SERVICE: &str = "com.skills-hub.device-sync";
pub(crate) const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
pub(crate) const GITLAB_TOKEN_URL: &str = "https://gitlab.com/oauth/token";

pub trait CredentialStore: Send + Sync {
    fn set(&self, key: &str, secret: &str) -> Result<()>;
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn delete(&self, key: &str) -> Result<()>;
}

const CREDENTIAL_PREFIX: &str = "skills-hub-credential-v2:";
const CREDENTIAL_VERSION: u8 = 2;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OAuthCredential {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub token_url: String,
    pub client_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CredentialKind {
    OAuth,
    PersonalAccessToken,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredCredential {
    version: u8,
    kind: CredentialKind,
    provider: ProviderId,
    allowed_origin: String,
    token: StoredToken,
    refresh: Option<TrustedRefreshConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredToken {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TrustedRefreshConfig {
    token_url: String,
    client_id: String,
}

impl CredentialUsage {
    pub fn official(provider: ProviderId) -> Self {
        Self {
            provider,
            origin: format!("{}:443", provider.official_git_host()),
        }
    }

    pub fn from_https_remote(provider: ProviderId, remote_url: &str) -> Result<Self> {
        let remote_url = remote_url.trim();
        let url = reqwest::Url::parse(remote_url).context("parse credential remote URL")?;
        if url.scheme() != "https" {
            bail!("token authentication requires an HTTPS repository URL");
        }
        let authority = remote_url
            .split_once("://")
            .map(|(_, remainder)| remainder)
            .unwrap_or_default()
            .split(['/', '?', '#'])
            .next()
            .unwrap_or_default();
        if authority.is_empty() {
            bail!("repository URL is missing an authority");
        }
        if authority.contains('@') || !url.username().is_empty() || url.password().is_some() {
            bail!("repository URL must not contain user information");
        }
        if url.query().is_some() || url.fragment().is_some() {
            bail!("repository URL must not contain a query or fragment");
        }
        let host = url
            .host_str()
            .context("repository URL is missing a host")?
            .to_ascii_lowercase();
        let port = url
            .port_or_known_default()
            .context("repository URL is missing an effective port")?;
        let normalized_host = if host.contains(':') {
            format!("[{host}]")
        } else {
            host
        };
        Ok(Self {
            provider,
            origin: format!("{normalized_host}:{port}"),
        })
    }
}

pub fn save_oauth_credential(
    store: &dyn CredentialStore,
    key: &str,
    provider: ProviderId,
    credential: &OAuthCredential,
) -> Result<()> {
    save_oauth_credential_with_http_policy(store, key, provider, credential, false)
}

pub(crate) fn save_oauth_credential_with_http_policy(
    store: &dyn CredentialStore,
    key: &str,
    provider: ProviderId,
    credential: &OAuthCredential,
    allow_http: bool,
) -> Result<()> {
    let refresh = match credential.refresh_token.as_ref() {
        Some(_) => {
            validate_refresh_endpoint(provider, &credential.token_url, allow_http)?;
            Some(TrustedRefreshConfig {
                token_url: credential.token_url.clone(),
                client_id: credential.client_id.clone(),
            })
        }
        None => None,
    };
    save_credential(
        store,
        key,
        &StoredCredential {
            version: CREDENTIAL_VERSION,
            kind: CredentialKind::OAuth,
            provider,
            allowed_origin: CredentialUsage::official(provider).origin,
            token: StoredToken {
                access_token: credential.access_token.clone(),
                refresh_token: credential.refresh_token.clone(),
                expires_at: credential.expires_at,
            },
            refresh,
        },
    )
}

pub fn save_personal_access_token(
    store: &dyn CredentialStore,
    key: &str,
    usage: &CredentialUsage,
    access_token: &str,
) -> Result<()> {
    save_credential(
        store,
        key,
        &StoredCredential {
            version: CREDENTIAL_VERSION,
            kind: CredentialKind::PersonalAccessToken,
            provider: usage.provider,
            allowed_origin: usage.origin.clone(),
            token: StoredToken {
                access_token: access_token.to_string(),
                refresh_token: None,
                expires_at: None,
            },
            refresh: None,
        },
    )
}

fn save_credential(
    store: &dyn CredentialStore,
    key: &str,
    credential: &StoredCredential,
) -> Result<()> {
    let payload = format!(
        "{}{}",
        CREDENTIAL_PREFIX,
        serde_json::to_string(credential)?
    );
    store.set(key, &payload)
}

pub fn resolve_access_token(
    store: &dyn CredentialStore,
    key: &str,
    expected_usage: &CredentialUsage,
) -> Result<Option<String>> {
    resolve_access_token_with_refresh_endpoint(store, key, expected_usage, false, |provider| {
        super::oauth::trusted_refresh_endpoint(provider)
    })
}

pub(crate) enum PersonalAccessTokenLookup {
    Missing,
    Invalid,
    Present(String),
}

pub(crate) fn inspect_personal_access_token(
    store: &dyn CredentialStore,
    key: &str,
    expected_usage: &CredentialUsage,
) -> Result<PersonalAccessTokenLookup> {
    let Some(value) = store.get(key)? else {
        return Ok(PersonalAccessTokenLookup::Missing);
    };
    let Ok(credential) = decode_stored_credential(&value) else {
        return Ok(PersonalAccessTokenLookup::Invalid);
    };
    if credential.kind != CredentialKind::PersonalAccessToken
        || credential.provider != expected_usage.provider
        || credential.allowed_origin != expected_usage.origin
        || credential.refresh.is_some()
        || credential.token.refresh_token.is_some()
        || credential.token.expires_at.is_some()
    {
        return Ok(PersonalAccessTokenLookup::Invalid);
    }
    Ok(PersonalAccessTokenLookup::Present(
        credential.token.access_token,
    ))
}

pub fn resolve_personal_access_token(
    store: &dyn CredentialStore,
    key: &str,
    expected_usage: &CredentialUsage,
) -> Result<Option<String>> {
    match inspect_personal_access_token(store, key, expected_usage)? {
        PersonalAccessTokenLookup::Missing => Ok(None),
        PersonalAccessTokenLookup::Present(token) => Ok(Some(token)),
        PersonalAccessTokenLookup::Invalid => {
            bail!("saved credential is not a valid personal access token; enter it again")
        }
    }
}

#[cfg(test)]
pub(crate) fn resolve_access_token_with_trusted_refresh_endpoint(
    store: &dyn CredentialStore,
    key: &str,
    expected_usage: &CredentialUsage,
    trusted_refresh_endpoint: Option<&str>,
) -> Result<Option<String>> {
    let trusted_refresh_endpoint = trusted_refresh_endpoint.map(str::to_string);
    resolve_access_token_with_refresh_endpoint(store, key, expected_usage, true, move |_| {
        Ok(trusted_refresh_endpoint)
    })
}

fn resolve_access_token_with_refresh_endpoint<F>(
    store: &dyn CredentialStore,
    key: &str,
    expected_usage: &CredentialUsage,
    allow_http: bool,
    trusted_refresh_endpoint: F,
) -> Result<Option<String>>
where
    F: FnOnce(ProviderId) -> Result<Option<String>>,
{
    let Some(value) = store.get(key)? else {
        return Ok(None);
    };
    let mut credential = decode_stored_credential(&value)?;
    match credential.kind {
        CredentialKind::OAuth => {}
        CredentialKind::PersonalAccessToken => {
            if credential.refresh.is_some()
                || credential.token.refresh_token.is_some()
                || credential.token.expires_at.is_some()
            {
                bail!("authorization required; malformed personal access token credential");
            }
        }
    }
    if credential.provider != expected_usage.provider
        || credential.allowed_origin != expected_usage.origin
    {
        bail!("authorization required for this provider and repository origin");
    }
    let needs_refresh = credential.kind == CredentialKind::OAuth
        && credential
            .token
            .expires_at
            .is_some_and(|expires_at| expires_at <= now_seconds() + 60);
    if !needs_refresh {
        return Ok(Some(credential.token.access_token));
    }
    let refresh_token = credential
        .token
        .refresh_token
        .clone()
        .context("OAuth authorization expired; sign in again")?;
    let refresh = credential
        .refresh
        .as_ref()
        .context("OAuth authorization expired; sign in again")?;
    let trusted_refresh_endpoint = trusted_refresh_endpoint(credential.provider)?.context(
        "OAuth authorization expired; current trusted refresh endpoint is not configured",
    )?;
    validate_refresh_endpoint(credential.provider, &trusted_refresh_endpoint, allow_http)?;
    validate_refresh_endpoint(credential.provider, &refresh.token_url, allow_http)?;
    if refresh.token_url != trusted_refresh_endpoint {
        bail!(
            "OAuth credential does not match the current trusted refresh endpoint; sign in again"
        );
    }
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("build OAuth refresh client")?;
    let response = client
        .post(&refresh.token_url)
        .header(reqwest::header::ACCEPT, "application/json")
        .form(&[
            ("client_id", refresh.client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.as_str()),
        ])
        .send()
        .context("refresh OAuth authorization")?;
    if !response.status().is_success() {
        bail!("OAuth authorization expired; sign in again");
    }
    let refreshed: RefreshResponse = response.json().context("decode refreshed OAuth token")?;
    credential.token.access_token = refreshed.access_token;
    credential.token.refresh_token = refreshed.refresh_token.or(Some(refresh_token));
    credential.token.expires_at = refreshed
        .expires_in
        .map(|seconds| now_seconds() + seconds as i64);
    save_credential(store, key, &credential)?;
    Ok(Some(credential.token.access_token))
}

fn decode_stored_credential(value: &str) -> Result<StoredCredential> {
    let Some(json) = value.strip_prefix(CREDENTIAL_PREFIX) else {
        bail!("authorization required; sign in or enter an access token again");
    };
    let credential: StoredCredential =
        serde_json::from_str(json).context("decode saved device sync credential")?;
    if credential.version != CREDENTIAL_VERSION {
        bail!("authorization required; unsupported saved credential version");
    }
    Ok(credential)
}

fn validate_refresh_endpoint(
    provider: ProviderId,
    token_url: &str,
    allow_http: bool,
) -> Result<()> {
    let url = reqwest::Url::parse(token_url).context("parse OAuth refresh endpoint")?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("OAuth refresh endpoint is not trusted for this provider");
    }
    let test_loopback = allow_http
        && url.scheme() == "http"
        && url.host_str().is_some_and(|host| {
            host.parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
        });
    let trusted = match provider {
        ProviderId::Github => {
            token_url == GITHUB_TOKEN_URL || (test_loopback && url.path() == "/github/token")
        }
        ProviderId::Gitlab => {
            token_url == GITLAB_TOKEN_URL || (test_loopback && url.path() == "/gitlab/token")
        }
        ProviderId::Gitee => {
            (url.scheme() == "https" || test_loopback)
                && url.path() == "/v1/oauth/gitee/device/poll"
        }
    };
    if !trusted {
        bail!("OAuth refresh endpoint is not trusted for this provider");
    }
    Ok(())
}

#[derive(Deserialize)]
struct RefreshResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemCredentialStore;

impl CredentialStore for SystemCredentialStore {
    fn set(&self, key: &str, secret: &str) -> Result<()> {
        let entry = keyring::Entry::new(DEVICE_SYNC_KEYRING_SERVICE, key)
            .context("open system credential store")?;
        entry
            .set_password(secret)
            .context("save device sync credential")
    }

    fn get(&self, key: &str) -> Result<Option<String>> {
        let entry = keyring::Entry::new(DEVICE_SYNC_KEYRING_SERVICE, key)
            .context("open system credential store")?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err).context("read device sync credential"),
        }
    }

    fn delete(&self, key: &str) -> Result<()> {
        let entry = keyring::Entry::new(DEVICE_SYNC_KEYRING_SERVICE, key)
            .context("open system credential store")?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err).context("delete device sync credential"),
        }
    }
}

#[cfg(test)]
#[derive(Default)]
pub struct MemoryCredentialStore(std::sync::Mutex<std::collections::HashMap<String, String>>);

#[cfg(test)]
impl CredentialStore for MemoryCredentialStore {
    fn set(&self, key: &str, secret: &str) -> Result<()> {
        self.0
            .lock()
            .unwrap()
            .insert(key.to_string(), secret.to_string());
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }

    fn delete(&self, key: &str) -> Result<()> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_roundtrips_and_deletes_secret() {
        let store = MemoryCredentialStore::default();
        assert_eq!(store.get("account").unwrap(), None);
        store.set("account", "secret").unwrap();
        assert_eq!(store.get("account").unwrap().as_deref(), Some("secret"));
        store.delete("account").unwrap();
        assert_eq!(store.get("account").unwrap(), None);
    }

    #[test]
    fn github_oauth_token_only_resolves_for_github_usage() {
        let store = MemoryCredentialStore::default();
        save_oauth_credential(
            &store,
            "oauth",
            ProviderId::Github,
            &OAuthCredential {
                access_token: "oauth-token".to_string(),
                refresh_token: None,
                expires_at: Some(now_seconds() + 3600),
                token_url: "https://example/token".to_string(),
                client_id: "client".to_string(),
            },
        )
        .unwrap();

        let github = CredentialUsage::from_https_remote(
            ProviderId::Github,
            "https://GitHub.COM/example/sync.git",
        )
        .unwrap();
        assert_eq!(
            resolve_access_token(&store, "oauth", &github)
                .unwrap()
                .as_deref(),
            Some("oauth-token")
        );

        let attacker = CredentialUsage::from_https_remote(
            ProviderId::Github,
            "https://attacker.example/example/sync.git",
        )
        .unwrap();
        assert!(resolve_access_token(&store, "oauth", &attacker).is_err());

        let gitlab = CredentialUsage::from_https_remote(
            ProviderId::Gitlab,
            "https://github.com/example/sync.git",
        )
        .unwrap();
        assert!(resolve_access_token(&store, "oauth", &gitlab).is_err());
    }

    #[test]
    fn oauth_refresh_endpoint_must_match_the_saved_provider() {
        let store = MemoryCredentialStore::default();
        let error = save_oauth_credential(
            &store,
            "oauth",
            ProviderId::Github,
            &OAuthCredential {
                access_token: "oauth-token".to_string(),
                refresh_token: Some("refresh-token".to_string()),
                expires_at: Some(now_seconds()),
                token_url: "https://gitlab.com/oauth/token".to_string(),
                client_id: "client".to_string(),
            },
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("refresh endpoint"), "{error}");
        assert_eq!(store.get("oauth").unwrap(), None);
    }

    #[test]
    fn loaded_oauth_credential_revalidates_refresh_endpoint_before_request() {
        let mut attacker_server = mockito::Server::new();
        let attacker_request = attacker_server
            .mock("POST", "/gitlab/token")
            .expect(0)
            .with_status(200)
            .with_body(r#"{"access_token":"attacker-token","expires_in":3600}"#)
            .create();
        let store = MemoryCredentialStore::default();
        save_credential(
            &store,
            "oauth",
            &StoredCredential {
                version: CREDENTIAL_VERSION,
                kind: CredentialKind::OAuth,
                provider: ProviderId::Github,
                allowed_origin: "github.com:443".to_string(),
                token: StoredToken {
                    access_token: "expired-token".to_string(),
                    refresh_token: Some("refresh-token".to_string()),
                    expires_at: Some(now_seconds()),
                },
                refresh: Some(TrustedRefreshConfig {
                    token_url: format!("{}/gitlab/token", attacker_server.url()),
                    client_id: "github-client".to_string(),
                }),
            },
        )
        .unwrap();

        let stored_endpoint = format!("{}/gitlab/token", attacker_server.url());
        let error = resolve_access_token_with_trusted_refresh_endpoint(
            &store,
            "oauth",
            &CredentialUsage::official(ProviderId::Github),
            Some(&stored_endpoint),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("refresh endpoint"), "{error}");
        attacker_request.assert();
    }

    #[test]
    fn oauth_refresh_does_not_follow_temporary_or_permanent_redirects() {
        for redirect_status in [307, 308] {
            let mut trusted_server = mockito::Server::new();
            let mut attacker_server = mockito::Server::new();
            let redirect_location = format!("{}/token", attacker_server.url());
            trusted_server
                .mock("POST", "/github/token")
                .with_status(redirect_status)
                .with_header("location", &redirect_location)
                .create();
            let attacker_request = attacker_server
                .mock("POST", "/token")
                .expect(0)
                .with_status(200)
                .with_body(r#"{"access_token":"attacker-token","expires_in":3600}"#)
                .create();
            let store = MemoryCredentialStore::default();
            let trusted_endpoint = format!("{}/github/token", trusted_server.url());
            save_oauth_credential_with_http_policy(
                &store,
                "oauth",
                ProviderId::Github,
                &OAuthCredential {
                    access_token: "expired-token".to_string(),
                    refresh_token: Some("refresh-token".to_string()),
                    expires_at: Some(now_seconds()),
                    token_url: trusted_endpoint.clone(),
                    client_id: "github-client".to_string(),
                },
                true,
            )
            .unwrap();

            let result = resolve_access_token_with_trusted_refresh_endpoint(
                &store,
                "oauth",
                &CredentialUsage::official(ProviderId::Github),
                Some(&trusted_endpoint),
            );

            assert!(result.is_err(), "accepted HTTP {redirect_status} redirect");
            attacker_request.assert();
        }
    }

    #[test]
    fn gitee_refresh_endpoint_must_exactly_match_current_local_relay() {
        let store = MemoryCredentialStore::default();
        save_credential(
            &store,
            "oauth",
            &StoredCredential {
                version: CREDENTIAL_VERSION,
                kind: CredentialKind::OAuth,
                provider: ProviderId::Gitee,
                allowed_origin: "gitee.com:443".to_string(),
                token: StoredToken {
                    access_token: "expired-token".to_string(),
                    refresh_token: Some("refresh-token".to_string()),
                    expires_at: Some(now_seconds()),
                },
                refresh: Some(TrustedRefreshConfig {
                    token_url: "https://attacker.example/v1/oauth/gitee/device/poll".to_string(),
                    client_id: "relay".to_string(),
                }),
            },
        )
        .unwrap();

        let error = resolve_access_token_with_trusted_refresh_endpoint(
            &store,
            "oauth",
            &CredentialUsage::official(ProviderId::Gitee),
            Some("https://relay.example/v1/oauth/gitee/device/poll"),
        )
        .unwrap_err()
        .to_string();

        assert!(
            error.contains("current trusted refresh endpoint"),
            "{error}"
        );
    }

    #[test]
    fn gitee_refresh_endpoint_mismatch_is_rejected_before_request() {
        let trusted_server = mockito::Server::new();
        let mut attacker_server = mockito::Server::new();
        let attacker_request = attacker_server
            .mock("POST", "/v1/oauth/gitee/device/poll")
            .expect(0)
            .with_status(200)
            .with_body(r#"{"access_token":"attacker-token","expires_in":3600}"#)
            .create();
        let store = MemoryCredentialStore::default();
        save_credential(
            &store,
            "oauth",
            &StoredCredential {
                version: CREDENTIAL_VERSION,
                kind: CredentialKind::OAuth,
                provider: ProviderId::Gitee,
                allowed_origin: "gitee.com:443".to_string(),
                token: StoredToken {
                    access_token: "expired-token".to_string(),
                    refresh_token: Some("refresh-token".to_string()),
                    expires_at: Some(now_seconds()),
                },
                refresh: Some(TrustedRefreshConfig {
                    token_url: format!("{}/v1/oauth/gitee/device/poll", attacker_server.url()),
                    client_id: "relay".to_string(),
                }),
            },
        )
        .unwrap();
        let trusted_endpoint = format!("{}/v1/oauth/gitee/device/poll", trusted_server.url());

        let error = resolve_access_token_with_trusted_refresh_endpoint(
            &store,
            "oauth",
            &CredentialUsage::official(ProviderId::Gitee),
            Some(&trusted_endpoint),
        )
        .unwrap_err()
        .to_string();

        assert!(
            error.contains("current trusted refresh endpoint"),
            "{error}"
        );
        attacker_request.assert();
    }

    #[test]
    fn personal_access_token_only_resolves_for_its_exact_saved_host() {
        let store = MemoryCredentialStore::default();
        let saved_usage = CredentialUsage::from_https_remote(
            ProviderId::Gitlab,
            "https://Git.Example.COM/team/sync.git",
        )
        .unwrap();
        save_personal_access_token(&store, "pat", &saved_usage, "pat-token").unwrap();

        let same_host = CredentialUsage::from_https_remote(
            ProviderId::Gitlab,
            "https://git.example.com/another/repository.git",
        )
        .unwrap();
        assert_eq!(
            resolve_access_token(&store, "pat", &same_host)
                .unwrap()
                .as_deref(),
            Some("pat-token")
        );

        let subdomain = CredentialUsage::from_https_remote(
            ProviderId::Gitlab,
            "https://sub.git.example.com/team/sync.git",
        )
        .unwrap();
        assert!(resolve_access_token(&store, "pat", &subdomain).is_err());
    }

    #[test]
    fn personal_access_token_is_bound_to_the_effective_https_port() {
        let store = MemoryCredentialStore::default();
        let saved_usage = CredentialUsage::from_https_remote(
            ProviderId::Gitlab,
            "https://git.example.com:8443/team/sync.git",
        )
        .unwrap();
        save_personal_access_token(&store, "pat-port", &saved_usage, "pat-token").unwrap();

        let different_port = CredentialUsage::from_https_remote(
            ProviderId::Gitlab,
            "https://git.example.com:9443/team/sync.git",
        )
        .unwrap();
        assert!(resolve_access_token(&store, "pat-port", &different_port).is_err());

        let default_port = CredentialUsage::from_https_remote(
            ProviderId::Github,
            "https://github.com:443/example/sync.git",
        )
        .unwrap();
        save_personal_access_token(&store, "pat-default-port", &default_port, "pat-token").unwrap();
        let implicit_default = CredentialUsage::from_https_remote(
            ProviderId::Github,
            "https://github.com/example/other.git",
        )
        .unwrap();
        assert_eq!(
            resolve_access_token(&store, "pat-default-port", &implicit_default)
                .unwrap()
                .as_deref(),
            Some("pat-token")
        );
    }

    #[test]
    fn legacy_unbound_credentials_require_authorization_again() {
        let store = MemoryCredentialStore::default();
        store.set("raw", "raw-token").unwrap();
        store
            .set(
                "oauth-v1",
                r#"skills-hub-oauth-v1:{"access_token":"old-token"}"#,
            )
            .unwrap();
        let usage = CredentialUsage::from_https_remote(
            ProviderId::Github,
            "https://github.com/example/sync.git",
        )
        .unwrap();

        for key in ["raw", "oauth-v1"] {
            let error = resolve_access_token(&store, key, &usage)
                .unwrap_err()
                .to_string();
            assert!(error.contains("authorization"), "{error}");
        }
    }

    #[test]
    fn credential_usage_rejects_non_https_and_ambiguous_urls() {
        for remote_url in [
            "http://github.com/example/sync.git",
            "ssh://git@github.com/example/sync.git",
            "https:////@github.com/repo.git",
            "https://@github.com/example/sync.git",
            "https://user@github.com/example/sync.git",
            "https://github.com/example/sync.git?ref=main",
            "https://github.com/example/sync.git#fragment",
        ] {
            assert!(
                CredentialUsage::from_https_remote(ProviderId::Github, remote_url).is_err(),
                "accepted {remote_url}"
            );
        }
    }
}
