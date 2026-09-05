use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, USER_AGENT};
use serde::Deserialize;
use uuid::Uuid;

use super::credentials::{
    save_oauth_credential, save_oauth_credential_with_http_policy, CredentialStore,
    OAuthCredential, GITHUB_TOKEN_URL, GITLAB_TOKEN_URL,
};
use super::providers::provider;
use super::types::{
    OAuthPollResult, OAuthPollStatus, OAuthProviderAvailability, OAuthStartResult, ProviderAccount,
    ProviderId,
};

const GITHUB_DEVICE_URL: &str = "https://github.com/login/device/code";
const GITLAB_DEVICE_URL: &str = "https://gitlab.com/oauth/authorize_device";

#[derive(Clone, Debug)]
struct PendingFlow {
    claim_id: Uuid,
    provider: ProviderId,
    device_code: String,
    token_url: String,
    client_id: String,
    expires_at: i64,
    interval_seconds: u64,
    next_poll_at: i64,
    relay: bool,
}

static FLOWS: OnceLock<Mutex<HashMap<String, PendingFlow>>> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct OAuthEndpoints {
    github_client_id: Option<String>,
    github_device_url: String,
    github_token_url: String,
    gitlab_client_id: Option<String>,
    gitlab_device_url: String,
    gitlab_token_url: String,
    gitee_relay_url: Option<String>,
}

impl Default for OAuthEndpoints {
    fn default() -> Self {
        Self {
            github_client_id: configured_value(
                "SKILLS_HUB_GITHUB_CLIENT_ID",
                option_env!("SKILLS_HUB_GITHUB_CLIENT_ID"),
            ),
            github_device_url: GITHUB_DEVICE_URL.to_string(),
            github_token_url: GITHUB_TOKEN_URL.to_string(),
            gitlab_client_id: configured_value(
                "SKILLS_HUB_GITLAB_CLIENT_ID",
                option_env!("SKILLS_HUB_GITLAB_CLIENT_ID"),
            ),
            gitlab_device_url: GITLAB_DEVICE_URL.to_string(),
            gitlab_token_url: GITLAB_TOKEN_URL.to_string(),
            gitee_relay_url: configured_value(
                "SKILLS_HUB_GITEE_AUTH_RELAY_URL",
                option_env!("SKILLS_HUB_GITEE_AUTH_RELAY_URL"),
            ),
        }
    }
}

impl OAuthEndpoints {
    fn trusted_refresh_endpoint(
        &self,
        provider_id: ProviderId,
        allow_http: bool,
    ) -> Result<Option<String>> {
        match provider_id {
            ProviderId::Github => Ok(Some(self.github_token_url.clone())),
            ProviderId::Gitlab => Ok(Some(self.gitlab_token_url.clone())),
            ProviderId::Gitee => {
                let Some(relay_url) = self.gitee_relay_url.as_deref() else {
                    return Ok(None);
                };
                let relay_origin = validate_relay_url(relay_url, allow_http)?;
                Ok(Some(format!("{relay_origin}/v1/oauth/gitee/device/poll")))
            }
        }
    }
}

pub fn availability() -> Vec<OAuthProviderAvailability> {
    let endpoints = OAuthEndpoints::default();
    vec![
        availability_item(ProviderId::Github, endpoints.github_client_id.is_some()),
        availability_item(ProviderId::Gitlab, endpoints.gitlab_client_id.is_some()),
        availability_item(ProviderId::Gitee, endpoints.gitee_relay_url.is_some()),
    ]
}

pub fn start(provider_id: ProviderId) -> Result<OAuthStartResult> {
    start_with_endpoints(provider_id, &OAuthEndpoints::default(), false)
}

pub(crate) fn trusted_refresh_endpoint(provider_id: ProviderId) -> Result<Option<String>> {
    OAuthEndpoints::default().trusted_refresh_endpoint(provider_id, false)
}

