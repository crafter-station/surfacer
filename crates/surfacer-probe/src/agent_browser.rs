use anyhow::{anyhow, Context};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::time::{sleep, Duration};

pub struct BrowserProcess {
    pub child_id: u32,
    pub cdp_port: u16,
    pub profile_dir: PathBuf,
}

pub struct ProbeSession {
    pub browser: BrowserProcess,
    pub output_dir: PathBuf,
    pub session_name: String,
}

pub async fn spawn_comet(
    port: u16,
    profile_dir: PathBuf,
) -> anyhow::Result<BrowserProcess> {
    std::fs::create_dir_all(&profile_dir).with_context(|| {
        format!(
            "failed to create Comet profile dir at {}",
            profile_dir.display()
        )
    })?;

    let comet_bin = "/Applications/Comet.app/Contents/MacOS/Comet";
    if !Path::new(comet_bin).exists() {
        return Err(anyhow!("Comet not found at {comet_bin}"));
    }

    let output = Command::new("zsh")
        .arg("-lc")
        .arg(format!(
            "nohup {comet_bin:?} --remote-debugging-port={port} --user-data-dir={profile:?} --no-first-run --no-default-browser-check > /tmp/surfacer-comet.log 2>&1 & echo $!",
            profile = profile_dir
        ))
        .output()
        .await
        .context("failed to launch Comet via nohup")?;
    ensure_success(&output, "launch Comet via nohup")?;

    let child_id = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .context("failed to parse Comet child pid from launcher output")?;

    wait_for_cdp(port).await?;

    Ok(BrowserProcess {
        child_id,
        cdp_port: port,
        profile_dir,
    })
}

pub async fn connect_session(
    process: BrowserProcess,
    opts: &super::capture::ProbeOptions,
) -> anyhow::Result<ProbeSession> {
    let session_name = session_name_for_url(&opts.url)?;

    let connect = Command::new(agent_browser_bin())
        .args([
            "--session",
            &session_name,
            "connect",
            &process.cdp_port.to_string(),
        ])
        .output()
        .await
        .with_context(|| format!("failed to run agent-browser connect for {session_name}"))?;
    ensure_success(&connect, "agent-browser connect")?;

    let open = Command::new(agent_browser_bin())
        .args(["--session", &session_name, "open", &opts.url])
        .output()
        .await
        .with_context(|| format!("failed to run agent-browser open for {}", opts.url))?;
    ensure_success(&open, "agent-browser open")?;

    Ok(ProbeSession {
        browser: process,
        output_dir: opts.output_dir.clone(),
        session_name,
    })
}

pub async fn start_har_capture(session: &ProbeSession) -> anyhow::Result<()> {
    let output = Command::new(agent_browser_bin())
        .args(["--session", &session.session_name, "network", "har", "start"])
        .output()
        .await
        .with_context(|| {
            format!(
                "failed to run agent-browser network har start for {}",
                session.session_name
            )
        })?;
    ensure_success(&output, "agent-browser network har start")
}

pub async fn stop_har_capture(session: &ProbeSession) -> anyhow::Result<PathBuf> {
    let output_path = crate::paths::har_path(&session.output_dir);
    let output = Command::new(agent_browser_bin())
        .args([
            "--session",
            &session.session_name,
            "network",
            "har",
            "stop",
            output_path.to_string_lossy().as_ref(),
        ])
        .output()
        .await
        .with_context(|| {
            format!(
                "failed to run agent-browser network har stop for {}",
                session.session_name
            )
        })?;
    ensure_success(&output, "agent-browser network har stop")?;
    Ok(output_path)
}

pub async fn take_ax_snapshot(session: &ProbeSession, output_path: &Path) -> anyhow::Result<()> {
    let output = Command::new(agent_browser_bin())
        .args([
            "--session",
            &session.session_name,
            "snapshot",
            "--interactive",
            "--urls",
            "--compact",
        ])
        .output()
        .await
        .with_context(|| {
            format!(
                "failed to run agent-browser snapshot for {}",
                session.session_name
            )
        })?;
    ensure_success(&output, "agent-browser snapshot")?;
    std::fs::write(output_path, &output.stdout).with_context(|| {
        format!("failed to write AX snapshot to {}", output_path.display())
    })?;
    Ok(())
}

