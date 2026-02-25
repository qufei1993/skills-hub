use anyhow::{Context, Result};
use reqwest::blocking::Client;
use scraper::{Html, Selector};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LeaderboardEntry {
    pub rank: u32,
    pub name: String,
    pub repo: String,
    pub owner: String,
    pub description: Option<String>,
    pub installs: u64,
    pub installs_formatted: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LeaderboardType {
    AllTime,
    Trending,
    Hot,
}

impl LeaderboardType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LeaderboardType::AllTime => "all",
            LeaderboardType::Trending => "trending",
            LeaderboardType::Hot => "hot",
        }
    }
}

pub fn fetch_leaderboard(
    leaderboard_type: &LeaderboardType,
) -> Result<Vec<LeaderboardEntry>> {
    let client = Client::new();
    let url = match leaderboard_type {
        LeaderboardType::AllTime => "https://skills.sh/",
        LeaderboardType::Trending => "https://skills.sh/trending",
        LeaderboardType::Hot => "https://skills.sh/hot",
    };

    let response = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8")
        .send()
        .context("Failed to fetch leaderboard from skills.sh")?
        .error_for_status()
        .context("skills.sh returned an error")?;

    let html = response.text().context("Failed to read response body")?;
    parse_leaderboard_html(&html)
}

fn parse_leaderboard_html(html: &str) -> Result<Vec<LeaderboardEntry>> {
    let document = Html::parse_document(html);
    let mut entries = Vec::new();

    // The new HTML structure uses div-based layout with links
    // Each skill entry is in an <a> tag with href like "/owner/repo/skill-name"
    // Structure:
    // <a href="/vercel-labs/skills/find-skills" class="...">
    //   <div class="lg:col-span-1"><span>1</span></div>  // rank
    //   <div class="lg:col-span-13">
    //     <h3>find-skills</h3>  // skill name
    //     <p>vercel-labs/skills</p>  // owner/repo
    //   </div>
    //   <div class="lg:col-span-2"><span>319.0K</span></div>  // installs
    // </a>

    // Select all anchor tags that link to skills (pattern: /owner/repo/skill-name)
    let link_selector = Selector::parse("a[href^='/'][href*='/'][href*='/']").unwrap();

    for link in document.select(&link_selector) {
        let href = link.value().attr("href").unwrap_or("");
        
        // Skip navigation links and other non-skill links
        // Skill links have pattern: /owner/repo/skill-name (at least 3 path segments)
        let parts: Vec<&str> = href.trim_start_matches('/').split('/').collect();
        if parts.len() < 3 {
            continue;
        }
        
        // Skip non-skill pages (like /audits, /docs, etc.)
        if parts[0].starts_with('_') || parts[0] == "audits" || parts[0] == "docs" {
            continue;
        }

        // Extract rank from the first span inside the link
        let rank_selector = Selector::parse("div:first-child span").unwrap();
        let rank_text = link
            .select(&rank_selector)
            .next()
            .and_then(|elem| elem.text().next())
            .map(|t| t.trim())
            .unwrap_or("0");

        let rank: u32 = match rank_text.parse() {
            Ok(r) => r,
            Err(_) => continue, // Skip if rank is not a number (likely not a skill entry)
        };

        // Extract skill name from h3
        let name_selector = Selector::parse("h3").unwrap();
        let name = link
            .select(&name_selector)
            .next()
            .and_then(|elem| elem.text().next())
            .map(|t| t.trim().to_string())
            .unwrap_or_default();

        // Extract owner/repo from the p tag
        let repo_selector = Selector::parse("p").unwrap();
        let owner_repo = link
            .select(&repo_selector)
            .next()
            .and_then(|elem| elem.text().next())
            .map(|t| t.trim().to_string())
            .unwrap_or_default();

        // Parse owner and repo from "owner/repo" format
        let (owner, repo) = if owner_repo.contains('/') {
            let parts: Vec<&str> = owner_repo.splitn(2, '/').collect();
            if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                (owner_repo.clone(), String::new())
            }
        } else {
            (owner_repo.clone(), String::new())
        };

        // Extract installs - look for the last span in the link (the one showing installs)
        let installs_selector = Selector::parse("div:last-child span").unwrap();
        let installs_text = link
            .select(&installs_selector)
            .next()
            .and_then(|elem| elem.text().next())
            .map(|t| t.trim().to_string())
            .unwrap_or_default();

        let installs = parse_installs_number(&installs_text);

        // Only add entry if we have valid data
        if !name.is_empty() && !owner.is_empty() && rank > 0 {
            entries.push(LeaderboardEntry {
                rank,
                name,
                repo,
                owner,
                description: None,
                installs,
                installs_formatted: installs_text,
            });
        }
    }

    // Sort by rank and remove duplicates
    entries.sort_by_key(|e| e.rank);
    entries.dedup_by_key(|e| e.rank);

    Ok(entries)
}

fn parse_repo_path(href: &str) -> (String, String) {
    // Handle paths like "/vercel-labs/skills" or "https://github.com/vercel-labs/skills"
    let path = href.trim_start_matches("https://github.com/");
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if parts.len() >= 2 {
        (parts[0].to_string(), parts[1].to_string())
    } else if parts.len() == 1 {
        (parts[0].to_string(), String::new())
    } else {
        (String::new(), String::new())
    }
}

fn parse_installs_number(text: &str) -> u64 {
    // Parse numbers like "319.0K", "49.3K", "74,313"
    let text = text.trim();
    if text.is_empty() {
        return 0;
    }
    
    // Handle K suffix (thousands)
    if text.ends_with('K') || text.ends_with('k') {
        let num_str = text.trim_end_matches(['K', 'k']).trim();
        if let Ok(num) = num_str.parse::<f64>() {
            return (num * 1000.0) as u64;
        }
    }
    
    // Handle M suffix (millions)
    if text.ends_with('M') || text.ends_with('m') {
        let num_str = text.trim_end_matches(['M', 'm']).trim();
        if let Ok(num) = num_str.parse::<f64>() {
            return (num * 1_000_000.0) as u64;
        }
    }
    
    // Handle plain numbers with or without commas
    let text = text.replace(',', "");
    text.parse().unwrap_or(0)
}

#[cfg(test)]
#[path = "tests/skills_sh_leaderboard.rs"]
mod tests;