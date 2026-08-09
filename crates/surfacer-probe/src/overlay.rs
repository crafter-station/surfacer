use crate::capture::LiveProbeStats;

pub enum ProbeOverlayEvent {
    Done,
    Abort,
}

pub fn overlay_show_stats(stats: &LiveProbeStats) -> String {
    format!(
        "iterations: {} | endpoints: {} | requests: {}",
        stats.iterations, stats.endpoint_count, stats.request_count
    )
}

pub fn handle_done_click(event: ProbeOverlayEvent) -> bool {
    matches!(event, ProbeOverlayEvent::Done)
}

pub fn handle_abort_click(event: ProbeOverlayEvent) -> bool {
    matches!(event, ProbeOverlayEvent::Abort)
}

#[cfg(test)]
mod tests {
    use super::{handle_abort_click, handle_done_click, overlay_show_stats, ProbeOverlayEvent};
    use crate::capture::LiveProbeStats;

    #[test]
    fn formats_overlay_stats() {
        let stats = LiveProbeStats {
            iterations: 7,
            endpoint_count: 75,
            request_count: 412,
        };

        assert_eq!(
            overlay_show_stats(&stats),
            "iterations: 7 | endpoints: 75 | requests: 412"
        );
    }

    #[test]
    fn detects_done_click() {
        assert!(handle_done_click(ProbeOverlayEvent::Done));
        assert!(!handle_done_click(ProbeOverlayEvent::Abort));
    }

    #[test]
    fn detects_abort_click() {
        assert!(handle_abort_click(ProbeOverlayEvent::Abort));
        assert!(!handle_abort_click(ProbeOverlayEvent::Done));
    }
}
