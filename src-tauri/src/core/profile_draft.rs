use std::path::Path;
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::device_sync::credentials::CredentialStore;
use super::network_proxy::{app_http_client, get_github_proxy_url};
use super::skill_files;
use super::skill_store::{SkillProfileRecord, SkillStore};

const CONFIG_SETTING: &str = "profile_draft_config_v1";
const KEY_CONFIGURED_SETTING: &str = "profile_draft_key_configured_v1";
pub const PROFILE_DRAFT_CREDENTIAL_KEY: &str = "profile-draft-api-key-v1";

const KEYRING_SERVICE: &str = "com.skills-hub.profile-draft";

static PROFILE_DRAFT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

const ALLOWED_CATEGORIES: [&str; 12] = [
    "科研", "研发", "创意", "审查", "网页UI", "桌面", "办公", "设计", "运维", "工具", "写作", "金融",
];

const CATEGORY_COLORS: &[(&str, &str)] = &[
    ("科研", "#92400E"),
    ("研发", "#3B82F6"),
    ("创意", "#22D3EE"),
    ("审查", "#EF4444"),
    ("网页UI", "#EAB308"),
    ("桌面", "#7C3AED"),
    ("办公", "#22C55E"),
    ("设计", "#F97316"),
    ("运维", "#111827"),
    ("工具", "#A855F7"),
    ("写作", "#6B7280"),
    ("金融", "#F472B6"),
];

const SYSTEM_PROMPT: &str = r#"你是 Skills Hub 的中文资料员。根据用户给出的 Skill 仓库原文，只输出一个 JSON 对象，不要解释，不要 Markdown。
字段：
- zh_name：2–8 个汉字功能名，不要英文，不要标点堆砌
- category：只能是下列之一：科研、研发、创意、审查、网页UI、桌面、办公、设计、运维、工具、写作、金融
- summary：12–30 个汉字的中文功能简介（可对英文说明做英译汉），不要口号，不要重复英文 id
- note：可空字符串。只有原文明确写了密钥、付费、危险操作或硬依赖时，才用一句中文备注；否则 ""
禁止编造 GitHub 地址。不要输出除 JSON 以外的任何文字。"#;

fn lock_profile_draft() -> Result<std::sync::MutexGuard<'static, ()>> {
    PROFILE_DRAFT_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| anyhow!("profile draft credential lock is poisoned"))
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemProfileDraftKeyStore;

