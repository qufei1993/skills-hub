use serde::{Deserialize, Serialize};
const MAX_COLLECTION_SKILLS: usize = 2000;
const MAX_COLLECTION_SOURCES: usize = 100;
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionSource {
    pub repo: String,
    #[serde(rename = "ref")]
    pub revision: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionSkill {
    pub name: String,
    pub path: String,
    pub source: usize,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionManifest {
    pub v: u8,
    pub title: String,
    pub sources: Vec<CollectionSource>,
    pub skills: Vec<CollectionSkill>,
}
fn safe_segment(value: &str) -> bool {
    !value.is_empty()
        && !matches!(value, "." | "..")
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
pub fn parse_link(link: &str) -> Result<CollectionManifest, &'static str> {
    let invalid = "COLLECTION_LINK_INVALID";
    if link.len() > 30000 {
        return Err("COLLECTION_LINK_TOO_LARGE");
    }
    let url = reqwest::Url::parse(link).map_err(|_| invalid)?;
    if url.scheme() != "skills-hub"
        || url.host_str() != Some("install")
        || !matches!(url.path(), "" | "/")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid);
    }
    let params: Vec<_> = url.query_pairs().collect();
    if params.len() != 1 || params[0].0 != "manifest" {
        return Err(invalid);
    }
    let manifest: CollectionManifest = serde_json::from_str(&params[0].1).map_err(|_| invalid)?;
    validate_manifest(manifest, MAX_COLLECTION_SKILLS, MAX_COLLECTION_SOURCES)
}

