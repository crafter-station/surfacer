use std::io::IsTerminal;

use anyhow::{Context, anyhow};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecResult {
    pub status: u16,
    pub url: String,
    pub title: String,
    pub content: String,
    pub word_count: u64,
}

pub fn use_color() -> bool {
    std::env::var("NO_COLOR").is_err() && std::io::stdout().is_terminal()
}

pub async fn fetch_page(url: &str) -> anyhow::Result<ExecResult> {
    let output = Command::new("defuddle")
        .args(["parse", url, "--json"])
        .output()
        .await
        .context("failed to run defuddle. Install with: npm install -g @anthropic-ai/defuddle")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("defuddle failed: {stderr}"));
    }

    let parsed: DefuddleOutput = serde_json::from_slice(&output.stdout)
        .context("failed to parse defuddle JSON output")?;

    let content = html_to_text(&parsed.content);

    Ok(ExecResult {
        status: 200,
        url: url.to_string(),
        title: parsed.title,
        content,
        word_count: parsed.word_count,
    })
}

pub fn format_human(
    result: &ExecResult,
    site_name: &str,
    command: &str,
    descriptor: &surfacer_ir::SiteDescriptor,
) -> String {
    let color = use_color();
    let mut output = String::new();

    let header = format!("{} — {}", site_name, result.title);
    if color {
        output.push_str(&format!("\n  {}\n\n", header.bold()));
    } else {
        output.push_str(&format!("\n  {header}\n\n"));
    }

    let lines: Vec<&str> = result.content.lines().collect();
    let page_size = 30;
    let total_lines = lines.len();

    for line in lines.iter().take(page_size) {
        if line.trim().is_empty() {
            continue;
        }
        let formatted = format_content_line(line, color);
        output.push_str(&format!("  {formatted}\n"));
    }

    if total_lines > page_size {
        let more_msg = format!("... ({} more lines, showing first {})", total_lines - page_size, page_size);
        if color {
            output.push_str(&format!("\n  {}\n", more_msg.dimmed()));
        } else {
            output.push_str(&format!("\n  {more_msg}\n"));
        }
    }

    output.push('\n');
    output.push_str(&surfacer_emit_cli::build_next_steps_after_exec(
        site_name, command, descriptor, color,
    ));

    output
}

fn format_content_line(line: &str, color: bool) -> String {
    if !color {
        return line.to_string();
    }

    let trimmed = line.trim();

    if trimmed.starts_with(|c: char| c.is_ascii_digit()) && trimmed.contains('.') {
        let parts: Vec<&str> = trimmed.splitn(2, ". ").collect();
        if parts.len() == 2 {
            return format!("{}. {}", parts[0].green().bold(), parts[1].white());
        }
    }

    if trimmed.contains("points by") || trimmed.contains("hours ago") || trimmed.contains("comments") {
        return format!("{}", trimmed.dimmed());
    }

    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        return format!("{}", trimmed.cyan());
    }

    line.to_string()
}

pub async fn fetch_raw_html(url: &str) -> anyhow::Result<String> {
    let output = Command::new("defuddle")
        .args(["parse", url, "--json"])
        .output()
        .await
        .context("failed to run defuddle")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("defuddle failed: {stderr}"));
    }

    let parsed: DefuddleOutput = serde_json::from_slice(&output.stdout)
        .context("failed to parse defuddle output")?;

    Ok(parsed.content)
}

pub async fn fetch_authenticated_html(url: &str, session_name: &str) -> anyhow::Result<String> {
    let ab = surfacer_probe::agent_browser::agent_browser_bin_public();

    let connect = Command::new(&ab)
        .args(["--session", session_name, "connect", "9222"])
        .output()
        .await;

    if connect.is_err() || !connect.as_ref().unwrap().status.success() {
        let open = Command::new(&ab)
            .args(["--session", session_name, "open", url])
            .output()
            .await
            .context("failed to open URL in authenticated session")?;
        if !open.status.success() {
            return Err(anyhow!("agent-browser open failed: {}", String::from_utf8_lossy(&open.stderr)));
        }
    } else {
        let open = Command::new(&ab)
            .args(["--session", session_name, "open", url])
            .output()
            .await
            .context("failed to navigate in authenticated session")?;
        if !open.status.success() {
            return Err(anyhow!("agent-browser open failed: {}", String::from_utf8_lossy(&open.stderr)));
        }
    }

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let html_output = Command::new(&ab)
        .args(["--session", session_name, "eval", "document.documentElement.outerHTML"])
        .output()
        .await
        .context("failed to extract HTML from authenticated page")?;

    if !html_output.status.success() {
        return Err(anyhow!("failed to get page HTML: {}", String::from_utf8_lossy(&html_output.stderr)));
    }

    Ok(String::from_utf8_lossy(&html_output.stdout).to_string())
}

