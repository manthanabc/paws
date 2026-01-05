use std::io;

use colored::Colorize;
use crossterm::terminal::size;

const BANNER_LOGO: &str = include_str!("banner");

pub struct BannerInfo {
    pub model: String,
    pub provider: String,
    pub version: String,
    pub conversation_id: String,
}

pub fn display(info: &BannerInfo) -> io::Result<()> {
    let logo_lines: Vec<&str> = BANNER_LOGO.lines().collect();
    let (term_width, _) = size().unwrap_or((80, 24));
    let term_width = term_width as usize;

    // Switch to vertical stacked layout for terminals narrower than 80 columns
    if term_width < 80 {
        // Use a reasonable width for the vertical box, but ensure it fits in terminal
        let box_width = term_width.min(50).saturating_sub(2); // -2 for margins
        let inner_width = box_width.saturating_sub(2); // -2 for borders

        // Top Border
        println!("╭{}╮", "─".repeat(inner_width));

        // Logo Section (Centered)
        for (i, line) in logo_lines.iter().enumerate() {
            let styled = if i < 3 {
                line.blue()
            } else {
                line.white().bold()
            };

            // Calculate padding to center the logo
            let logo_len = 11; // Logo is fixed width
            let padding_left = (inner_width.saturating_sub(logo_len)) / 2;
            let padding_right = inner_width
                .saturating_sub(logo_len)
                .saturating_sub(padding_left);

            println!(
                "│{}{}{}│",
                " ".repeat(padding_left),
                styled,
                " ".repeat(padding_right)
            );
        }

        // Empty Separator Line
        println!("│{}│", " ".repeat(inner_width));

        // Info Section
        let label_width = 12; // " ● Session: " is 12 chars
        let max_val_len = inner_width.saturating_sub(label_width);

        let print_info_line =
            |label: &str, value: &str, color_fn: fn(&str) -> colored::ColoredString| {
                let truncated_val = truncate(value, max_val_len);
                let colored_val = color_fn(&truncated_val);
                let total_content_len = label.chars().count() + truncated_val.chars().count();
                let padding = inner_width.saturating_sub(total_content_len);

                println!("│{}{}{}│", label, colored_val, " ".repeat(padding));
            };

        print_info_line(" ● Session: ", &info.conversation_id, |s| s.green());
        print_info_line(" ● Model:   ", &info.model, |s| s.blue());
        print_info_line(" ● Provider:", &info.provider, |s| s.magenta());

        // Empty Separator Line
        println!("│{}│", " ".repeat(inner_width));

        // Help Suggestion (Centered)
        let help_text = "Type /help for commands";
        let help_len = help_text.chars().count();
        let h_pad_left = (inner_width.saturating_sub(help_len)) / 2;
        let h_pad_right = inner_width
            .saturating_sub(help_len)
            .saturating_sub(h_pad_left);
        println!(
            "│{}{}{}│",
            " ".repeat(h_pad_left),
            help_text.yellow(),
            " ".repeat(h_pad_right)
        );

        // Bottom Border
        println!("╰{}╯", "─".repeat(inner_width));
        println!();

        return Ok(());
    }

    // Horizontal Layout
    // The logo is 11 chars wide
    let logo_width = 11;

    // Calculate available width based on terminal size
    // Border (1) + Logo (11) + Padding (2) + Separator (1) + Padding (2) + Border
    // (1) = 18 chars fixed
    let available_width = term_width.saturating_sub(18);
    // Keep reasonable bounds: min 40 for readability, max 80 (reduced from 100)
    let content_width = available_width.clamp(40, 80);

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

    // Calculate max length for values (content_width - label_len - space)
    // Label is approx 12 chars (" ● Model:   ") + 1 space = 13
    let max_value_len = content_width.saturating_sub(13);

    // Row 1: Logo | Model
    let model_display = truncate(&info.model, max_value_len);
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
    let provider_display = truncate(&info.provider, max_value_len);
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
