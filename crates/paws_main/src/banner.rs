use std::io;

use colored::Colorize;

const BANNER_LOGO: &str = include_str!("banner");

pub struct BannerInfo {
    pub model: String,
    pub provider: String,
    pub version: String,
    pub conversation_id: String,
}

pub fn display(info: &BannerInfo) -> io::Result<()> {
    let logo_lines: Vec<&str> = BANNER_LOGO.lines().collect();
    // The logo is 11 chars wide
    let logo_width = 11;
    let content_width = 60;

    // Header
    println!(
        "╭{}┬{}╮",
        "─".repeat(logo_width + 2),
        "─".repeat(content_width + 2)
    );

    // Row 0: Logo | Session
    print_row(
        0,
        &logo_lines,
        " ● Session: ",
        &info.conversation_id,
        "green",
        logo_width,
        content_width,
    );

    // Row 1: Logo | Model
    let model_display = truncate(&info.model, 25);
    print_row(
        1,
        &logo_lines,
        " ● Model:   ",
        &model_display,
        "blue",
        logo_width,
        content_width,
    );

    // Row 2: Logo | Provider
    let provider_display = truncate(&info.provider, 25);
    print_row(
        2,
        &logo_lines,
        " ● Provider:",
        &provider_display,
        "magenta",
        logo_width,
        content_width,
    );

    // Row 3: Suggestion
    print_row(
        3,
        &logo_lines,
        "",
        "Type /help for commands",
        "yellow",
        logo_width,
        content_width,
    );

    // Bottom
    println!(
        "╰{}┴{}╯",
        "─".repeat(logo_width + 2),
        "─".repeat(content_width + 2)
    );
    println!();

    Ok(())
}

fn print_row(
    row_idx: usize,
    logo_lines: &[&str],
    label: &str,
    value: &str,
    color: &str,
    logo_width: usize,
    content_width: usize,
) {
    let logo_line = logo_lines.get(row_idx).unwrap_or(&"");

    // Style logo: first 3 lines blue, last line white bold
    let logo_styled = if row_idx < 3 {
        logo_line.blue()
    } else {
        logo_line.white().bold()
    };

    // Style value
    let value_styled = match color {
        "green" => value.green(),
        "blue" => value.blue(),
        "magenta" => value.magenta(),
        "yellow" => value.yellow(),
        _ => value.normal(),
    };

    // Calculate visible length for padding
    let label_len = label.chars().count();
    let value_len = value.chars().count();
    let space_len = if value_len > 0 { 1 } else { 0 };
    let total_len = label_len + space_len + value_len;

    let padding = content_width.saturating_sub(total_len);

    print!(
        "│ {:<width$} │ {}{}",
        logo_styled,
        label,
        if value_len > 0 { " " } else { "" },
        width = logo_width
    );
    print!("{}", value_styled);
    if padding > 0 {
        print!("{}", " ".repeat(padding));
    }
    println!(" │");
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let mut chars: String = s.chars().take(max_len - 3).collect();
        chars.push_str("...");
        chars
    } else {
        s.to_string()
    }
}