pub fn get_session_name(site_name: &str) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let session_file = surfacer_ir::site_dir(&std::path::PathBuf::from(home), site_name)
        .join("session-name");
    std::fs::read_to_string(session_file).ok()
}

pub fn format_extracted_human(
    items: &[surfacer_ir::ExtractedItem],
    site_name: &str,
    command: &str,
    title: &str,
    descriptor: &surfacer_ir::SiteDescriptor,
) -> String {
    let color = use_color();
    let mut out = String::new();

    let header = format!("{} — {}", site_name, title);
    if color {
        out.push_str(&format!("\n  {}\n\n", header.bold()));
    } else {
        out.push_str(&format!("\n  {header}\n\n"));
    }

    let page_size = 15;
    for item in items.iter().take(page_size) {
        let title_text = item.primary_title().unwrap_or("(untitled)");
        let url_text = item.get_text("domain")
            .or_else(|| item.get_url("url").map(|u| {
                url::Url::parse(u).ok()
                    .and_then(|p| p.host_str().map(|h| h.to_string()))
                    .unwrap_or_default()
                    .leak() as &str
            }))
            .unwrap_or("");

        if color {
            out.push_str(&format!("  {}  {}\n",
                format!("{:>3}", item.index).green().bold(),
                title_text.white().bold(),
            ));
            if !url_text.is_empty() {
                out.push_str(&format!("      {}", url_text.cyan()));
            }
            if let Some(n) = item.get_number("points") {
                out.push_str(&format!(" · {} pts", (n as u64).to_string().yellow()));
            }
            if let Some(author) = item.get_text("author") {
                out.push_str(&format!(" · {}", author.dimmed()));
            }
            out.push('\n');
        } else {
            out.push_str(&format!("  {:>3}  {}\n", item.index, title_text));
            let mut meta = Vec::new();
            if !url_text.is_empty() { meta.push(url_text.to_string()); }
            if let Some(n) = item.get_number("points") { meta.push(format!("{} pts", n as u64)); }
            if let Some(author) = item.get_text("author") { meta.push(author.to_string()); }
            if !meta.is_empty() {
                out.push_str(&format!("       {}\n", meta.join(" · ")));
            }
        }
    }

    if items.len() > page_size {
        let more = format!("Showing 1-{} of {}", page_size, items.len());
        if color {
            out.push_str(&format!("\n  {}\n", more.dimmed()));
        } else {
            out.push_str(&format!("\n  {more}\n"));
        }
    }

    out.push('\n');

    let has_urls = items.iter().any(|i| i.primary_url().is_some());
    if color {
        out.push_str(&format!("  {}\n", "Drill down:".dimmed()));
        if has_urls {
            out.push_str(&format!("    {}     {}\n",
                format!("{site_name} open 1").cyan(),
                "Open item #1 in browser".dimmed()));
        }
        out.push_str(&format!("    {}  {}\n",
            format!("{site_name} {command} --json").cyan(),
            "Machine-readable output".dimmed()));
    } else {
        out.push_str("  Drill down:\n");
        if has_urls {
            out.push_str(&format!("    {site_name} open 1     Open item #1 in browser\n"));
        }
        out.push_str(&format!("    {site_name} {command} --json  Machine-readable output\n"));
    }

    out.push_str(&surfacer_emit_cli::build_next_steps_after_exec(
        site_name, command, descriptor, color,
    ));

    out
}

pub fn format_extracted_json(items: &[surfacer_ir::ExtractedItem], url: &str, title: &str) -> anyhow::Result<String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct StructuredResult<'a> {
        url: &'a str,
        title: &'a str,
        item_count: usize,
        items: &'a [surfacer_ir::ExtractedItem],
    }

    let result = StructuredResult {
        url,
        title,
        item_count: items.len(),
        items,
    };

    serde_json::to_string_pretty(&result).context("failed to serialize extracted items")
}

pub fn format_json(result: &ExecResult) -> anyhow::Result<String> {
    serde_json::to_string_pretty(result).context("failed to serialize exec result")
}

pub fn html_to_text_public(html: &str) -> String {
    html_to_text(html)
}

fn html_to_text(html: &str) -> String {
    let mut text = html.to_string();

    text = text.replace("<br>", "\n").replace("<br/>", "\n").replace("<br />", "\n");
    text = text.replace("</p>", "\n").replace("</div>", "\n").replace("</li>", "\n");
    text = text.replace("</tr>", "\n").replace("</h1>", "\n").replace("</h2>", "\n");
    text = text.replace("</h3>", "\n").replace("</h4>", "\n");

    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }

    result = result.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ");

    let lines: Vec<String> = result
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    lines.join("\n")
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DefuddleOutput {
    content: String,
    title: String,
    #[serde(default)]
    word_count: u64,
}
