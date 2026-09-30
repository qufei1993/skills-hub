use anyhow::{Context, Result};
use git2::{FetchOptions, ProxyOptions, PushOptions};
use reqwest::blocking::{Client, ClientBuilder};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use super::skill_store::SkillStore;

pub const GITHUB_PROXY_URL_KEY: &str = "github_proxy_url";
pub const DEFAULT_GITHUB_PROXY_URL: &str = "http://127.0.0.1:7890";
const DEFAULT_GITHUB_PROXY_HOST: &str = "127.0.0.1";
pub const DEFAULT_GITHUB_PROXY_PORT: u16 = 7890;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubProxyConfig {
    pub enabled: bool,
    pub port: u16,
    pub url: String,
    pub auto_detected: bool,
}

pub fn get_github_proxy_url(store: &SkillStore) -> Result<String> {
    Ok(get_github_proxy_config(store)?.url)
}

pub fn get_github_proxy_config(store: &SkillStore) -> Result<GithubProxyConfig> {
    Ok(match store.get_setting(GITHUB_PROXY_URL_KEY)? {
        Some(value) => config_from_saved_url(&normalize_proxy_url(&value), false),
        None => {
            let url = auto_detect_github_proxy_url();
            config_from_saved_url(&url, !url.is_empty())
        }
    })
}

pub fn set_github_proxy_url(store: &SkillStore, proxy_url: &str) -> Result<String> {
    let normalized = normalize_proxy_url(proxy_url);
    if !normalized.is_empty() {
        validate_proxy_url(&normalized)?;
    }
    store.set_setting(GITHUB_PROXY_URL_KEY, &normalized)?;
    Ok(normalized)
}

pub fn set_github_proxy_config(
    store: &SkillStore,
    enabled: bool,
    port: u16,
) -> Result<GithubProxyConfig> {
    let normalized_port = if port == 0 {
        DEFAULT_GITHUB_PROXY_PORT
    } else {
        port
    };
    let url = if enabled {
        format!("http://{}:{}", DEFAULT_GITHUB_PROXY_HOST, normalized_port)
    } else {
        String::new()
    };
    if !url.is_empty() {
        validate_proxy_url(&url)?;
    }
    store.set_setting(GITHUB_PROXY_URL_KEY, &url)?;
    Ok(config_from_saved_url(&url, false))
}

pub fn auto_detect_github_proxy_url() -> String {
    if local_tcp_port_is_open(
        DEFAULT_GITHUB_PROXY_HOST,
        DEFAULT_GITHUB_PROXY_PORT,
        Duration::from_millis(200),
    ) {
        DEFAULT_GITHUB_PROXY_URL.to_string()
    } else {
        String::new()
    }
}

pub fn app_http_client(proxy_url: &str, timeout_secs: Option<u64>) -> Result<Client> {
    http_client_builder(proxy_url, timeout_secs)?
        .build()
        .context("build HTTP client")
}

pub fn github_http_client_no_redirects(
    proxy_url: &str,
    timeout_secs: Option<u64>,
) -> Result<Client> {
    app_http_client_no_redirects(proxy_url, timeout_secs)
}

pub fn app_http_client_no_redirects(proxy_url: &str, timeout_secs: Option<u64>) -> Result<Client> {
    http_client_builder(proxy_url, timeout_secs)?
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("build HTTP client")
}

fn download_redirect_allowed(url: &str, previous: usize) -> bool {
    reqwest::Url::parse(url).is_ok_and(|url| {
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && previous <= 5
    })
}

pub fn app_download_client(proxy_url: Option<&str>, timeout_secs: u64) -> Result<Client> {
    http_client_builder(proxy_url.unwrap_or(""), Some(timeout_secs))?
        .https_only(true)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if download_redirect_allowed(attempt.url().as_str(), attempt.previous().len()) {
                attempt.follow()
            } else {
                attempt.error("CLI_DOWNLOAD_REDIRECT_REJECTED")
            }
        }))
        .build()
        .context("build CLI download client")
}

fn http_client_builder(proxy_url: &str, timeout_secs: Option<u64>) -> Result<ClientBuilder> {
    let mut builder = ClientBuilder::new().no_proxy();
    if let Some(secs) = timeout_secs {
        builder = builder.timeout(std::time::Duration::from_secs(secs));
    }
    let proxy_url = proxy_url.trim();
    if !proxy_url.is_empty() {
        builder = builder.proxy(
            reqwest::Proxy::all(proxy_url)
                .with_context(|| format!("invalid proxy URL: {}", proxy_url))?,
        );
    }
    Ok(builder)
}

pub fn github_http_client(proxy_url: &str, timeout_secs: Option<u64>) -> Result<Client> {
    app_http_client(proxy_url, timeout_secs)
}

pub fn git_fetch_options<'a>(proxy_url: &str) -> FetchOptions<'a> {
    let mut options = FetchOptions::new();
    options.proxy_options(git_proxy_options(proxy_url));
    options
}

pub fn git_push_options<'a>(proxy_url: &str) -> PushOptions<'a> {
    let mut options = PushOptions::new();
    options.proxy_options(git_proxy_options(proxy_url));
    options
}

fn git_proxy_options(proxy_url: &str) -> ProxyOptions<'static> {
    let mut options = ProxyOptions::new();
    let proxy_url = proxy_url.trim();
    if !proxy_url.is_empty() {
        options.url(proxy_url);
    }
    options
}

