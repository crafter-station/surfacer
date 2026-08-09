use std::path::{Path, PathBuf};

pub fn probe_output_dir(base: &Path, site_name: &str) -> PathBuf {
    base.join(format!("probe-{site_name}"))
}

pub fn har_path(output_dir: &Path) -> PathBuf {
    output_dir.join("capture.har")
}

pub fn ax_pre_path(output_dir: &Path) -> PathBuf {
    output_dir.join("ax-pre.txt")
}

pub fn ax_final_path(output_dir: &Path) -> PathBuf {
    output_dir.join("ax-final.txt")
}

#[cfg(test)]
mod tests {
    use super::{ax_final_path, ax_pre_path, har_path, probe_output_dir};
    use std::path::Path;

    #[test]
    fn builds_probe_paths() {
        let base = Path::new("/tmp/surfacer");
        let output_dir = probe_output_dir(base, "sunat-gob-pe");

        assert_eq!(output_dir, base.join("probe-sunat-gob-pe"));
        assert_eq!(har_path(&output_dir), output_dir.join("capture.har"));
        assert_eq!(ax_pre_path(&output_dir), output_dir.join("ax-pre.txt"));
        assert_eq!(ax_final_path(&output_dir), output_dir.join("ax-final.txt"));
    }
}