impl CredentialStore for SystemProfileDraftKeyStore {
    fn set(&self, key: &str, secret: &str) -> Result<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, key)
            .context("open profile draft credential store")?;
        entry
            .set_password(secret)
            .context("save profile draft API key")
    }

    fn get(&self, key: &str) -> Result<Option<String>> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, key)
            .context("open profile draft credential store")?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err).context("read profile draft API key"),
        }
    }

    fn delete(&self, key: &str) -> Result<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, key)
            .context("open profile draft credential store")?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err).context("delete profile draft API key"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileDraftConfig {
    pub provider: String,
    #[serde(default, alias = "baseUrl")]
    pub base_url: String,
    pub model: String,
    #[serde(default, alias = "autoOnInstall")]
    pub auto_on_install: bool,
}

impl Default for ProfileDraftConfig {
    fn default() -> Self {
        Self {
            provider: "custom".into(),
            base_url: String::new(),
            model: String::new(),
            auto_on_install: false,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ProfileDraftStatus {
    pub configured: bool,
    pub has_key: bool,
    pub config: ProfileDraftConfig,
    pub presets: Vec<ProfileDraftPreset>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProfileDraftPreset {
    pub id: String,
    pub label: String,
    pub base_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileDraftSuggestion {
    pub zh_name: String,
    pub category: String,
    pub color: String,
    pub summary: String,
    pub note: Option<String>,
    pub source_url: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProfileDraftModelItem {
    pub id: String,
}

fn presets() -> Vec<ProfileDraftPreset> {
    vec![
        ProfileDraftPreset {
            id: "deepseek".into(),
            label: "DeepSeek".into(),
            base_url: "https://api.deepseek.com/v1".into(),
        },
        ProfileDraftPreset {
            id: "siliconflow".into(),
            label: "硅基流动".into(),
            base_url: "https://api.siliconflow.cn/v1".into(),
        },
        ProfileDraftPreset {
            id: "openai".into(),
            label: "OpenAI".into(),
            base_url: "https://api.openai.com/v1".into(),
        },
        ProfileDraftPreset {
            id: "grok".into(),
            label: "Grok (xAI)".into(),
            base_url: "https://api.x.ai/v1".into(),
        },
        ProfileDraftPreset {
            id: "custom".into(),
            label: "自定义中转".into(),
            base_url: String::new(),
        },
    ]
}

fn normalize_base_url(raw: &str) -> String {
    raw.trim().trim_end_matches('/').to_string()
}

pub fn load_config(store: &SkillStore) -> Result<ProfileDraftConfig> {
    let Some(raw) = store.get_setting(CONFIG_SETTING)? else {
        return Ok(ProfileDraftConfig::default());
    };
    let mut cfg: ProfileDraftConfig = serde_json::from_str(&raw).unwrap_or_default();
    cfg.base_url = normalize_base_url(&cfg.base_url);
    cfg.provider = cfg.provider.trim().to_string();
    if cfg.provider.is_empty() {
        cfg.provider = "custom".into();
    }
    cfg.model = cfg.model.trim().to_string();
    cfg.auto_on_install = false; // product decision: always off
    Ok(cfg)
}

pub fn save_config(store: &SkillStore, mut cfg: ProfileDraftConfig) -> Result<ProfileDraftConfig> {
    cfg.base_url = normalize_base_url(&cfg.base_url);
    cfg.provider = cfg.provider.trim().to_string();
    if cfg.provider.is_empty() {
        cfg.provider = "custom".into();
    }
    cfg.model = cfg.model.trim().to_string();
    cfg.auto_on_install = false;
    store.set_setting(CONFIG_SETTING, &serde_json::to_string(&cfg)?)?;
    Ok(cfg)
}

pub fn resolve_api_key(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
) -> Result<Option<String>> {
    let _guard = lock_profile_draft()?;
    let token = credentials
        .get(PROFILE_DRAFT_CREDENTIAL_KEY)
        .context("read profile draft API key")?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if token.is_some() {
        store.set_setting(KEY_CONFIGURED_SETTING, "true")?;
    } else {
        let _ = store.delete_setting(KEY_CONFIGURED_SETTING);
    }
    Ok(token)
}

pub fn has_api_key(store: &SkillStore, credentials: &dyn CredentialStore) -> Result<bool> {
    if store
        .get_setting(KEY_CONFIGURED_SETTING)?
        .is_some_and(|value| value == "true")
    {
        return Ok(true);
    }
    Ok(resolve_api_key(store, credentials)?.is_some())
}

pub fn set_api_key(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
    key: &str,
) -> Result<()> {
    let _guard = lock_profile_draft()?;
    let key = key.trim();
    if key.is_empty() {
        credentials
            .delete(PROFILE_DRAFT_CREDENTIAL_KEY)
            .context("delete profile draft API key")?;
        let _ = store.delete_setting(KEY_CONFIGURED_SETTING);
    } else {
        credentials
            .set(PROFILE_DRAFT_CREDENTIAL_KEY, key)
            .context("save profile draft API key")?;
        store.set_setting(KEY_CONFIGURED_SETTING, "true")?;
    }
    Ok(())
}

pub fn status(store: &SkillStore, credentials: &dyn CredentialStore) -> Result<ProfileDraftStatus> {
    let config = load_config(store)?;
    let has_key = has_api_key(store, credentials)?;
    let configured = has_key && !config.base_url.trim().is_empty() && !config.model.trim().is_empty();
    Ok(ProfileDraftStatus {
        configured,
        has_key,
        config,
        presets: presets(),
    })
}

fn color_for_category(category: &str) -> String {
    CATEGORY_COLORS
        .iter()
        .find(|(name, _)| *name == category)
        .map(|(_, color)| (*color).to_string())
        .unwrap_or_else(|| "#3B82F6".into())
}

fn clip_chars(input: &str, max: usize) -> String {
    input.chars().take(max).collect()
}

fn count_chars(input: &str) -> usize {
    input.chars().count()
}

fn extract_github_url(text: &str) -> Option<String> {
    for token in text.split_whitespace() {
        let cleaned = token.trim_matches(|c: char| {
            matches!(c, '(' | ')' | '[' | ']' | '"' | '\'' | '`' | ',' | '.' | ';')
        });
        if cleaned.starts_with("https://github.com/") {
            let parts: Vec<_> = cleaned.split('/').collect();
            if parts.len() >= 5 {
                return Some(format!("https://github.com/{}/{}", parts[3], parts[4]));
            }
        }
        if let Some(rest) = cleaned.strip_prefix("github.com/") {
            let parts: Vec<_> = rest.split('/').collect();
            if parts.len() >= 2 {
                return Some(format!("https://github.com/{}/{}", parts[0], parts[1]));
            }
        }
    }
    None
}

fn read_skill_context(central_path: &Path) -> Result<(String, Option<String>, Option<String>)> {
    let skill_md = skill_files::read_file(central_path, "SKILL.md")
        .or_else(|_| skill_files::read_file(central_path, "skill.md"))
        .unwrap_or_default();
    let readme = skill_files::read_file(central_path, "README.md")
        .or_else(|_| skill_files::read_file(central_path, "readme.md"))
        .ok();
    let blob = format!(
        "{}\n{}",
        skill_md,
        readme.clone().unwrap_or_default()
    );
    let source_url = extract_github_url(&blob);
    Ok((clip_chars(&skill_md, 4000), readme.map(|r| clip_chars(&r, 2000)), source_url))
}

fn parse_model_message_content(content: &Value) -> Result<String> {
    if let Some(text) = content.as_str() {
        return Ok(text.to_string());
    }
    if let Some(arr) = content.as_array() {
        let mut out = String::new();
        for item in arr {
            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                out.push_str(text);
            } else if let Some(text) = item.as_str() {
                out.push_str(text);
            }
        }
        if !out.is_empty() {
            return Ok(out);
        }
    }
    bail!("model response missing text content");
}

fn extract_json_object(raw: &str) -> Result<Value> {
    let trimmed = raw.trim();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Ok(value);
    }
    let start = trimmed
        .find('{')
        .ok_or_else(|| anyhow!("model did not return JSON object"))?;
    let end = trimmed
        .rfind('}')
        .ok_or_else(|| anyhow!("model did not return JSON object"))?;
    serde_json::from_str(&trimmed[start..=end]).context("parse model JSON")
}

fn sanitize_suggestion(
    value: &Value,
    fallback_source_url: Option<String>,
) -> Result<ProfileDraftSuggestion> {
    let mut zh_name = value
        .get("zh_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let category = value
        .get("category")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let mut summary = value
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let note = value
        .get("note")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| clip_chars(v, 80));

    if zh_name.is_empty() {
        bail!("模型返回的中文名不可用");
    }
    if count_chars(&zh_name) > 8 {
        zh_name = clip_chars(&zh_name, 8);
    }
    if !ALLOWED_CATEGORIES.iter().any(|item| *item == category) {
        bail!("模型返回的分类不在十二套内: {category}");
    }
    if summary.is_empty() {
        bail!("模型返回的简介为空");
    }
    if count_chars(&summary) > 30 {
        summary = clip_chars(&summary, 30);
    }
    // Reject ASCII-only summaries: Hub five-elements require Chinese.
    if summary.chars().all(|ch| ch.is_ascii()) {
        bail!("模型返回的简介不是中文");
    }

    Ok(ProfileDraftSuggestion {
        zh_name: clip_chars(&zh_name, 8),
        category: category.clone(),
        color: color_for_category(&category),
        summary,
        note,
        source_url: fallback_source_url,
    })
}

fn chat_completions(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
    user_content: &str,
) -> Result<String> {
    let cfg = load_config(store)?;
    if cfg.base_url.trim().is_empty() {
        bail!("请先在设置里填写 API Base URL");
    }
    if cfg.model.trim().is_empty() {
        bail!("请先选择或填写模型");
    }
    let api_key = resolve_api_key(store, credentials)?.ok_or_else(|| anyhow!("请先保存 API Key"))?;
    let proxy_url = get_github_proxy_url(store).unwrap_or_default();
    let client = app_http_client(&proxy_url, Some(90))?;
    let url = format!("{}/chat/completions", normalize_base_url(&cfg.base_url));
    let body = json!({
        "model": cfg.model,
        "temperature": 0.2,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": user_content},
        ],
    });
    let response = client
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .context("call chat/completions")?
        .error_for_status()
        .context("chat/completions HTTP error")?;
    let payload: Value = response.json().context("parse chat/completions JSON")?;
    let content = payload
        .pointer("/choices/0/message/content")
        .ok_or_else(|| anyhow!("chat/completions missing choices"))?;
    parse_model_message_content(content)
}

pub fn list_models(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
) -> Result<Vec<ProfileDraftModelItem>> {
    let cfg = load_config(store)?;
    if cfg.base_url.trim().is_empty() {
        bail!("请先在设置里填写 API Base URL");
    }
    let api_key = resolve_api_key(store, credentials)?.ok_or_else(|| anyhow!("请先保存 API Key"))?;
    let proxy_url = get_github_proxy_url(store).unwrap_or_default();
    let client = app_http_client(&proxy_url, Some(45))?;
    let url = format!("{}/models", normalize_base_url(&cfg.base_url));
    let response = client
        .get(url)
        .bearer_auth(api_key)
        .send()
        .context("call /models")?
        .error_for_status()
        .context("/models HTTP error")?;
    let payload: Value = response.json().context("parse /models JSON")?;
    let mut models = Vec::new();
    if let Some(arr) = payload.get("data").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                let id = id.trim();
                if !id.is_empty() {
                    models.push(ProfileDraftModelItem { id: id.to_string() });
                }
            }
        }
    }
    models.sort_by(|a, b| a.id.to_lowercase().cmp(&b.id.to_lowercase()));
    models.dedup_by(|a, b| a.id == b.id);
    if models.is_empty() {
        bail!("未获取到模型列表，可手填模型 ID");
    }
    Ok(models)
}

pub fn test_connection(store: &SkillStore, credentials: &dyn CredentialStore) -> Result<String> {
    let raw = chat_completions(
        store,
        credentials,
        "只回复 JSON：{\"ok\":true}",
    )?;
    Ok(clip_chars(raw.trim(), 200))
}

pub fn draft_for_skill(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
    skill_id: &str,
) -> Result<ProfileDraftSuggestion> {
    let skill = store
        .get_skill_by_id(skill_id)?
        .ok_or_else(|| anyhow!("skill not found: {skill_id}"))?;
    let existing = store.get_skill_profile(skill_id)?;
    let (skill_md, readme, extracted_url) = read_skill_context(Path::new(&skill.central_path))?;
    let existing_url = existing
        .as_ref()
        .and_then(|p| p.source_url.clone())
        .filter(|v| !v.trim().is_empty());
    let source_url = existing_url.or(extracted_url);

    let user = format!(
        "英文调用名: {}\n已有 GitHub: {}\n\nSKILL.md:\n{}\n\nREADME:\n{}\n",
        skill.name,
        source_url.clone().unwrap_or_else(|| "(无)".into()),
        skill_md,
        readme.unwrap_or_else(|| "(无)".into())
    );
    let raw = chat_completions(store, credentials, &user)?;
    let value = extract_json_object(&raw)?;
    sanitize_suggestion(&value, source_url)
}

/// Fill only empty Hub profile fields. Never overwrite filled zh_name / summary / note / category / source_url.
pub fn apply_suggestion_fill_empty(
    store: &SkillStore,
    skill_id: &str,
    suggestion: &ProfileDraftSuggestion,
) -> Result<SkillProfileRecord> {
    let existing = store.get_skill_profile(skill_id)?;
    let sort_order = existing.as_ref().map(|p| p.sort_order).unwrap_or(0);
    let zh_name = existing
        .as_ref()
        .and_then(|p| p.zh_name.clone())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| suggestion.zh_name.clone());
    let category = existing
        .as_ref()
        .and_then(|p| p.category.clone())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| suggestion.category.clone());
    let summary = existing
        .as_ref()
        .and_then(|p| p.summary.clone())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| suggestion.summary.clone());
    let note = existing
        .as_ref()
        .and_then(|p| p.note.clone())
        .filter(|v| !v.trim().is_empty())
        .or_else(|| suggestion.note.clone());
    let source_url = existing
        .as_ref()
        .and_then(|p| p.source_url.clone())
        .filter(|v| !v.trim().is_empty())
        .or_else(|| suggestion.source_url.clone());
    let color = existing
        .as_ref()
        .and_then(|p| p.color.clone())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| color_for_category(&category));
    let summary_source = if existing
        .as_ref()
        .and_then(|p| p.summary.as_deref())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .is_some()
    {
        existing
            .as_ref()
            .map(|p| p.summary_source.clone())
            .unwrap_or_else(|| "manual".into())
    } else {
        "auto".into()
    };

    store.upsert_skill_profile(&SkillProfileRecord {
        skill_id: skill_id.to_string(),
        zh_name: Some(zh_name),
        category: Some(category),
        color: Some(color),
        summary: Some(summary),
        note,
        source_url,
        summary_source,
        sort_order,
        created_at: 0,
        updated_at: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_chinese_fields_and_clips_summary() {
        let value = json!({
            "zh_name": "自然润色助手超长名字会被裁",
            "category": "科研",
            "summary": "这是一段明显超过三十个汉字的功能简介用来测试裁剪是否生效一二三四五六七八九十",
            "note": ""
        });
        let suggestion = sanitize_suggestion(&value, Some("https://github.com/a/b".into())).unwrap();
        assert_eq!(suggestion.category, "科研");
        assert_eq!(suggestion.color, "#92400E");
        assert!(count_chars(&suggestion.zh_name) <= 8);
        assert!(count_chars(&suggestion.summary) <= 30);
        assert_eq!(suggestion.source_url.as_deref(), Some("https://github.com/a/b"));
    }

    #[test]
    fn rejects_unknown_category() {
        let value = json!({
            "zh_name": "测试",
            "category": "学术写作",
            "summary": "这是一段合格的中文功能简介啊",
            "note": ""
        });
        assert!(sanitize_suggestion(&value, None).is_err());
    }

    #[test]
    fn extracts_github_owner_repo() {
        let text = "install via npx skills add blader/humanizer see https://github.com/blader/humanizer/tree/main";
        assert_eq!(
            extract_github_url(text).as_deref(),
            Some("https://github.com/blader/humanizer")
        );
    }

    #[test]
    fn fill_empty_keeps_existing_chinese_fields() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = SkillStore::new(dir.path().join("test.db"));
        store.ensure_schema().expect("schema");
        store
            .upsert_skill(&crate::core::skill_store::SkillRecord {
                id: "s1".into(),
                name: "nature-polishing".into(),
                description: None,
                source_type: "local".into(),
                source_ref: Some("/tmp/s1".into()),
                source_subpath: None,
                source_revision: None,
                central_path: "/tmp/s1".into(),
                content_hash: None,
                created_at: 1,
                updated_at: 1,
                last_sync_at: None,
                last_seen_at: 1,
                enabled: true,
                status: "ok".into(),
            })
            .unwrap();
        store
            .upsert_skill_profile(&SkillProfileRecord {
                skill_id: "s1".into(),
                zh_name: Some("自然润色".into()),
                category: Some("科研".into()),
                color: Some("#92400E".into()),
                summary: Some("润色学术英文段落与结构。".into()),
                note: None,
                source_url: None,
                summary_source: "manual".into(),
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();

        let suggestion = ProfileDraftSuggestion {
            zh_name: "覆盖名".into(),
            category: "写作".into(),
            color: "#6B7280".into(),
            summary: "这是模型想覆盖的中文简介啊".into(),
            note: Some("需要 API Key".into()),
            source_url: Some("https://github.com/a/b".into()),
        };
        let saved = apply_suggestion_fill_empty(&store, "s1", &suggestion).unwrap();
        assert_eq!(saved.zh_name.as_deref(), Some("自然润色"));
        assert_eq!(saved.category.as_deref(), Some("科研"));
        assert_eq!(saved.summary.as_deref(), Some("润色学术英文段落与结构。"));
        assert_eq!(saved.summary_source, "manual");
        assert_eq!(saved.note.as_deref(), Some("需要 API Key"));
        assert_eq!(saved.source_url.as_deref(), Some("https://github.com/a/b"));
        assert_eq!(saved.color.as_deref(), Some("#92400E"));
    }

    #[test]
    fn config_accepts_snake_case_and_legacy_camel_case() {
        let snake: ProfileDraftConfig = serde_json::from_str(
            r#"{"provider":"custom","base_url":"https://api.tyas.cc/v1","model":"gpt-4o","auto_on_install":false}"#,
        )
        .unwrap();
        assert_eq!(snake.base_url, "https://api.tyas.cc/v1");
        let encoded = serde_json::to_string(&snake).unwrap();
        assert!(encoded.contains("base_url"));
        assert!(!encoded.contains("baseUrl"));

        let camel: ProfileDraftConfig = serde_json::from_str(
            r#"{"provider":"openai","baseUrl":"https://api.openai.com/v1","model":"gpt-4o","autoOnInstall":false}"#,
        )
        .unwrap();
        assert_eq!(camel.base_url, "https://api.openai.com/v1");
    }
}