pub async fn get_url(session: &ProbeSession) -> anyhow::Result<String> {
    let output = Command::new(agent_browser_bin())
        .args(["--session", &session.session_name, "get", "url"])
        .output()
        .await
        .with_context(|| format!("failed to run agent-browser get url for {}", session.session_name))?;
    ensure_success(&output, "agent-browser get url")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn get_title(session: &ProbeSession) -> anyhow::Result<String> {
    let output = Command::new(agent_browser_bin())
        .args(["--session", &session.session_name, "get", "title"])
        .output()
        .await
        .with_context(|| {
            format!(
                "failed to run agent-browser get title for {}",
                session.session_name
            )
        })?;
    ensure_success(&output, "agent-browser get title")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn click(session: &ProbeSession, ref_id: &str) -> anyhow::Result<()> {
    let output = Command::new(agent_browser_bin())
        .args(["--session", &session.session_name, "click", ref_id])
        .output()
        .await
        .with_context(|| format!("failed to click {ref_id} in {}", session.session_name))?;
    ensure_success(&output, &format!("agent-browser click {ref_id}"))
}

pub async fn back(session: &ProbeSession) -> anyhow::Result<()> {
    let output = Command::new(agent_browser_bin())
        .args(["--session", &session.session_name, "back"])
        .output()
        .await
        .with_context(|| format!("failed to go back in {}", session.session_name))?;
    ensure_success(&output, "agent-browser back")
}

pub async fn snapshot_text(session: &ProbeSession) -> anyhow::Result<String> {
    let output = Command::new(agent_browser_bin())
        .args([
            "--session",
            &session.session_name,
            "snapshot",
            "--interactive",
            "--urls",
            "--compact",
        ])
        .output()
        .await
        .with_context(|| format!("failed to run snapshot for {}", session.session_name))?;
    ensure_success(&output, "agent-browser snapshot")?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn agent_browser_bin_public() -> String {
    agent_browser_bin()
}

fn agent_browser_bin() -> String {
    if let Ok(bin) = std::env::var("AGENT_BROWSER_BIN") {
        return bin;
    }
    if let Ok(home) = std::env::var("HOME") {
        let local = format!("{home}/.local/bin/agent-browser");
        if std::path::Path::new(&local).exists() {
            return local;
        }
    }
    "agent-browser".to_string()
}

fn session_name_for_url(raw_url: &str) -> anyhow::Result<String> {
    let url = url::Url::parse(raw_url).with_context(|| format!("failed to parse URL {raw_url}"))?;
    let host = url
        .host_str()
        .context("URL is missing a host for session naming")?;
    let slug = host
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_ascii_lowercase();
    Ok(format!("surfacer-{slug}"))
}

async fn wait_for_cdp(port: u16) -> anyhow::Result<()> {
    for _ in 0..30 {
        let output = Command::new("curl")
            .args(["-sf", &format!("http://localhost:{port}/json/version")])
            .output()
            .await
            .with_context(|| format!("failed to probe CDP endpoint on port {port}"))?;
        if output.status.success() {
            return Ok(());
        }
        sleep(Duration::from_millis(500)).await;
    }
    Err(anyhow!(
        "CDP did not become ready within 15s on http://localhost:{port}/json/version"
    ))
}

fn ensure_success(output: &std::process::Output, action: &str) -> anyhow::Result<()> {
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        Err(anyhow!("{action} failed with status {}", output.status))
    } else {
        Err(anyhow!("{action} failed: {stderr}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{connect_session, spawn_comet, start_har_capture, stop_har_capture};
    use crate::capture::ProbeOptions;

    #[test]
    #[ignore = "requires a live Comet + agent-browser environment"]
    fn spawn_comet_smoke() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime
            .block_on(async {
                let _ = spawn_comet(9222, std::env::temp_dir().join("surfacer-probe-test")).await?;
                anyhow::Ok(())
            })
            .expect("spawn comet");
    }

    #[test]
    #[ignore = "requires a live Comet + agent-browser environment"]
    fn session_smoke() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime
            .block_on(async {
                let browser = spawn_comet(9222, std::env::temp_dir().join("surfacer-probe-test")).await?;
                let opts = ProbeOptions {
                    url: "https://www.sunat.gob.pe".to_string(),
                    visible: true,
                    output_dir: std::env::temp_dir().join("surfacer-probe-output"),
                };
                let session = connect_session(browser, &opts).await?;
                start_har_capture(&session).await?;
                let _ = stop_har_capture(&session).await?;
                anyhow::Ok(())
            })
            .expect("session flow");
    }
}
