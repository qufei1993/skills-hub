use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SkillsShResponse {
    skills: Vec<SkillsShItem>,
}

#[derive(Debug, Deserialize)]
struct SkillsShItem {
    name: String,
    installs: u64,
    source: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OnlineSkillResult {
    pub name: String,
    pub installs: u64,
    pub source: String,
    pub source_url: String,
}

pub fn search_skills_online(query: &str, limit: usize) -> Result<Vec<OnlineSkillResult>> {
    search_skills_online_inner("https://skills.sh", query, limit)
}

fn search_skills_online_inner(
    base_url: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<OnlineSkillResult>> {
    let client = Client::new();
    let base_url = base_url.trim_end_matches('/');
    let url = format!(
        "{}/api/search?q={}&limit={}",
        base_url,
        urlencoding::encode(query),
        limit.clamp(1, 50)
    );

    let response = client
        .get(url)
        .header("User-Agent", "SkillSyncer")
        .send()
        .context("skills.sh search request failed")?
        .error_for_status()
        .context("skills.sh search returned error")?;

    let result: SkillsShResponse = response.json().context("parse skills.sh response")?;

    Ok(result
        .skills
        .into_iter()
        .map(|item| {
            let source_url = format!("https://github.com/{}", item.source);
            OnlineSkillResult {
                name: item.name,
                installs: item.installs,
                source: item.source,
                source_url,
            }
        })
        .collect())
}

#[cfg(test)]
#[path = "tests/skills_search.rs"]
mod tests;
