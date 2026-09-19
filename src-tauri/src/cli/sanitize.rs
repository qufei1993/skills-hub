use serde_json::{Map, Value};

const REDACTED: &str = "[REDACTED]";
const URL_SCHEMES: [&str; 5] = ["https://", "http://", "ssh://", "git://", "ftp://"];

pub fn sanitize_payload(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(sanitize_object(object)),
        Value::Array(values) => Value::Array(values.iter().map(sanitize_payload).collect()),
        Value::String(value) => Value::String(sanitize_string(value)),
        Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
    }
}

fn sanitize_object(object: &Map<String, Value>) -> Map<String, Value> {
    object
        .iter()
        .map(|(key, value)| {
            let value = if is_sensitive_key(key) {
                Value::String(REDACTED.to_string())
            } else {
                sanitize_payload(value)
            };
            (key.clone(), value)
        })
        .collect()
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    normalized == "code"
        || normalized.contains("token")
        || normalized.contains("password")
        || normalized.contains("passwd")
        || normalized.contains("secret")
        || normalized.contains("privatekey")
        || normalized.contains("authorization")
        || normalized.contains("credential")
        || normalized.contains("cookie")
        || normalized.contains("apikey")
        || normalized.contains("oauthcode")
}

fn sanitize_string(value: &str) -> String {
    if let Some(url) = sanitize_complete_url(value) {
        return url;
    }

    let Some(with_safe_urls) = sanitize_embedded_urls(value) else {
        return REDACTED.to_string();
    };
    if contains_secret_material(&with_safe_urls) {
        REDACTED.to_string()
    } else {
        with_safe_urls
    }
}

fn sanitize_complete_url(value: &str) -> Option<String> {
    let mut url = reqwest::Url::parse(value).ok()?;
    if !URL_SCHEMES.iter().any(|scheme| {
        url.scheme()
            .eq_ignore_ascii_case(scheme.trim_end_matches("://"))
    }) {
        return None;
    }
    sanitize_url(&mut url)?;
    Some(url.to_string())
}

fn sanitize_embedded_urls(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;

    while let Some((start, _)) = next_url(&lower, cursor) {
        output.push_str(&value[cursor..start]);
        let end = value[start..]
            .char_indices()
            .find_map(|(offset, character)| {
                (offset > 0 && is_url_boundary(character)).then_some(start + offset)
            })
            .unwrap_or(value.len());
        let raw_url = &value[start..end];
        let mut url = match reqwest::Url::parse(raw_url) {
            Ok(url) => url,
            Err(_) if looks_credential_bearing(raw_url) => return None,
            Err(_) => {
                output.push_str(raw_url);
                cursor = end;
                continue;
            }
        };
        sanitize_url(&mut url)?;
        output.push_str(url.as_str());
        cursor = end;
    }
    output.push_str(&value[cursor..]);
    Some(output)
}

fn next_url(value: &str, from: usize) -> Option<(usize, &'static str)> {
    URL_SCHEMES
        .iter()
        .filter_map(|scheme| {
            value[from..]
                .find(scheme)
                .map(|offset| (from + offset, *scheme))
        })
        .min_by_key(|(offset, _)| *offset)
}

fn is_url_boundary(character: char) -> bool {
    character.is_whitespace()
        || matches!(
            character,
            '"' | '\'' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
        )
}

fn sanitize_url(url: &mut reqwest::Url) -> Option<()> {
    if url.password().is_some() {
        url.set_password(None).ok()?;
    }
    if !url.username().is_empty() {
        url.set_username("").ok()?;
    }
    url.set_query(None);
    url.set_fragment(None);
    Some(())
}

fn looks_credential_bearing(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains('@')
        || lower.contains('?')
        || lower.contains('#')
        || contains_secret_material(&lower)
}

fn contains_secret_material(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "bearer ",
        "basic ",
        "access_token=",
        "access-token=",
        "token=",
        "token:",
        "refresh_token=",
        "refresh-token=",
        "oauth_code=",
        "oauth-code=",
        "code=",
        "password=",
        "password:",
        "passwd=",
        "secret=",
        "secret:",
        "client_secret=",
        "client-secret=",
        "private_key=",
        "private-key=",
        "api_key=",
        "api-key=",
        "apikey=",
        "authorization:",
        "cookie:",
        "set-cookie:",
        "-----begin private key-----",
        "-----begin rsa private key-----",
        "-----begin openssh private key-----",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
        || lower
            .split(|character: char| {
                character.is_whitespace() || matches!(character, '"' | '\'' | ',' | ';' | '(' | ')')
            })
            .any(looks_like_secret_token)
}

fn looks_like_secret_token(value: &str) -> bool {
    let value = value.trim_matches(|character: char| {
        !character.is_ascii_alphanumeric() && character != '_' && character != '-'
    });
    (value.starts_with("ghp_") && value.len() > 12)
        || (value.starts_with("github_pat_") && value.len() > 20)
        || (value.starts_with("glpat-") && value.len() > 12)
        || (value.starts_with("sk-") && value.len() > 12)
        || (value.starts_with("eyJ") && value.matches('.').count() == 2)
}
