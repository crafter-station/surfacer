use indicatif::{ProgressBar, ProgressStyle};

#[allow(dead_code)]
pub fn spinner(message: &str) -> indicatif::ProgressBar {
    let progress = ProgressBar::new_spinner();
    progress.set_message(message.to_string());
    progress.set_style(
        ProgressStyle::with_template("{spinner} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    progress.enable_steady_tick(std::time::Duration::from_millis(100));
    progress
}

#[allow(dead_code)]
pub fn bar(total: u64, message: &str) -> indicatif::ProgressBar {
    let progress = ProgressBar::new(total);
    progress.set_message(message.to_string());
    progress.set_style(
        ProgressStyle::with_template("{bar:40.cyan/blue} {pos}/{len} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_bar()),
    );
    progress
}