pub fn poll(flow_id: &str, credentials: &dyn CredentialStore) -> Result<OAuthPollResult> {
    poll_flow(flow_id, credentials, false, |provider_id, token| {
        provider(provider_id).validate_token(token)
    })
}

pub fn cancel(flow_id: &str) {
    flows().lock().unwrap().remove(flow_id);
}

fn start_with_endpoints(
    provider_id: ProviderId,
    endpoints: &OAuthEndpoints,
    allow_http: bool,
) -> Result<OAuthStartResult> {
    let client = oauth_http_client()?;
    let (client_id, response, token_url, relay) = match provider_id {
        ProviderId::Github => {
            let client_id = endpoints
                .github_client_id
                .clone()
                .context("GitHub OAuth is not configured in this build")?;
            let response = client
                .post(&endpoints.github_device_url)
                .header(ACCEPT, "application/json")
                .header(USER_AGENT, "Skills-Hub")
                .form(&[("client_id", client_id.as_str()), ("scope", "repo")])
                .send()
                .context("start GitHub authorization")?;
            (
                client_id,
                response,
                endpoints.github_token_url.clone(),
                false,
            )
        }
        ProviderId::Gitlab => {
            let client_id = endpoints
                .gitlab_client_id
                .clone()
                .context("GitLab OAuth is not configured in this build")?;
            let response = client
                .post(&endpoints.gitlab_device_url)
                .header(ACCEPT, "application/json")
                .form(&[("client_id", client_id.as_str()), ("scope", "api")])
                .send()
                .context("start GitLab authorization")?;
            (
                client_id,
                response,
                endpoints.gitlab_token_url.clone(),
                false,
            )
        }
        ProviderId::Gitee => {
            let relay_url = endpoints
                .gitee_relay_url
                .as_deref()
                .context("Gitee OAuth relay is not configured in this build")?;
            let relay_origin = validate_relay_url(relay_url, allow_http)?;
            let token_url = endpoints
                .trusted_refresh_endpoint(ProviderId::Gitee, allow_http)?
                .context("Gitee OAuth relay is not configured in this build")?;
            let response = client
                .post(format!("{relay_origin}/v1/oauth/gitee/device/start"))
                .header(ACCEPT, "application/json")
                .json(&serde_json::json!({"app": "skills-hub"}))
                .send()
                .context("start Gitee authorization")?;
            ("relay".to_string(), response, token_url, true)
        }
    };
    if !response.status().is_success() {
        bail!("OAuth provider rejected the authorization request");
    }
    let device: DeviceResponse = response.json().context("decode OAuth device response")?;
    validate_verification_uri(provider_id, &device.verification_uri, allow_http)?;
    if let Some(uri) = device.verification_uri_complete.as_deref() {
        validate_verification_uri(provider_id, uri, allow_http)?;
    }
    let now = now_seconds();
    let expires_at = now + device.expires_in as i64;
    let interval = device.interval.max(3);
    let flow_id = Uuid::new_v4().to_string();
    flows().lock().unwrap().insert(
        flow_id.clone(),
        PendingFlow {
            claim_id: Uuid::new_v4(),
            provider: provider_id,
            device_code: device.device_code,
            token_url,
            client_id,
            expires_at,
            interval_seconds: interval,
            next_poll_at: now,
            relay,
        },
    );
    Ok(OAuthStartResult {
        flow_id,
        verification_uri: device.verification_uri,
        verification_uri_complete: device.verification_uri_complete,
        user_code: device.user_code,
        expires_at,
        interval_seconds: interval,
    })
}

