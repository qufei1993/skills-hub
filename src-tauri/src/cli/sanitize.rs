use serde_json::{Map, Value};

const REDACTED: &str = "[REDACTED]";

pub fn sanitize_payload(value: &Value) -> Value {
    sanitize_value(value, false)
}

fn sanitize_value(value: &Value, inherited_auth_context: bool) -> Value {
    match value {
        Value::Object(object) => Value::Object(sanitize_object(object, inherited_auth_context)),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| sanitize_value(value, inherited_auth_context))
                .collect(),
        ),
        Value::String(value) => Value::String(sanitize_string(value)),
        Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
    }
}

fn sanitize_object(
    object: &Map<String, Value>,
    inherited_auth_context: bool,
) -> Map<String, Value> {
    let object_auth_context = inherited_auth_context || object_indicates_auth_context(object);
    object
        .iter()
        .map(|(key, value)| {
            let normalized = normalize_key(key);
            let value =
                if is_sensitive_key(&normalized) || (normalized == "code" && object_auth_context) {
                    Value::String(REDACTED.to_string())
                } else {
                    sanitize_value(value, is_auth_context_container(&normalized))
                };
            (key.clone(), value)
        })
        .collect()
}

fn normalize_key(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_sensitive_key(normalized: &str) -> bool {
    matches!(
        normalized,
        "token"
            | "accesstoken"
            | "refreshtoken"
            | "idtoken"
            | "authtoken"
            | "bearertoken"
            | "sessiontoken"
            | "apikey"
            | "password"
            | "passwd"
            | "secret"
            | "clientsecret"
            | "privatekey"
            | "authorization"
            | "proxyauthorization"
            | "cookie"
            | "setcookie"
            | "oauthcode"
            | "authcode"
            | "authorizationcode"
            | "codeverifier"
            | "credential"
            | "credentials"
    )
}

fn is_auth_context_container(normalized: &str) -> bool {
    matches!(
        normalized,
        "auth"
            | "oauth"
            | "authentication"
            | "oauthresponse"
            | "authorizationresponse"
            | "tokenresponse"
    )
}

fn object_indicates_auth_context(object: &Map<String, Value>) -> bool {
    object.iter().any(|(key, value)| {
        let normalized = normalize_key(key);
        is_sensitive_key(&normalized)
            || matches!(
                normalized.as_str(),
                "granttype"
                    | "clientid"
                    | "codechallenge"
                    | "codechallengemethod"
                    | "redirecturi"
                    | "oauthflow"
                    | "tokenendpoint"
                    | "authorizationendpoint"
            )
            || (normalized == "oauth" && !value.is_object() && !value.is_array())
    })
}

fn sanitize_string(value: &str) -> String {
    if let Ok(mut url) = reqwest::Url::parse(value) {
        if has_sensitive_url_components(&url) {
            return sanitize_url(&mut url)
                .map(|()| url.to_string())
                .unwrap_or_else(|| REDACTED.to_string());
        }
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

fn sanitize_embedded_urls(value: &str) -> Option<String> {
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;

    while let Some(start) = next_url_start(value, cursor) {
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
            Err(_) if looks_credential_bearing_url(raw_url) => return None,
            Err(_) => {
                output.push_str(raw_url);
                cursor = end;
                continue;
            }
        };
        if has_sensitive_url_components(&url) {
            sanitize_url(&mut url)?;
            output.push_str(url.as_str());
        } else {
            output.push_str(raw_url);
        }
        cursor = end;
    }
    output.push_str(&value[cursor..]);
    Some(output)
}

fn next_url_start(value: &str, from: usize) -> Option<usize> {
    let bytes = value.as_bytes();
    let mut search_from = from;
    while search_from + 3 <= bytes.len() {
        let relative = value[search_from..].find("://")?;
        let separator = search_from + relative;
        let mut start = separator;
        while start > from && is_scheme_character(bytes[start - 1]) {
            start -= 1;
        }
        if start < separator && bytes[start].is_ascii_alphabetic() {
            return Some(start);
        }
        search_from = separator + 3;
    }
    None
}

fn is_scheme_character(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.')
}

fn is_url_boundary(character: char) -> bool {
    character.is_whitespace()
        || matches!(
            character,
            '"' | '\'' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
        )
}

fn has_sensitive_url_components(url: &reqwest::Url) -> bool {
    !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
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

fn looks_credential_bearing_url(value: &str) -> bool {
    value.contains("://") && (value.contains('@') || value.contains('?') || value.contains('#'))
}

fn contains_secret_material(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "access_token=",
        "access-token=",
        "token=",
        "token:",
        "refresh_token=",
        "refresh-token=",
        "oauth_code=",
        "oauth-code=",
        "authorization_code=",
        "authorization-code=",
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
        "cookie:",
        "set-cookie:",
        "-----begin private key-----",
        "-----begin rsa private key-----",
        "-----begin openssh private key-----",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
        || contains_authorization_credential(value)
        || value
            .split(|character: char| {
                character.is_ascii_whitespace()
                    || matches!(character, '"' | '\'' | ',' | ';' | '(' | ')')
            })
            .any(looks_like_secret_token)
}

fn contains_authorization_credential(value: &str) -> bool {
    contains_scheme_credential(value, "bearer")
        || contains_scheme_credential(value, "basic")
        || contains_authorization_assignment(value)
}

fn contains_scheme_credential(value: &str, scheme: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let mut from = 0;
    while let Some(relative) = lower[from..].find(scheme) {
        let start = from + relative;
        let end = start + scheme.len();
        let starts_at_boundary = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphanumeric();
        let ends_at_boundary = end == lower.len() || !lower.as_bytes()[end].is_ascii_alphanumeric();
        if starts_at_boundary && ends_at_boundary {
            let mut cursor = end;
            let mut has_separator = false;
            while cursor < value.len() && value.as_bytes()[cursor].is_ascii_whitespace() {
                cursor += 1;
                has_separator = true;
            }
            if cursor < value.len() && matches!(value.as_bytes()[cursor], b':' | b'=') {
                cursor += 1;
                has_separator = true;
                while cursor < value.len() && value.as_bytes()[cursor].is_ascii_whitespace() {
                    cursor += 1;
                }
            }
            if has_separator {
                let candidate = next_credential_token(&value[cursor..]);
                if looks_like_auth_credential(candidate) {
                    return true;
                }
            }
        }
        from = end;
    }
    false
}

fn contains_authorization_assignment(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let mut from = 0;
    const LABEL: &str = "authorization";
    while let Some(relative) = lower[from..].find(LABEL) {
        let start = from + relative;
        let mut cursor = start + LABEL.len();
        let starts_at_boundary = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphanumeric();
        while cursor < value.len() && value.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if starts_at_boundary
            && cursor < value.len()
            && matches!(value.as_bytes()[cursor], b':' | b'=')
        {
            cursor += 1;
            while cursor < value.len() && value.as_bytes()[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            let candidate = next_credential_token(&value[cursor..]);
            if looks_like_auth_credential(candidate)
                || contains_scheme_credential(&value[cursor..], "bearer")
                || contains_scheme_credential(&value[cursor..], "basic")
            {
                return true;
            }
        }
        from = start + LABEL.len();
    }
    false
}

fn next_credential_token(value: &str) -> &str {
    value
        .split(|character: char| {
            character.is_ascii_whitespace()
                || matches!(character, '"' | '\'' | ',' | ';' | '(' | ')')
        })
        .next()
        .unwrap_or_default()
}

fn looks_like_auth_credential(value: &str) -> bool {
    let value = trim_token_punctuation(value);
    let has_upper = value.bytes().any(|byte| byte.is_ascii_uppercase());
    let has_lower = value.bytes().any(|byte| byte.is_ascii_lowercase());
    value.eq_ignore_ascii_case("secret")
        || looks_like_secret_token(value)
        || value.len() >= 16
        || (value.len() >= 8
            && (value.bytes().any(|byte| byte.is_ascii_digit())
                || value
                    .bytes()
                    .any(|byte| matches!(byte, b'_' | b'-' | b'+' | b'=' | b'.' | b'/'))
                || (has_upper && has_lower)))
}

fn looks_like_secret_token(value: &str) -> bool {
    let value = trim_token_punctuation(value);
    let lower = value.to_ascii_lowercase();
    (lower.starts_with("ghp_") && value.len() > 12)
        || (lower.starts_with("github_pat_") && value.len() > 20)
        || (lower.starts_with("glpat-") && value.len() > 12)
        || (lower.starts_with("sk-") && value.len() > 12)
        || (lower.starts_with("eyj") && value.matches('.').count() == 2)
}

fn trim_token_punctuation(value: &str) -> &str {
    value.trim_matches(|character: char| {
        !character.is_ascii_alphanumeric()
            && character != '_'
            && character != '-'
            && character != '.'
            && character != '+'
            && character != '='
            && character != '/'
    })
}
