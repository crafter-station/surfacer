use surfacer_ir::AxAction;

pub fn extract_ax_actions(ax_text: &str) -> Vec<AxAction> {
    ax_text.lines().filter_map(parse_ax_action).collect()
}

fn parse_ax_action(line: &str) -> Option<AxAction> {
    let trimmed = line.trim_start();
    let role = ["link", "button", "textbox", "menuitem"]
        .into_iter()
        .find(|candidate| trimmed.starts_with(candidate))?;
    let after_role = trimmed[role.len()..].trim_start();
    if !after_role.starts_with("@e") {
        return None;
    }

    let text = trimmed
        .split('"')
        .nth(1)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(role);

    Some(AxAction {
        command_path: command_path_from_text(text, role),
        description: text.to_string(),
    })
}

fn command_path_from_text(text: &str, fallback: &str) -> Vec<String> {
    let parts = text
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect::<Vec<_>>();

    if parts.is_empty() {
        vec![fallback.to_string()]
    } else {
        parts
    }
}
