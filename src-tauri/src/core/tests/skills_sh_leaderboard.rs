use super::{leaderboard_url, parse_leaderboard_html, LeaderboardType};

#[test]
fn parses_skill_slug_from_href_instead_of_display_name() {
    let html = r#"
    <html>
      <body>
        <a href="/anthropics/claude-code/mcp-integration">
          <div><span>42</span></div>
          <div>
            <h3>mcp integration</h3>
            <p>anthropics/claude-code</p>
          </div>
          <div><span>1.2K</span></div>
        </a>
      </body>
    </html>
    "#;

    let entries = parse_leaderboard_html(html).expect("parse should succeed");
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];

    assert_eq!(entry.name, "mcp integration");
    assert_eq!(entry.skill_slug, "mcp-integration");
}

#[test]
fn builds_search_url_with_q_parameter() {
    let url = leaderboard_url(&LeaderboardType::AllTime, Some("ss"));
    assert_eq!(url, "https://skills.sh/?q=ss");
}

#[test]
fn builds_search_url_with_encoded_q_parameter() {
    let url = leaderboard_url(&LeaderboardType::AllTime, Some("mcp integration"));
    assert_eq!(url, "https://skills.sh/?q=mcp%20integration");
}
