use anyhow::Context;

pub struct ProbeOptions {
    pub url: String,
    pub visible: bool,
    pub output_dir: std::path::PathBuf,
}

pub struct ProbeCapture {
    pub har_path: std::path::PathBuf,
    pub har_entry_count: usize,
    pub final_url: Option<String>,
    pub final_title: Option<String>,
    pub ax_pre_path: Option<std::path::PathBuf>,
    pub ax_final_path: Option<std::path::PathBuf>,
}

pub struct LiveProbeStats {
    pub iterations: u32,
    pub endpoint_count: usize,
    pub request_count: usize,
}

pub async fn capture_probe(opts: ProbeOptions) -> anyhow::Result<ProbeCapture> {
    std::fs::create_dir_all(&opts.output_dir).with_context(|| {
        format!(
            "failed to create probe output dir at {}",
            opts.output_dir.display()
        )
    })?;

    let browser = crate::agent_browser::spawn_comet(9222, opts.output_dir.join("comet-profile")).await?;
    let session = crate::agent_browser::connect_session(browser, &opts).await?;
    crate::agent_browser::start_har_capture(&session).await?;

    let ax_pre_path = crate::paths::ax_pre_path(&opts.output_dir);
    crate::agent_browser::take_ax_snapshot(&session, &ax_pre_path).await?;

    Ok(ProbeCapture {
        har_path: crate::paths::har_path(&opts.output_dir),
        har_entry_count: 0,
        final_url: None,
        final_title: None,
        ax_pre_path: Some(ax_pre_path),
        ax_final_path: None,
    })
}

pub async fn finalize_capture(
    session: super::agent_browser::ProbeSession,
) -> anyhow::Result<ProbeCapture> {
    std::fs::create_dir_all(&session.output_dir).with_context(|| {
        format!(
            "failed to ensure probe output dir exists at {}",
            session.output_dir.display()
        )
    })?;

    let ax_pre_path = crate::paths::ax_pre_path(&session.output_dir);
    let ax_final_path = crate::paths::ax_final_path(&session.output_dir);
    crate::agent_browser::take_ax_snapshot(&session, &ax_final_path).await?;
    let har_path = crate::agent_browser::stop_har_capture(&session).await?;
    let har_bytes = read_har_bytes(&har_path)?;
    let har = crate::har::parse_har(&har_bytes)?;
    let final_url = crate::agent_browser::get_url(&session).await.ok();
    let final_title = crate::agent_browser::get_title(&session).await.ok();

    Ok(ProbeCapture {
        har_path,
        har_entry_count: har.log.entries.len(),
        final_url,
        final_title,
        ax_pre_path: ax_pre_path.exists().then_some(ax_pre_path),
        ax_final_path: Some(ax_final_path),
    })
}

pub fn read_har_bytes(path: impl AsRef<std::path::Path>) -> anyhow::Result<Vec<u8>> {
    let path = path.as_ref();
    std::fs::read(path).with_context(|| format!("failed to read HAR bytes from {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{capture_probe, finalize_capture, read_har_bytes, ProbeOptions};
    use std::path::PathBuf;

    #[test]
    fn reads_har_fixture_bytes() {
        let bytes =
            read_har_bytes(workspace_root().join("fixtures/sunat/capture.har")).expect("read fixture");
        assert!(!bytes.is_empty());
    }

    #[test]
    #[ignore = "requires a live Comet + agent-browser environment"]
    fn capture_probe_smoke() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime
            .block_on(async {
                let _ = capture_probe(ProbeOptions {
                    url: "https://www.sunat.gob.pe".to_string(),
                    visible: true,
                    output_dir: std::env::temp_dir().join("surfacer-probe-output"),
                })
                .await?;
                anyhow::Ok(())
            })
            .expect("capture probe");
    }

    #[test]
    #[ignore = "requires a live Comet + agent-browser environment"]
    fn finalize_capture_smoke() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime
            .block_on(async {
                let browser =
                    crate::agent_browser::spawn_comet(9222, std::env::temp_dir().join("surfacer-probe-test")).await?;
                let opts = ProbeOptions {
                    url: "https://www.sunat.gob.pe".to_string(),
                    visible: true,
                    output_dir: std::env::temp_dir().join("surfacer-probe-output"),
                };
                let session = crate::agent_browser::connect_session(browser, &opts).await?;
                crate::agent_browser::start_har_capture(&session).await?;
                let _ = finalize_capture(session).await?;
                anyhow::Ok(())
            })
            .expect("finalize capture");
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }
}
