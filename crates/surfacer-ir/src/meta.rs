use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteMeta {
    pub site_name: String,
    pub display_name: String,
    pub source_url: String,
    pub ir_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub generated_at: String,
    pub technique: ProvenanceTechnique,
    pub classifier_bucket: String,
    pub probe_duration_sec: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProvenanceTechnique {
    Http,
    Ax,
    Hybrid,
    /// IR produced by an agent doing recon with judgment, rather than by a
    /// mechanical pass over observed traffic or an accessibility tree.
    ///
    /// A consumer has to be able to tell this apart from the mechanical
    /// variants because the reproducibility guarantees differ. `Http`, `Ax`
    /// and `Hybrid` come from a deterministic derivation over a capture: the
    /// same capture yields the same IR, and a field is present because it was
    /// observed. `Agent` IR is the output of a judgment call: which terrain the
    /// surface is, which endpoints are worth naming, what an official spec says
    /// a parameter means. Re-running the recon can legitimately produce a
    /// different, better IR, and a field can be present because the agent read
    /// documentation rather than because traffic showed it.
    ///
    /// So: do not treat this IR as a replayable artifact of a capture, and do
    /// not offer to regenerate it with a mechanical crawler, which would
    /// discard the judgment that produced it.
    Agent,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(technique: &ProvenanceTechnique) -> ProvenanceTechnique {
        let json = serde_json::to_string(technique).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn agent_technique_roundtrips() {
        let technique = ProvenanceTechnique::Agent;
        let json = serde_json::to_string(&technique).unwrap();
        assert_eq!(json, r#""agent""#);
        assert!(matches!(roundtrip(&technique), ProvenanceTechnique::Agent));
    }

    #[test]
    fn mechanical_techniques_keep_their_wire_names() {
        // The new variant must not shift the existing ones: IR already on disk
        // carries these strings.
        assert_eq!(
            serde_json::to_string(&ProvenanceTechnique::Http).unwrap(),
            r#""http""#
        );
        assert_eq!(
            serde_json::to_string(&ProvenanceTechnique::Ax).unwrap(),
            r#""ax""#
        );
        assert_eq!(
            serde_json::to_string(&ProvenanceTechnique::Hybrid).unwrap(),
            r#""hybrid""#
        );
        assert!(matches!(
            roundtrip(&ProvenanceTechnique::Http),
            ProvenanceTechnique::Http
        ));
    }

    #[test]
    fn agent_provenance_roundtrips_inside_a_descriptor() {
        // Provenance is what `check` reads to decide how to word a drift fix,
        // so the variant has to survive a trip through the enclosing struct.
        let provenance = Provenance {
            generated_at: "2026-08-17T00:00:00Z".into(),
            technique: ProvenanceTechnique::Agent,
            classifier_bucket: "RestModernSpa".into(),
            probe_duration_sec: 0,
        };
        let json = serde_json::to_string(&provenance).unwrap();
        assert!(json.contains(r#""technique":"agent""#));

        let parsed: Provenance = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed.technique, ProvenanceTechnique::Agent));
    }
}
