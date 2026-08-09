use std::collections::HashSet;
use std::path::Path;

use anyhow::Context;
use regex::Regex;
use tokio::time::{sleep, Duration};

use crate::agent_browser::{self, ProbeSession};

const MAX_ITERATIONS: usize = 15;
const SETTLE_DELAY_MS: u64 = 2000;

static DESTRUCTIVE_PATTERNS: &[&str] = &[
    "/vote", "/hide", "/submit", "/delete", "/logout", "/pay",
    "/remove", "/cancel", "/revoke", "/flag", "/unsubscribe",
    "/transfer", "/revertir", "/dar-de-baja", "/emitir",
];

#[derive(Debug, Clone)]
pub struct AutoReconResult {
    pub iterations: usize,
    pub pages_visited: usize,
    pub ax_snapshots: Vec<String>,
    pub urls_visited: Vec<String>,
    pub stop_reason: String,
}

#[derive(Debug, Clone)]
struct InteractiveElement {
    ref_id: String,
    role: String,
    text: String,
    url: Option<String>,
}

pub async fn run_auto_recon(
    session: &ProbeSession,
    on_progress: impl Fn(usize, usize, &str),
) -> anyhow::Result<AutoReconResult> {
    let mut visited_urls: HashSet<String> = HashSet::new();
    let mut ax_hashes: HashSet<String> = HashSet::new();
    let mut ax_snapshots: Vec<String> = Vec::new();
    let mut urls_visited: Vec<String> = Vec::new();
    let mut stop_reason = String::new();

    let current_url = agent_browser::get_url(session).await.unwrap_or_default();
    visited_urls.insert(normalize_url(&current_url));

    for iteration in 1..=MAX_ITERATIONS {
        let url = agent_browser::get_url(session).await.unwrap_or_default();
        let ax_text = agent_browser::snapshot_text(session).await
            .context("auto-recon: failed to take AX snapshot")?;

        let ax_hash = hash_ax(&ax_text);
        let elements = parse_interactive_elements(&ax_text);

        on_progress(iteration, elements.len(), &url);

        urls_visited.push(url.clone());
        ax_snapshots.push(ax_text.clone());

        if ax_hashes.contains(&ax_hash) {
            stop_reason = "ax_hash_repeat (page already seen with identical state)".into();
            break;
        }
        ax_hashes.insert(ax_hash);
        visited_urls.insert(normalize_url(&url));

        let snapshot_path = session.output_dir.join(format!("ax-auto-{iteration:02}.txt"));
        std::fs::write(&snapshot_path, &ax_text)
            .with_context(|| format!("failed to write AX snapshot to {}", snapshot_path.display()))?;

        let next = pick_next_action(&elements, &visited_urls);

        match next {
            AutoAction::Click(element) => {
                if let Some(ref target_url) = element.url {
                    visited_urls.insert(normalize_url(target_url));
                }
                agent_browser::click(session, &element.ref_id).await
                    .with_context(|| format!("auto-recon: failed to click {}", element.ref_id))?;
                sleep(Duration::from_millis(SETTLE_DELAY_MS)).await;
            }
            AutoAction::Back => {
                agent_browser::back(session).await
                    .context("auto-recon: failed to go back")?;
                sleep(Duration::from_millis(SETTLE_DELAY_MS)).await;
            }
            AutoAction::Stop(reason) => {
                stop_reason = reason;
                break;
            }
        }

        if iteration == MAX_ITERATIONS {
            stop_reason = "max_iterations".into();
        }
    }

    Ok(AutoReconResult {
        iterations: urls_visited.len(),
        pages_visited: visited_urls.len(),
        ax_snapshots,
        urls_visited,
        stop_reason,
    })
}

enum AutoAction {
    Click(InteractiveElement),
    Back,
    Stop(String),
}

fn pick_next_action(elements: &[InteractiveElement], visited: &HashSet<String>) -> AutoAction {
    let nav_links: Vec<&InteractiveElement> = elements
        .iter()
        .filter(|e| e.role == "link")
        .filter(|e| !e.text.is_empty())
        .filter(|e| {
            if let Some(ref url) = e.url {
                !is_destructive(url) && !is_external(url, visited) && !visited.contains(&normalize_url(url))
            } else {
                false
            }
        })
        .collect();

    if let Some(best) = nav_links.first() {
        return AutoAction::Click((*best).clone());
    }

    let unvisited_buttons: Vec<&InteractiveElement> = elements
        .iter()
        .filter(|e| e.role == "button" || e.role == "menuitem" || e.role == "tab")
        .filter(|e| !is_destructive_text(&e.text))
        .take(1)
        .collect();

    if let Some(btn) = unvisited_buttons.first() {
        return AutoAction::Click((*btn).clone());
    }

    AutoAction::Stop("no_unvisited_links_or_buttons".into())
}