fn validate_manifest(
    manifest: CollectionManifest,
    max_skills: usize,
    max_sources: usize,
) -> Result<CollectionManifest, &'static str> {
    let invalid = "COLLECTION_LINK_INVALID";
    if manifest.v != 1 {
        return Err("COLLECTION_LINK_UNSUPPORTED_VERSION");
    }
    if manifest.skills.is_empty() {
        return Err("COLLECTION_LINK_EMPTY");
    }
    if manifest.skills.len() > max_skills {
        return Err("COLLECTION_LINK_TOO_MANY_SKILLS");
    }
    if manifest.sources.len() > max_sources {
        return Err("COLLECTION_LINK_TOO_MANY_SOURCES");
    }
    if manifest.title.trim().is_empty()
        || manifest.title.chars().count() > 120
        || manifest.title.chars().any(char::is_control)
        || manifest.sources.is_empty()
    {
        return Err(invalid);
    }
    for source in &manifest.sources {
        let segments: Vec<_> = source.repo.split('/').collect();
        if segments.len() != 2
            || !segments.iter().all(|s| safe_segment(s))
            || source.revision.len() != 40
            || !source.revision.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("COLLECTION_LINK_UNSAFE");
        }
    }
    let mut names = std::collections::HashSet::new();
    let mut directories = std::collections::HashSet::new();
    for skill in &manifest.skills {
        if !safe_segment(&skill.name)
            || skill.source >= manifest.sources.len()
            || skill.path.len() > 512
            || !(skill.path == "." || skill.path.split('/').all(safe_segment))
            || skill.path.ends_with("SKILL.md")
        {
            return Err("COLLECTION_LINK_UNSAFE");
        }
        if !names.insert(skill.name.to_lowercase())
            || !directories.insert((
                manifest.sources[skill.source].repo.to_lowercase(),
                skill.path.to_lowercase(),
            ))
        {
            return Err("COLLECTION_LINK_DUPLICATE");
        }
    }
    Ok(manifest)
}
#[tauri::command]
pub fn parse_collection_link(link: String) -> Result<CollectionManifest, String> {
    parse_link(&link).map_err(str::to_owned)
}
#[tauri::command]
pub fn validate_collection_manifest(
    manifest: CollectionManifest,
) -> Result<CollectionManifest, String> {
    validate_manifest(manifest, MAX_COLLECTION_SKILLS, MAX_COLLECTION_SOURCES)
        .map_err(str::to_owned)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> serde_json::Value {
        serde_json::json!({"v":1,"title":"真实合集","sources":[{"repo":"owner/repo","ref":"a".repeat(40)}],"skills":[{"name":"design","path":"skills/design","source":0}]})
    }
    fn link(value: serde_json::Value) -> String {
        format!(
            "skills-hub://install?manifest={}",
            urlencoding::encode(&value.to_string())
        )
    }
    #[test]
    fn reports_oversized_external_links() {
        assert_eq!(
            parse_link(&"x".repeat(30001)).unwrap_err(),
            "COLLECTION_LINK_TOO_LARGE"
        );
    }
    #[test]
    fn reports_unsupported_manifest_versions() {
        let mut value = manifest();
        value["v"] = 2.into();
        assert_eq!(
            parse_link(&link(value)).unwrap_err(),
            "COLLECTION_LINK_UNSUPPORTED_VERSION"
        );
    }
    #[test]
    fn reports_empty_selections() {
        let mut value = manifest();
        value["skills"] = serde_json::json!([]);
        assert_eq!(
            parse_link(&link(value)).unwrap_err(),
            "COLLECTION_LINK_EMPTY"
        );
    }
    #[test]
    fn reports_duplicate_skills() {
        let mut value = manifest();
        let duplicate = value["skills"][0].clone();
        value["skills"].as_array_mut().unwrap().push(duplicate);
        assert_eq!(
            parse_link(&link(value)).unwrap_err(),
            "COLLECTION_LINK_DUPLICATE"
        );
    }
    #[test]
    fn reports_unsafe_installation_data_without_echoing_it() {
        for (field, content) in [
            ("path", "../private-path-example"),
            ("repo", "owner/repo?token=private-token-example"),
            ("ref", "main"),
        ] {
            let mut value = manifest();
            if field == "path" {
                value["skills"][0][field] = content.into();
            } else {
                value["sources"][0][field] = content.into();
            }
            assert_eq!(
                parse_link(&link(value)).unwrap_err(),
                "COLLECTION_LINK_UNSAFE"
            );
        }
    }
    #[test]
    fn reports_excessive_source_selection() {
        let mut value = manifest();
        value["sources"] = (0..101)
            .map(|i| serde_json::json!({"repo":format!("owner/repo-{i}"),"ref":"a".repeat(40)}))
            .collect();
        assert_eq!(
            parse_link(&link(value)).unwrap_err(),
            "COLLECTION_LINK_TOO_MANY_SOURCES"
        );
    }
    #[test]
    fn accepts_website_aaron_marketing_collection_with_120_skills() {
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "tests/fixtures/website-aaron-marketing-manifest.json"
        ))
        .unwrap();
        let parsed = parse_link(&link(value)).unwrap();
        assert_eq!(parsed.title, "Aaron 营销技能合集");
        assert_eq!(parsed.skills.len(), 120);
        assert_eq!(parsed.sources.len(), 1);
        assert_eq!(
            parsed.sources[0].repo,
            "aaron-he-zhu/aaron-marketing-skills"
        );
        assert_eq!(
            parsed.sources[0].revision,
            "9c7e1cebeabca15e0af0ce856799361a0f5d35d7"
        );
        assert_eq!(parsed.skills[0].path, "ad/activate/ad-account-auditor");
        assert_eq!(
            parsed.skills[119].path,
            "social/observe/social-pulse-monitor"
        );
    }

    #[test]
    fn native_catalog_accepts_large_manifests_without_expanding_deep_links() {
        let mut value = manifest();
        value["skills"] = (0..1297)
            .map(|i| {
                serde_json::json!({
                    "name": format!("skill-{i}"), "path": format!("skills/skill-{i}"), "source": 0
                })
            })
            .collect();
        assert!(parse_link(&link(value.clone())).is_err());
        let parsed =
            validate_collection_manifest(serde_json::from_value(value.clone()).unwrap()).unwrap();
        assert_eq!(parsed.skills.len(), 1297);
        value["skills"][0]["path"] = "../escape".into();
        assert_eq!(
            validate_collection_manifest(serde_json::from_value(value).unwrap()).unwrap_err(),
            "COLLECTION_LINK_UNSAFE"
        );
    }

    #[test]
    fn native_catalog_rejects_excessive_and_unsafe_manifests() {
        let mut value = manifest();
        value["skills"] = (0..2001)
            .map(|i| {
                serde_json::json!({
                    "name": format!("skill-{i}"), "path": format!("skills/skill-{i}"), "source": 0
                })
            })
            .collect();
        assert_eq!(
            validate_collection_manifest(serde_json::from_value(value).unwrap()).unwrap_err(),
            "COLLECTION_LINK_TOO_MANY_SKILLS"
        );
        let mut value = manifest();
        value["sources"][0]["ref"] = "main".into();
        assert!(validate_collection_manifest(serde_json::from_value(value).unwrap()).is_err());
    }

    #[test]
    fn accepts_website_collection_manifest_fixtures() {
        let fixtures: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("tests/fixtures/website-collections.json")).unwrap();
        let expected = [
            ("engineering", 31),
            ("marketing", 50),
            ("product-video", 14),
            ("taste", 13),
        ];
        assert_eq!(fixtures.len(), expected.len());
        for (fixture, (slug, count)) in fixtures.iter().zip(expected) {
            assert_eq!(fixture["slug"], slug);
            let parsed = parse_link(&link(fixture["manifest"].clone())).unwrap();
            assert_eq!(parsed.skills.len(), count);
        }
    }
    #[test]
    fn accepts_website_contract_without_installing_anything() {
        let parsed = parse_link(&link(manifest())).unwrap();
        assert_eq!(parsed.skills[0].path, "skills/design");
        assert_eq!(parsed.sources[0].revision, "a".repeat(40));
    }
    #[test]
    fn rejects_unsafe_and_ambiguous_install_requests() {
        for path in [
            "../x",
            "skills/../../x",
            "/tmp/x",
            "skills\\x",
            "skills//x",
            "skills/%2e%2e",
            "skills/x/SKILL.md",
        ] {
            let mut v = manifest();
            v["skills"][0]["path"] = path.into();
            assert!(parse_link(&link(v)).is_err());
        }
        for repo in [
            "https://github.com/a/b",
            "a/b?token=secret",
            "a/../b",
            "user@host/repo",
        ] {
            let mut v = manifest();
            v["sources"][0]["repo"] = repo.into();
            assert!(parse_link(&link(v)).is_err());
        }
        let mut v = manifest();
        v["sources"][0]["ref"] = "main".into();
        assert!(parse_link(&link(v)).is_err());
        let mut v = manifest();
        v["skills"][0]["source"] = 9.into();
        assert!(parse_link(&link(v)).is_err());
        let mut v = manifest();
        v["v"] = 2.into();
        assert!(parse_link(&link(v)).is_err());
        let mut v = manifest();
        let duplicate = v["skills"][0].clone();
        v["skills"].as_array_mut().unwrap().push(duplicate);
        assert!(parse_link(&link(v)).is_err());
        let mut v = manifest();
        v["skills"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"name":"design","path":"other/design","source":0}));
        assert!(parse_link(&link(v)).is_err());
        let mut v = manifest();
        v["token"] = "secret".into();
        assert!(parse_link(&link(v)).is_err());
        for suffix in ["&manifest=x", "&token=secret", "#x"] {
            assert!(parse_link(&format!("{}{}", link(manifest()), suffix)).is_err());
        }
        for input in [
            "https://install",
            "skills-hub://other",
            "skills-hub://install?manifest={}",
        ] {
            assert!(parse_link(input).is_err());
        }
        assert!(parse_link(&"x".repeat(30001)).is_err());
    }
}
