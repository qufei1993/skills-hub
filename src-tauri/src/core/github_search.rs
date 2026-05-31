use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SearchResponse {
    items: Vec<RepoItem>,
}

#[derive(Debug, Deserialize)]
struct RepoItem {
    full_name: String,
    html_url: String,
    description: Option<String>,
    stargazers_count: u64,
    updated_at: String,
    clone_url: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RepoSummary {
    pub full_name: String,
    pub html_url: String,
    pub description: Option<String>,
    pub stars: u64,
    pub updated_at: String,
    pub clone_url: String,
}

pub fn search_github_repos(
    query: &str,
    limit: usize,
    token: Option<&str>,
) -> Result<Vec<RepoSummary>> {
    search_github_repos_inner("https://api.github.com", query, limit, token)
}

fn search_github_repos_inner(
    base_url: &str,
    query: &str,
    limit: usize,
    token: Option<&str>,
) -> Result<Vec<RepoSummary>> {
    let client = Client::new();
    let base_url = base_url.trim_end_matches('/');
    let url = format!(
        "{}/search/repositories?q={}&per_page={}",
        base_url,
        urlencoding::encode(query),
        limit.clamp(1, 50)
    );

    let mut req = client.get(url).header("User-Agent", "SkillSyncer");
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }
    let response = req
        .send()
        .context("GitHub search request failed")?
        .error_for_status()
        .context("GitHub search returned error")?;

    let result: SearchResponse = response.json().context("parse GitHub response")?;

    Ok(result
        .items
        .into_iter()
        .map(|item| RepoSummary {
            full_name: item.full_name,
            html_url: item.html_url,
            description: item.description,
            stars: item.stargazers_count,
            updated_at: item.updated_at,
            clone_url: item.clone_url,
        })
        .collect())
}

#[cfg(test)]
#[path = "tests/github_search.rs"]
mod tests;