fn parse_interactive_elements(ax_text: &str) -> Vec<InteractiveElement> {
    let re = Regex::new(
        r#"(?i)^\s*-?\s*(?P<role>link|button|textbox|menuitem|checkbox|radio|combobox|tab)\s+(?:"(?P<text>[^"]*)")?\s*\[ref=(?P<ref>e\d+)(?:,\s*url=(?P<url>[^\]]+))?\]"#
    ).expect("valid regex");

    let re_alt = Regex::new(
        r#"(?i)^\s*-?\s*(?P<role>link|button|textbox|menuitem|checkbox|radio|combobox|tab)\s+\[ref=(?P<ref>e\d+)(?:,\s*url=(?P<url>[^\]]+))?\]"#
    ).expect("valid regex");

    let mut elements = Vec::new();

    for line in ax_text.lines() {
        if let Some(caps) = re.captures(line).or_else(|| re_alt.captures(line)) {
            let ref_id = format!("@{}", &caps["ref"]);
            let role = caps["role"].to_lowercase();
            let text = caps.name("text").map(|m| m.as_str().to_string()).unwrap_or_default();
            let url = caps.name("url").map(|m| m.as_str().to_string());

            elements.push(InteractiveElement {
                ref_id,
                role,
                text,
                url,
            });
        }
    }

    elements
}

fn is_destructive(url: &str) -> bool {
    let lower = url.to_lowercase();
    DESTRUCTIVE_PATTERNS.iter().any(|p| lower.contains(p))
}

fn is_destructive_text(text: &str) -> bool {
    let lower = text.to_lowercase();
    ["delete", "remove", "logout", "sign out", "cancel", "submit", "pay", "vote", "flag"]
        .iter()
        .any(|p| lower.contains(p))
}

fn is_external(url: &str, visited: &HashSet<String>) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };

    let target_base = base_domain(host);

    !visited.iter().any(|v| {
        url::Url::parse(v)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .map(|h| base_domain(&h) == target_base)
            .unwrap_or(false)
    })
}

fn base_domain(host: &str) -> String {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() <= 2 {
        return host.to_lowercase();
    }
    let tld_len = if parts.len() >= 3
        && matches!(parts[parts.len() - 2], "gob" | "gov" | "co" | "com" | "org" | "net" | "edu" | "ac")
    {
        3
    } else {
        2
    };
    parts[parts.len().saturating_sub(tld_len)..].join(".").to_lowercase()
}

fn normalize_url(url: &str) -> String {
    url.split('?').next().unwrap_or(url)
        .split('#').next().unwrap_or(url)
        .trim_end_matches('/')
        .to_lowercase()
}

fn hash_ax(text: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let normalized: Vec<&str> = text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    normalized.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_interactive_elements_from_ax() {
        let ax = r#"
- link "Hacker News" [ref=e113, url=https://news.ycombinator.com/news]
- link "new" [ref=e114, url=https://news.ycombinator.com/newest]
- link "threads" [ref=e115, url=https://news.ycombinator.com/threads?id=DemoUser]
- button "Submit" [ref=e200]
- link [ref=e273, url=https://news.ycombinator.com/vote?id=47724352&how=up]
"#;
        let elements = parse_interactive_elements(ax);
        assert!(elements.len() >= 3);

        let hn = elements.iter().find(|e| e.text == "Hacker News").unwrap();
        assert_eq!(hn.ref_id, "@e113");
        assert_eq!(hn.role, "link");
        assert!(hn.url.as_deref().unwrap().contains("news.ycombinator.com"));
    }

    #[test]
    fn filters_destructive_urls() {
        assert!(is_destructive("https://news.ycombinator.com/vote?id=123"));
        assert!(is_destructive("https://sunat.gob.pe/emitir/rhe"));
        assert!(is_destructive("https://example.com/logout"));
        assert!(!is_destructive("https://news.ycombinator.com/newest"));
        assert!(!is_destructive("https://sunat.gob.pe/consulta"));
    }

    #[test]
    fn filters_destructive_button_text() {
        assert!(is_destructive_text("Delete item"));
        assert!(is_destructive_text("Sign Out"));
        assert!(is_destructive_text("Submit Form"));
        assert!(!is_destructive_text("View Details"));
        assert!(!is_destructive_text("Search"));
    }

    #[test]
    fn normalizes_urls_for_dedup() {
        assert_eq!(normalize_url("https://example.com/page?q=1"), "https://example.com/page");
        assert_eq!(normalize_url("https://example.com/page#section"), "https://example.com/page");
        assert_eq!(normalize_url("https://EXAMPLE.COM/Page/"), "https://example.com/page");
    }

    #[test]
    fn pick_skips_visited_links() {
        let elements = vec![
            InteractiveElement {
                ref_id: "@e1".into(),
                role: "link".into(),
                text: "Already visited".into(),
                url: Some("https://example.com/visited".into()),
            },
            InteractiveElement {
                ref_id: "@e2".into(),
                role: "link".into(),
                text: "New page".into(),
                url: Some("https://example.com/new".into()),
            },
        ];

        let mut visited = HashSet::new();
        visited.insert("https://example.com/visited".into());

        match pick_next_action(&elements, &visited) {
            AutoAction::Click(el) => assert_eq!(el.ref_id, "@e2"),
            _ => panic!("expected Click on unvisited link"),
        }
    }

    #[test]
    fn pick_stops_when_all_visited() {
        let elements = vec![
            InteractiveElement {
                ref_id: "@e1".into(),
                role: "link".into(),
                text: "Visited".into(),
                url: Some("https://example.com/visited".into()),
            },
        ];

        let mut visited = HashSet::new();
        visited.insert("https://example.com/visited".into());

        assert!(matches!(pick_next_action(&elements, &visited), AutoAction::Stop(_)));
    }
}
