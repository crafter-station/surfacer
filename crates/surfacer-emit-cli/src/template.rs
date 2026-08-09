pub fn shim_main_rs(site_name: &str, surfacer_binary: &str) -> String {
    format!(
        "fn main() {{
    let args: Vec<String> = std::env::args().skip(1).collect();
    let status = std::process::Command::new({surfacer_binary:?})
        .arg(\"exec\")
        .arg({site_name:?})
        .args(&args)
        .status()
        .expect(\"failed to execute surfacer\");
    std::process::exit(status.code().unwrap_or(1));
}}
",
    )
}

pub fn shim_cargo_toml(site_name: &str) -> String {
    format!(
        "[package]
name = {site_name:?}
version = \"0.1.0\"
edition = \"2024\"

[[bin]]
name = {site_name:?}
path = \"main.rs\"
"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shim_source_generation() {
        let source = shim_main_rs("sunat", "/usr/local/bin/surfacer");

        assert!(source.contains("\"sunat\""));
        assert!(source.contains("\"/usr/local/bin/surfacer\""));
        assert!(source.contains(".arg(\"exec\")"));
    }
}
