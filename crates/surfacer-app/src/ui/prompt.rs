use std::io::{self, Write};

use anyhow::{Context, anyhow};

pub fn confirm(message: &str) -> anyhow::Result<bool> {
    print!("{message} [y/N]: ");
    io::stdout().flush().context("failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read confirmation from stdin")?;

    let normalized = input.trim().to_ascii_lowercase();
    Ok(matches!(normalized.as_str(), "y" | "yes"))
}

#[allow(dead_code)]
pub fn select(message: &str, options: &[&str]) -> anyhow::Result<usize> {
    if options.is_empty() {
        return Err(anyhow!("no options provided"));
    }

    println!("{message}");
    for (index, option) in options.iter().enumerate() {
        println!("{}. {}", index + 1, option);
    }
    print!("Select an option [1-{}]: ", options.len());
    io::stdout().flush().context("failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read selection from stdin")?;

    let selection = input
        .trim()
        .parse::<usize>()
        .context("selection must be a number")?;

    if (1..=options.len()).contains(&selection) {
        Ok(selection - 1)
    } else {
        Err(anyhow!("selection out of range"))
    }
}
