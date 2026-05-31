use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;

use super::skill_store::SkillStore;

const FEATURED_SKILLS_URL: &str =
    "https://raw.githubusercontent.com/AlwaysCodeHQ/skills-syncer/main/featured-skills.json";

const CACHE_KEY: &str = "featured_skills_cache";

// Bundled fallback so the app works even before the first network fetch succeeds.
const BUNDLED_JSON: &str = include_str!("../../../featured-skills.json");

#[derive(Debug, Deserialize)]
struct FeaturedSkillsData {
    skills: Vec<FeaturedSkillRaw>,
}

#[derive(Debug, Deserialize)]
struct FeaturedSkillRaw {
    slug: String,
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    stars: u64,
    #[serde(default)]
    source_url: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FeaturedSkill {
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub downloads: u64,
    pub stars: u64,
    pub source_url: String,
}

pub fn fetch_featured_skills(store: &SkillStore) -> Result<Vec<FeaturedSkill>> {
    fetch_featured_skills_inner(FEATURED_SKILLS_URL, store)
}

fn fetch_featured_skills_inner(url: &str, store: &SkillStore) -> Result<Vec<FeaturedSkill>> {
    if let Ok(json_str) = fetch_from_url(url) {
        if let Ok(skills) = parse_and_filter(&json_str) {
            if !skills.is_empty() {
                let _ = store.set_setting(CACHE_KEY, &json_str);
                return Ok(skills);
            }
        }
    }
    // Fallback to cache
    if let Ok(Some(cached)) = store.get_setting(CACHE_KEY) {
        if let Ok(skills) = parse_and_filter(&cached) {
            if !skills.is_empty() {
                return Ok(skills);
            }
        }
    }
    // Fallback to bundled JSON
    Ok(parse_and_filter(BUNDLED_JSON).unwrap_or_default())
}

fn fetch_from_url(url: &str) -> Result<String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .context("build HTTP client")?;

    let body = client
        .get(url)
        .header("User-Agent", "SkillSyncer")
        .send()
        .context("fetch featured skills")?
        .error_for_status()
        .context("featured skills HTTP error")?
        .text()
        .context("read featured skills body")?;

    Ok(body)
}

fn parse_and_filter(json_str: &str) -> Result<Vec<FeaturedSkill>> {
    let data: FeaturedSkillsData =
        serde_json::from_str(json_str).context("parse featured skills JSON")?;

    Ok(data
        .skills
        .into_iter()
        .filter(|s| !s.source_url.is_empty())
        .map(|s| FeaturedSkill {
            slug: s.slug,
            name: s.name,
            summary: s.summary,
            downloads: s.downloads,
            stars: s.stars,
            source_url: s.source_url,
        })
        .collect())
}

#[cfg(test)]
#[path = "tests/featured_skills.rs"]
mod tests;