pub fn normalize_proxy_url(proxy_url: &str) -> String {
    proxy_url.trim().to_string()
}

fn config_from_saved_url(url: &str, auto_detected: bool) -> GithubProxyConfig {
    let url = normalize_proxy_url(url);
    let port = proxy_port_from_url(&url).unwrap_or(DEFAULT_GITHUB_PROXY_PORT);
    GithubProxyConfig {
        enabled: !url.is_empty(),
        port,
        url,
        auto_detected,
    }
}

fn proxy_port_from_url(url: &str) -> Option<u16> {
    url.rsplit(':')
        .next()
        .and_then(|port| port.trim_end_matches('/').parse::<u16>().ok())
}

fn validate_proxy_url(proxy_url: &str) -> Result<()> {
    reqwest::Proxy::all(proxy_url)
        .map(|_| ())
        .with_context(|| format!("invalid proxy URL: {}", proxy_url))
}

fn local_tcp_port_is_open(host: &str, port: u16, timeout: Duration) -> bool {
    let Ok(addr) = format!("{}:{}", host, port).parse::<SocketAddr>() else {
        return false;
    };
    TcpStream::connect_timeout(&addr, timeout).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::skill_store::SkillStore;
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Mutex;

    #[test]
    fn cli_download_redirects_reject_downgrades_and_excess_hops() {
        assert!(download_redirect_allowed(
            "https://release-assets.githubusercontent.com/file",
            5
        ));
        assert!(!download_redirect_allowed(
            "http://release-assets.githubusercontent.com/file",
            1
        ));
        assert!(!download_redirect_allowed("https://github.com/file", 6));
        assert!(!download_redirect_allowed(
            "https://user:secret@github.com/file",
            1
        ));
    }

    static PROXY_ENV_LOCK: Mutex<()> = Mutex::new(());

    struct RestoredEnvironment(HashMap<&'static str, Option<OsString>>);

    impl RestoredEnvironment {
        fn replace(values: &[(&'static str, Option<&str>)]) -> Self {
            let previous = values
                .iter()
                .map(|(name, value)| {
                    let previous = std::env::var_os(name);
                    if let Some(value) = value {
                        std::env::set_var(name, value);
                    } else {
                        std::env::remove_var(name);
                    }
                    (*name, previous)
                })
                .collect();
            Self(previous)
        }
    }

    impl Drop for RestoredEnvironment {
        fn drop(&mut self) {
            for (name, value) in self.0.drain() {
                if let Some(value) = value {
                    std::env::set_var(name, value);
                } else {
                    std::env::remove_var(name);
                }
            }
        }
    }

    #[test]
    fn empty_github_proxy_disables_proxy() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkillStore::new(dir.path().join("db.sqlite"));
        store.ensure_schema().unwrap();

        let saved = set_github_proxy_url(&store, "  ").unwrap();

        assert_eq!(saved, "");
        assert_eq!(get_github_proxy_url(&store).unwrap(), "");
    }

    #[test]
    fn empty_application_proxy_ignores_inherited_proxy_environment() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
        });
        let client = {
            let _lock = PROXY_ENV_LOCK.lock().unwrap();
            let _environment = RestoredEnvironment::replace(&[
                ("HTTP_PROXY", Some("http://127.0.0.1:9")),
                ("HTTPS_PROXY", Some("http://127.0.0.1:9")),
                ("ALL_PROXY", Some("http://127.0.0.1:9")),
                ("NO_PROXY", Some("")),
                ("http_proxy", Some("http://127.0.0.1:9")),
                ("https_proxy", Some("http://127.0.0.1:9")),
                ("all_proxy", Some("http://127.0.0.1:9")),
                ("no_proxy", Some("")),
            ]);
            app_http_client("", Some(5)).unwrap()
        };

        let response = client.get(format!("http://{address}/health")).send();

        assert_eq!(response.unwrap().text().unwrap(), "ok");
        server.join().unwrap();
    }

    #[test]
    fn proxy_config_disable_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkillStore::new(dir.path().join("db.sqlite"));
        store.ensure_schema().unwrap();

        let saved = set_github_proxy_config(&store, false, 7890).unwrap();

        assert!(!saved.enabled);
        assert_eq!(saved.port, DEFAULT_GITHUB_PROXY_PORT);
        assert_eq!(saved.url, "");
        assert!(!get_github_proxy_config(&store).unwrap().enabled);
    }

    #[test]
    fn proxy_config_uses_localhost_port() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkillStore::new(dir.path().join("db.sqlite"));
        store.ensure_schema().unwrap();

        let saved = set_github_proxy_config(&store, true, 7897).unwrap();

        assert!(saved.enabled);
        assert_eq!(saved.port, 7897);
        assert_eq!(saved.url, "http://127.0.0.1:7897");
        assert_eq!(get_github_proxy_url(&store).unwrap(), saved.url);
    }

    #[test]
    fn explicit_github_proxy_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkillStore::new(dir.path().join("db.sqlite"));
        store.ensure_schema().unwrap();

        let saved = set_github_proxy_url(&store, " http://127.0.0.1:7890 ").unwrap();

        assert_eq!(saved, DEFAULT_GITHUB_PROXY_URL);
        assert_eq!(
            get_github_proxy_url(&store).unwrap(),
            DEFAULT_GITHUB_PROXY_URL
        );
    }

    #[test]
    fn local_tcp_port_detector_sees_open_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        assert!(local_tcp_port_is_open(
            "127.0.0.1",
            port,
            Duration::from_millis(200)
        ));
    }
}