fn poll_flow<F>(
    flow_id: &str,
    credentials: &dyn CredentialStore,
    allow_http: bool,
    validate: F,
) -> Result<OAuthPollResult>
where
    F: FnOnce(ProviderId, &str) -> Result<ProviderAccount>,
{
    let client = oauth_http_client()?;
    let now = now_seconds();
    let flow = flows()
        .lock()
        .unwrap()
        .get(flow_id)
        .cloned()
        .context("OAuth authorization session expired; start again")?;
    if now >= flow.expires_at {
        remove_flow_if_current(flow_id, flow.claim_id);
        bail!("OAuth authorization session expired; start again");
    }
    if now < flow.next_poll_at {
        return Ok(pending(flow.provider, flow.interval_seconds));
    }
    let response = if flow.relay {
        client
            .post(&flow.token_url)
            .header(ACCEPT, "application/json")
            .json(&serde_json::json!({"session_id": flow.device_code}))
            .send()
    } else {
        client
            .post(&flow.token_url)
            .header(ACCEPT, "application/json")
            .header(USER_AGENT, "Skills-Hub")
            .form(&[
                ("client_id", flow.client_id.as_str()),
                ("device_code", flow.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
    }
    .context("poll OAuth authorization")?;
    let token: TokenResponse = response.json().context("decode OAuth token response")?;
    if let Some(error) = token.error.as_deref() {
        let interval = if error == "slow_down" {
            flow.interval_seconds + 5
        } else {
            flow.interval_seconds
        };
        if matches!(error, "authorization_pending" | "slow_down") {
            update_pending_flow(flow_id, flow.claim_id, interval, now + interval as i64)?;
            return Ok(pending(flow.provider, interval));
        }
        remove_flow_if_current(flow_id, flow.claim_id);
        bail!("OAuth authorization failed: {}", sanitize_error(error));
    }
    let access_token = token
        .access_token
        .context("OAuth response missing access token")?;
    let account = validate(flow.provider, &access_token)?;
    claim_flow(flow_id, flow.claim_id)?;
    let credential_key = Uuid::new_v4().to_string();
    let credential = OAuthCredential {
        access_token,
        refresh_token: token.refresh_token,
        expires_at: token.expires_in.map(|seconds| now + seconds as i64),
        token_url: flow.token_url,
        client_id: flow.client_id,
    };
    if allow_http {
        save_oauth_credential_with_http_policy(
            credentials,
            &credential_key,
            flow.provider,
            &credential,
            true,
        )?;
    } else {
        save_oauth_credential(credentials, &credential_key, flow.provider, &credential)?;
    }
    Ok(OAuthPollResult {
        provider: flow.provider,
        status: OAuthPollStatus::Authorized,
        interval_seconds: flow.interval_seconds,
        credential_key: Some(credential_key),
        account: Some(account),
    })
}

fn claim_flow(flow_id: &str, claim_id: Uuid) -> Result<()> {
    let mut pending_flows = flows().lock().unwrap();
    let is_current = pending_flows
        .get(flow_id)
        .is_some_and(|current| current.claim_id == claim_id);
    if !is_current {
        bail!("OAuth authorization session was canceled or already completed");
    }
    pending_flows.remove(flow_id);
    Ok(())
}

fn update_pending_flow(
    flow_id: &str,
    claim_id: Uuid,
    interval_seconds: u64,
    next_poll_at: i64,
) -> Result<()> {
    let mut pending_flows = flows().lock().unwrap();
    let current = pending_flows
        .get_mut(flow_id)
        .filter(|current| current.claim_id == claim_id)
        .context("OAuth authorization session was canceled or already completed")?;
    current.interval_seconds = interval_seconds;
    current.next_poll_at = next_poll_at;
    Ok(())
}

fn remove_flow_if_current(flow_id: &str, claim_id: Uuid) {
    let mut pending_flows = flows().lock().unwrap();
    if pending_flows
        .get(flow_id)
        .is_some_and(|current| current.claim_id == claim_id)
    {
        pending_flows.remove(flow_id);
    }
}

fn oauth_http_client() -> Result<Client> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("build OAuth HTTP client")
}

fn pending(provider: ProviderId, interval_seconds: u64) -> OAuthPollResult {
    OAuthPollResult {
        provider,
        status: OAuthPollStatus::Pending,
        interval_seconds,
        credential_key: None,
        account: None,
    }
}

fn select_configured_value(
    debug_build: bool,
    runtime: Option<&str>,
    compiled: Option<&str>,
) -> Option<String> {
    let non_empty = |value: &str| (!value.trim().is_empty()).then(|| value.to_string());
    if debug_build {
        runtime.and_then(non_empty)
    } else {
        None
    }
    .or_else(|| compiled.and_then(non_empty))
}

#[cfg(debug_assertions)]
fn configured_value(key: &str, compiled: Option<&str>) -> Option<String> {
    let runtime = std::env::var(key).ok();
    select_configured_value(true, runtime.as_deref(), compiled)
}

#[cfg(not(debug_assertions))]
fn configured_value(_key: &str, compiled: Option<&str>) -> Option<String> {
    select_configured_value(false, None, compiled)
}

fn availability_item(provider: ProviderId, available: bool) -> OAuthProviderAvailability {
    OAuthProviderAvailability {
        provider,
        available,
        reason: (!available).then(|| "not_configured".to_string()),
    }
}

fn validate_verification_uri(provider: ProviderId, uri: &str, allow_http: bool) -> Result<()> {
    let url = reqwest::Url::parse(uri).context("parse OAuth authorization URL")?;
    let is_test_http =
        allow_http && url.scheme() == "http" && url.host_str().is_some_and(is_loopback_host);
    let allowed = (url.scheme() == "https"
        && url.host_str() == Some(provider.official_git_host())
        && url.port_or_known_default() == Some(443))
        || is_test_http;
    let allowed = allowed && url.username().is_empty() && url.password().is_none();
    if !allowed {
        bail!("OAuth provider returned an unsupported authorization URL");
    }
    Ok(())
}

fn validate_relay_url(value: &str, allow_http: bool) -> Result<String> {
    let url = reqwest::Url::parse(value.trim()).context("parse Gitee OAuth relay URL")?;
    let valid_scheme = url.scheme() == "https"
        || (allow_http && url.scheme() == "http" && url.host_str().is_some_and(is_loopback_host));
    if !valid_scheme
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
        || url.port_or_known_default().is_none()
    {
        bail!("Gitee OAuth relay must be a pure HTTPS origin");
    }
    Ok(url.origin().ascii_serialization())
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

fn flows() -> &'static Mutex<HashMap<String, PendingFlow>> {
    FLOWS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn sanitize_error(value: &str) -> String {
    value.replace(['\n', '\r'], " ").chars().take(80).collect()
}

#[derive(Deserialize)]
struct DeviceResponse {
    #[serde(alias = "session_id")]
    device_code: String,
    user_code: Option<String>,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    expires_in: u64,
    #[serde(default = "default_interval")]
    interval: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
}

fn default_interval() -> u64 {
    5
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::device_sync::credentials::{
        resolve_access_token, resolve_access_token_with_trusted_refresh_endpoint,
        MemoryCredentialStore,
    };
    use crate::core::device_sync::types::CredentialUsage;
    use crate::core::skill_store::SkillStore;
    use std::ffi::OsString;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::time::Duration;

    const TEST_CONFIG_ENV: &str = "SKILLS_HUB_TEST_CONFIGURED_VALUE";
    static CONFIG_ENV_LOCK: Mutex<()> = Mutex::new(());

    struct TemporaryEnvironmentVariable {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl TemporaryEnvironmentVariable {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for TemporaryEnvironmentVariable {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn endpoints(server: &mockito::Server) -> OAuthEndpoints {
        OAuthEndpoints {
            github_client_id: Some("github-client".to_string()),
            github_device_url: format!("{}/github/device", server.url()),
            github_token_url: format!("{}/github/token", server.url()),
            gitlab_client_id: Some("gitlab-client".to_string()),
            gitlab_device_url: format!("{}/gitlab/device", server.url()),
            gitlab_token_url: format!("{}/gitlab/token", server.url()),
            gitee_relay_url: Some(server.url()),
        }
    }

    #[derive(Default)]
    struct RecordingCredentialStore(Mutex<HashMap<String, String>>);

    impl RecordingCredentialStore {
        fn is_empty(&self) -> bool {
            self.0.lock().unwrap().is_empty()
        }

        fn len(&self) -> usize {
            self.0.lock().unwrap().len()
        }
    }

    impl CredentialStore for RecordingCredentialStore {
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

    fn read_complete_http_request(stream: &mut std::net::TcpStream) {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0);
            request.extend_from_slice(&buffer[..read]);
            let Some(headers_end) = request.windows(4).position(|part| part == b"\r\n\r\n") else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..headers_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.split_once(':').and_then(|(name, value)| {
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                })
                .unwrap_or(0);
            if request.len() >= headers_end + 4 + content_length {
                return;
            }
        }
    }

    fn blocked_token_server(
        request_count: usize,
    ) -> (
        String,
        Receiver<()>,
        Sender<()>,
        std::thread::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (arrived_tx, arrived_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let mut streams = Vec::new();
            for _ in 0..request_count {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                read_complete_http_request(&mut stream);
                arrived_tx.send(()).unwrap();
                streams.push(stream);
            }
            release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let body = r#"{"access_token":"delayed-success-token"}"#;
            for mut stream in streams {
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
                stream.flush().unwrap();
            }
        });
        (
            format!("http://{address}/token"),
            arrived_rx,
            release_tx,
            handle,
        )
    }

    fn insert_pollable_test_flow(token_url: String) -> String {
        let flow_id = Uuid::new_v4().to_string();
        flows().lock().unwrap().insert(
            flow_id.clone(),
            PendingFlow {
                claim_id: Uuid::new_v4(),
                provider: ProviderId::Github,
                device_code: "device".to_string(),
                token_url,
                client_id: "client".to_string(),
                expires_at: now_seconds() + 900,
                interval_seconds: 3,
                next_poll_at: now_seconds(),
                relay: false,
            },
        );
        flow_id
    }

    #[test]
    fn debug_configuration_prefers_non_empty_runtime_value() {
        assert_eq!(
            select_configured_value(true, Some("runtime-client"), Some("compiled-client")),
            Some("runtime-client".to_string())
        );
        assert_eq!(
            select_configured_value(true, Some("   "), Some("compiled-client")),
            Some("compiled-client".to_string())
        );
    }

    #[test]
    fn release_configuration_uses_only_compiled_value() {
        assert_eq!(
            select_configured_value(false, Some("runtime-client"), Some("compiled-client")),
            Some("compiled-client".to_string())
        );
        assert_eq!(
            select_configured_value(false, Some("runtime-client"), None),
            None
        );
    }

    #[test]
    fn gitee_relay_rejects_everything_except_a_pure_https_origin() {
        for relay_url in [
            "http://relay.example",
            "https://user@relay.example",
            "https://relay.example/nested",
            "https://relay.example?tenant=other",
            "https://relay.example/#fragment",
        ] {
            assert!(
                validate_relay_url(relay_url, false).is_err(),
                "accepted {relay_url}"
            );
        }
    }

    #[test]
    fn gitee_relay_normalizes_host_and_effective_port_with_explicit_test_http_policy() {
        assert_eq!(
            validate_relay_url("https://Relay.Example:443/", false).unwrap(),
            "https://relay.example"
        );
        assert_eq!(
            validate_relay_url("https://Relay.Example:8443", false).unwrap(),
            "https://relay.example:8443"
        );
        assert_eq!(
            validate_relay_url("http://127.0.0.1:4321", true).unwrap(),
            "http://127.0.0.1:4321"
        );
        assert!(validate_relay_url("http://127.0.0.1:4321/nested", true).is_err());
        assert!(validate_relay_url("http://relay.example:4321", true).is_err());
    }

    #[cfg(debug_assertions)]
    #[test]
    fn debug_configured_value_reads_runtime_environment() {
        let _lock = CONFIG_ENV_LOCK.lock().unwrap();
        let _environment = TemporaryEnvironmentVariable::set(TEST_CONFIG_ENV, "runtime-client");

        assert_eq!(
            configured_value(TEST_CONFIG_ENV, Some("compiled-client")),
            Some("runtime-client".to_string())
        );
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn release_configured_value_ignores_runtime_environment() {
        let _lock = CONFIG_ENV_LOCK.lock().unwrap();
        let _environment = TemporaryEnvironmentVariable::set(TEST_CONFIG_ENV, "runtime-client");

        assert_eq!(
            configured_value(TEST_CONFIG_ENV, Some("compiled-client")),
            Some("compiled-client".to_string())
        );
    }

    #[test]
    fn github_device_flow_stores_token_only_in_credential_store() {
        let mut server = mockito::Server::new();
        let start_request = server
            .mock("POST", "/github/device")
            .match_body(mockito::Matcher::UrlEncoded(
                "client_id".into(),
                "github-client".into(),
            ))
            .with_status(200)
            .with_body(format!(
                r#"{{"device_code":"device","user_code":"ABCD","verification_uri":"{}/verify","expires_in":900,"interval":3}}"#,
                server.url()
            ))
            .create();
        let token_request = server
            .mock("POST", "/github/token")
            .with_status(200)
            .with_body(r#"{"access_token":"secret-oauth-token"}"#)
            .create();
        let flow = start_with_endpoints(ProviderId::Github, &endpoints(&server), true).unwrap();
        let store = MemoryCredentialStore::default();
        let result = poll_flow(&flow.flow_id, &store, true, |provider_id, token| {
            assert_eq!(provider_id, ProviderId::Github);
            assert_eq!(token, "secret-oauth-token");
            Ok(ProviderAccount {
                login: "may".to_string(),
                display_name: None,
            })
        })
        .unwrap();
        assert_eq!(result.status, OAuthPollStatus::Authorized);
        assert_eq!(result.account.unwrap().login, "may");
        let key = result.credential_key.unwrap();
        assert_eq!(
            resolve_access_token(&store, &key, &CredentialUsage::official(ProviderId::Github),)
                .unwrap()
                .as_deref(),
            Some("secret-oauth-token")
        );
        start_request.assert();
        token_request.assert();
    }

    #[test]
    fn pending_authorization_remains_pending() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/gitlab/device")
            .with_status(200)
            .with_body(format!(
                r#"{{"device_code":"device","user_code":"EFGH","verification_uri":"{}/verify","expires_in":900,"interval":3}}"#,
                server.url()
            ))
            .create();
        server
            .mock("POST", "/gitlab/token")
            .with_status(400)
            .with_body(r#"{"error":"authorization_pending"}"#)
            .create();
        let flow = start_with_endpoints(ProviderId::Gitlab, &endpoints(&server), true).unwrap();
        let result = poll_flow(
            &flow.flow_id,
            &MemoryCredentialStore::default(),
            true,
            |_, _| unreachable!(),
        )
        .unwrap();
        assert_eq!(result.status, OAuthPollStatus::Pending);
        assert!(flows().lock().unwrap().contains_key(&flow.flow_id));
        cancel(&flow.flow_id);
    }

    #[test]
    fn slow_down_keeps_the_flow_and_increases_its_poll_interval() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/gitlab/device")
            .with_status(200)
            .with_body(format!(
                r#"{{"device_code":"device","user_code":"EFGH","verification_uri":"{}/verify","expires_in":900,"interval":3}}"#,
                server.url()
            ))
            .create();
        server
            .mock("POST", "/gitlab/token")
            .with_status(400)
            .with_body(r#"{"error":"slow_down"}"#)
            .create();
        let flow = start_with_endpoints(ProviderId::Gitlab, &endpoints(&server), true).unwrap();

        let result = poll_flow(
            &flow.flow_id,
            &MemoryCredentialStore::default(),
            true,
            |_, _| unreachable!(),
        )
        .unwrap();

        assert_eq!(result.status, OAuthPollStatus::Pending);
        assert_eq!(result.interval_seconds, 8);
        let stored = flows().lock().unwrap().get(&flow.flow_id).cloned().unwrap();
        assert_eq!(stored.interval_seconds, 8);
        assert!(stored.next_poll_at > now_seconds());
        cancel(&flow.flow_id);
    }

    #[test]
    fn cancel_during_a_blocked_successful_poll_prevents_credential_persistence() {
        let (token_url, arrived, release, server) = blocked_token_server(1);
        let flow_id = insert_pollable_test_flow(token_url);
        let credentials = RecordingCredentialStore::default();
        let dir = tempfile::tempdir().unwrap();
        let store = SkillStore::new(dir.path().join("skills.db"));
        store.ensure_schema().unwrap();

        let result = std::thread::scope(|scope| {
            let poll = scope.spawn(|| {
                crate::commands::poll_device_sync_oauth_with(
                    &store,
                    &credentials,
                    || {
                        poll_flow(&flow_id, &credentials, true, |_, _| {
                            Ok(ProviderAccount {
                                login: "may".to_string(),
                                display_name: None,
                            })
                        })
                    },
                    |pending| {
                        store.set_setting(
                            "device_sync_pending_oauth_v1",
                            &serde_json::to_string(pending)?,
                        )
                    },
                )
            });
            arrived.recv_timeout(Duration::from_secs(5)).unwrap();
            cancel(&flow_id);
            release.send(()).unwrap();
            poll.join().unwrap()
        });
        server.join().unwrap();

        assert!(result.is_err(), "a canceled flow returned authorization");
        assert!(credentials.is_empty(), "a canceled flow saved a credential");
        assert_eq!(
            store.get_setting("device_sync_pending_oauth_v1").unwrap(),
            None,
            "a canceled flow persisted a pending authorization"
        );
        assert!(!flows().lock().unwrap().contains_key(&flow_id));
    }

    #[test]
    fn concurrent_successful_polls_allow_only_one_flow_claim() {
        let (token_url, arrived, release, server) = blocked_token_server(2);
        let flow_id = insert_pollable_test_flow(token_url);
        let credentials = RecordingCredentialStore::default();

        let results = std::thread::scope(|scope| {
            let first = scope.spawn(|| {
                poll_flow(&flow_id, &credentials, true, |_, _| {
                    Ok(ProviderAccount {
                        login: "first".to_string(),
                        display_name: None,
                    })
                })
            });
            let second = scope.spawn(|| {
                poll_flow(&flow_id, &credentials, true, |_, _| {
                    Ok(ProviderAccount {
                        login: "second".to_string(),
                        display_name: None,
                    })
                })
            });
            arrived.recv_timeout(Duration::from_secs(5)).unwrap();
            arrived.recv_timeout(Duration::from_secs(5)).unwrap();
            release.send(()).unwrap();
            [first.join().unwrap(), second.join().unwrap()]
        });
        server.join().unwrap();

        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
        assert_eq!(credentials.len(), 1);
        assert!(!flows().lock().unwrap().contains_key(&flow_id));
    }

    #[test]
    fn oauth_start_does_not_follow_temporary_or_permanent_redirects() {
        for status in [307, 308] {
            let mut trusted_server = mockito::Server::new();
            let mut attacker_server = mockito::Server::new();
            let redirect_target = format!("{}/redirected", attacker_server.url());
            let trusted_request = trusted_server
                .mock("POST", "/github/device")
                .with_status(status)
                .with_header("location", &redirect_target)
                .create();
            let attacker_request = attacker_server
                .mock("POST", "/redirected")
                .expect(0)
                .with_status(200)
                .with_body(format!(
                    r#"{{"device_code":"stolen-device","user_code":"ABCD","verification_uri":"{}/verify","expires_in":900,"interval":3}}"#,
                    attacker_server.url()
                ))
                .create();
            let configured = endpoints(&trusted_server);

            let result = start_with_endpoints(ProviderId::Github, &configured, true);

            assert!(result.is_err(), "accepted HTTP {status} redirect");
            trusted_request.assert();
            attacker_request.assert();
        }
    }

    #[test]
    fn oauth_poll_does_not_follow_temporary_or_permanent_redirects() {
        for status in [307, 308] {
            let mut trusted_server = mockito::Server::new();
            let mut attacker_server = mockito::Server::new();
            let redirect_target = format!("{}/redirected", attacker_server.url());
            let trusted_request = trusted_server
                .mock("POST", "/github/token")
                .with_status(status)
                .with_header("location", &redirect_target)
                .create();
            let attacker_request = attacker_server
                .mock("POST", "/redirected")
                .expect(0)
                .with_status(200)
                .with_body(r#"{"access_token":"stolen-token"}"#)
                .create();
            let flow_id = Uuid::new_v4().to_string();
            flows().lock().unwrap().insert(
                flow_id.clone(),
                PendingFlow {
                    claim_id: Uuid::new_v4(),
                    provider: ProviderId::Github,
                    device_code: "secret-device-code".to_string(),
                    token_url: format!("{}/github/token", trusted_server.url()),
                    client_id: "secret-client-id".to_string(),
                    expires_at: now_seconds() + 900,
                    interval_seconds: 3,
                    next_poll_at: now_seconds(),
                    relay: false,
                },
            );

            let result = poll_flow(&flow_id, &MemoryCredentialStore::default(), true, |_, _| {
                Ok(ProviderAccount {
                    login: "attacker".to_string(),
                    display_name: None,
                })
            });

            cancel(&flow_id);
            assert!(result.is_err(), "accepted HTTP {status} redirect");
            trusted_request.assert();
            attacker_request.assert();
        }
    }

    #[test]
    fn token_response_cannot_override_trusted_refresh_destination() {
        let mut trusted_server = mockito::Server::new();
        let mut attacker_server = mockito::Server::new();
        trusted_server
            .mock("POST", "/github/device")
            .with_status(200)
            .with_body(format!(
                r#"{{"device_code":"device","user_code":"ABCD","verification_uri":"{}/verify","expires_in":900,"interval":3}}"#,
                trusted_server.url()
            ))
            .create();
        let initial_token_request = trusted_server
            .mock("POST", "/github/token")
            .match_body(mockito::Matcher::UrlEncoded(
                "grant_type".into(),
                "urn:ietf:params:oauth:grant-type:device_code".into(),
            ))
            .with_status(200)
            .with_body(format!(
                r#"{{"access_token":"initial-token","refresh_token":"refresh-token","expires_in":0,"refresh_url":"{}/token"}}"#,
                attacker_server.url()
            ))
            .create();
        let trusted_refresh_request = trusted_server
            .mock("POST", "/github/token")
            .match_body(mockito::Matcher::UrlEncoded(
                "grant_type".into(),
                "refresh_token".into(),
            ))
            .with_status(200)
            .with_body(r#"{"access_token":"trusted-refreshed-token","expires_in":3600}"#)
            .create();
        let attacker_request = attacker_server
            .mock("POST", "/token")
            .expect(0)
            .with_status(200)
            .with_body(r#"{"access_token":"attacker-token","expires_in":3600}"#)
            .create();

        let endpoints = endpoints(&trusted_server);
        let trusted_refresh_endpoint = endpoints.github_token_url.clone();
        let flow = start_with_endpoints(ProviderId::Github, &endpoints, true).unwrap();
        let store = MemoryCredentialStore::default();
        let result = poll_flow(&flow.flow_id, &store, true, |_, _| {
            Ok(ProviderAccount {
                login: "may".to_string(),
                display_name: None,
            })
        })
        .unwrap();

        let token = resolve_access_token_with_trusted_refresh_endpoint(
            &store,
            &result.credential_key.unwrap(),
            &CredentialUsage::official(ProviderId::Github),
            Some(&trusted_refresh_endpoint),
        )
        .unwrap();
        assert_eq!(token.as_deref(), Some("trusted-refreshed-token"));
        initial_token_request.assert();
        trusted_refresh_request.assert();
        attacker_request.assert();
    }
}
