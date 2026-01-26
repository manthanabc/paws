use console::strip_ansi_codes;
use termimad::crossterm::style::{Attribute, Stylize};

use crate::display::md::render::MarkdownRenderer;
use crate::spinner::SpinnerManager;

pub struct MarkdownWriter {
    buffer: String,
    renderer: MarkdownRenderer,
    previous_rendered: String,
    last_was_dimmed: bool,
    max_height: Option<usize>,
    header: Option<String>,
}

impl MarkdownWriter {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            renderer: MarkdownRenderer::default(),
            previous_rendered: String::new(),
            last_was_dimmed: false,
            max_height: None,
            header: None,
        }
    }

    pub fn set_max_height(&mut self, max_height: Option<usize>) {
        self.max_height = max_height;
    }

    /// Sets an optional header line rendered above the buffer.
    ///
    /// # Arguments
    /// - `header`: Header text to display, or `None` to clear it.
    pub fn set_header(&mut self, header: Option<String>) {
        self.header = header;
    }

    pub fn height(&self) -> usize {
        self.renderer.height
    }
}
impl Default for MarkdownWriter {
    fn default() -> Self {
        Self::new()
    }
}
impl MarkdownWriter {
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.previous_rendered.clear();
    }

    pub fn add_chunk(&mut self, chunk: &str, spn: &mut SpinnerManager) -> anyhow::Result<()> {
        if self.last_was_dimmed {
            self.reset();
        }
        if self.buffer.is_empty() {
            spn.write_ln("").expect("Failed to write");
        }
        self.buffer.push_str(chunk);
        self.stream(&self.renderer.render(&self.buffer, None), spn);
        self.last_was_dimmed = false;
        Ok(())
    }

    pub fn add_chunk_dimmed(
        &mut self,
        chunk: &str,
        spn: &mut SpinnerManager,
    ) -> anyhow::Result<()> {
        if !self.last_was_dimmed {
            self.reset();
        }
        if self.buffer.is_empty() {
            spn.write_ln("").expect("Failed to write");
        }
        self.buffer.push_str(chunk);
        let rendered = self.renderer.render(&self.buffer, None);
        let mut lines: Vec<String> = rendered.lines().map(|line| line.to_string()).collect();
        // Always apply mild dimming to all lines
        for line in lines.iter_mut() {
            *line = dim_line_mild(line);
        }
        self.stream(&lines.join("\n"), spn);
        self.last_was_dimmed = true;
        Ok(())
    }

    pub fn clear(&mut self, spn: &mut SpinnerManager, dur: f64) {
        let msg = format!("{} Thought for {:.2}s", "⏺".cyan(), dur)
            .attribute(Attribute::Bold)
            .attribute(Attribute::Dim)
            .to_string();

        self.stream(&msg, spn);
    }

    fn stream(&mut self, content: &str, spn: &mut SpinnerManager) {
        let mut lines_new: Vec<String> = content.lines().map(|line| line.to_string()).collect();
        let lines_prev: Vec<String> = self
            .previous_rendered
            .lines()
            .map(|s| s.to_string())
            .collect();

        // Apply max_height truncation if set
        let mut was_truncated = false;
        if let Some(max_h) = self.max_height
            && lines_new.len() > max_h
        {
            let start = lines_new.len() - max_h;
            lines_new = lines_new[start..].to_vec();
            was_truncated = true;
        }

        if was_truncated {
            // Apply gradient only to first 3 lines when scrolling starts
            dim_first_3_lines_gradient(&mut lines_new);
        }

        if let Some(header) = &self.header {
            let mut header_lines = header
                .lines()
                .map(|line| line.to_string())
                .collect::<Vec<_>>();
            header_lines.append(&mut lines_new);
            lines_new = header_lines;
        }

        // Compute common prefix to minimize redraw
        let common = lines_prev
            .iter()
            .zip(&lines_new)
            .take_while(|(p, n)| p == n)
            .count();

        let lines_to_update = self.renderer.height;
        let mut skip = 0;

        // +1 to consider the spinner
        let up_base = lines_prev.len().saturating_sub(common);
        if up_base > lines_to_update {
            skip = up_base - lines_to_update;
        }
        let up_lines = up_base.saturating_sub(skip) + 1;

        // Build ANSI sequence to write
        let mut out = String::new();
        if up_lines > 0 {
            out.push_str(&format!("\x1b[{}A", up_lines)); // move up
        }
        out.push_str("\x1b[0J"); // clear from cursor down
        for line in lines_new.iter().skip(common + skip) {
            out.push_str(line);
            out.push('\n');
            out.push_str("\x1b[0G"); // move to column 0
        }

        // Write above spinner; spinner will redraw itself
        let _ = spn.write_ln(out);
        self.previous_rendered = lines_new.join("\n");
    }
}

fn dim_line_gradient(line: &str, intensity: f64) -> String {
    const RESET: &str = "\x1b[0m";

    if line.is_empty() {
        return String::new();
    }

    // ANSI 256-color gray codes from light (240) to lightest (247)
    // Top lines (intensity 1.0) = lightest gray
    // Bottom lines (intensity 0.0) = lighter gray
    let gray_code = (240.0 + (intensity * 7.0)) as u8;
    let gray_code = gray_code.min(247).max(240);

    // Strip ANSI codes for the gradient effect
    let plain_text = strip_ansi_codes(line);

    format!("\x1b[38;5;{}m{}{}", gray_code, plain_text, RESET)
}

fn dim_line_mild(line: &str) -> String {
    const DIM: &str = "\x1b[2m";
    const RESET: &str = "\x1b[0m";
    const RESET_DIM: &str = "\x1b[0m\x1b[2m";

    if line.is_empty() {
        return String::new();
    }

    let dimmed = line.replace(RESET, RESET_DIM);
    format!("{DIM}{dimmed}{RESET}")
}

fn dim_first_3_lines_gradient(lines: &mut [String]) {
    let len = lines.len();
    if len == 0 {
        return;
    }

    // Apply gradient only to first 3 lines
    for (i, line) in lines.iter_mut().take(3).enumerate() {
        // Calculate intensity: 0.0 (newest) to 1.0 (oldest of first 3)
        let intensity = if len == 1 {
            0.0
        } else {
            i as f64 / (len.min(3) - 1) as f64
        };
        *line = dim_line_gradient(line, intensity);
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use strip_ansi_escapes::strip_str;

    use super::*;

    #[test]
    fn test_markdown_writer_basic_incremental_update() {
        let mut spn = SpinnerManager::new();
        let previous_rendered = {
            let mut fixture = MarkdownWriter::new();
            fixture.stream("Line 1\nLine 2\nLine 3", &mut spn);
            fixture.previous_rendered.clone()
        };
        let expected = "Line 1\nLine 2\nLine 3";
        assert_eq!(previous_rendered, expected);
    }

    #[test]
    fn test_render_plain_text() {
        let fixture = MarkdownRenderer::new(80, 24);
        let input = "This is plain text.\n\nWith multiple lines.";
        let actual = fixture.render(input, None);
        let clean_actual = strip_str(&actual);
        assert!(clean_actual.contains("This is plain text."));
        assert!(clean_actual.contains("With multiple lines."));
    }

    #[test]
    fn test_render_multiple_code_blocks() {
        let fixture = MarkdownRenderer::new(80, 24);
        let input = "Text 1\n\n```\ncode1\n```\n\nText 2\n\n```\ncode2\n```\n\nText 3";
        let actual = fixture.render(input, None);
        let clean_actual = strip_str(&actual);
        assert!(clean_actual.contains("Text 1"));
        assert!(clean_actual.contains("code1"));
        assert!(clean_actual.contains("Text 2"));
        assert!(clean_actual.contains("code2"));
        assert!(clean_actual.contains("Text 3"));
        // Should have two reset codes for two code blocks
        let reset_count = actual.matches("\x1b[0m").count();
        assert_eq!(reset_count, 2);
    }

    #[test]
    fn test_render_unclosed_code_block() {
        let fixture = MarkdownRenderer::new(80, 24);
        let input = "Text\n\n```\nunclosed code";
        let actual = fixture.render(input, None);
        let clean_actual = strip_str(&actual);
        assert!(clean_actual.contains("Text"));
        assert!(clean_actual.contains("unclosed code"));
        assert!(actual.contains("\x1b[0m"));
    }

    #[test]
    fn test_markdown_writer_long_text_chunk_by_chunk() {
        let mut fixture = MarkdownWriter::new();
        let mut spn = SpinnerManager::new();

        let long_text = r#"# Header

This is a long paragraph with multiple sentences. It contains various types of content including some code examples.

```rust
fn main() {
    println!("Hello, world!");
    let x = 42;
    println!("The answer is {}", x);
}
```

And some more text after the code block."#;

        // Split into chunks and add with spaces
        let chunks = long_text.split_whitespace().collect::<Vec<_>>();
        for chunk in chunks {
            fixture
                .add_chunk(&format!("{} ", chunk), &mut spn)
                .expect("EEE");
        }

        assert!(fixture.buffer.contains("Header"));
        assert!(fixture.buffer.contains("println!"));
        assert!(fixture.buffer.contains("Hello, world!"));
        assert!(fixture.buffer.contains("more text"));
    }

    #[test]
    fn test_markdown_writer_header_line() {
        let mut fixture = MarkdownWriter::new();
        let mut spn = SpinnerManager::new();

        fixture.set_header(Some("Thinking..".to_string()));
        fixture.stream("Line 1", &mut spn);

        let actual = fixture.previous_rendered.clone();
        let expected = "Thinking..\nLine 1";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_markdown_writer_truncation_dims_top_line() {
        let mut fixture = MarkdownWriter::new();
        let mut spn = SpinnerManager::new();

        fixture.set_max_height(Some(2));
        fixture.stream("Line 1\nLine 2\nLine 3", &mut spn);

        let actual = fixture.previous_rendered.clone();
        // After truncation, gradient is applied to all lines
        // Line 2 (index 0) should have lighter gray
        // Line 3 (index 1) should have darker gray
        assert!(actual.contains("\x1b[38;5;")); // Contains gradient codes
        assert!(actual.contains("Line 2"));
        assert!(actual.contains("Line 3"));
    }

    #[test]
    fn test_dim_first_3_lines_gradient() {
        let mut lines = vec![
            "Line 1".to_string(),
            "Line 2".to_string(),
            "Line 3".to_string(),
        ];
        dim_first_3_lines_gradient(&mut lines);

        // Check that all lines have been transformed with gradient ANSI codes
        assert!(lines[0].contains("\x1b[38;5;"));
        assert!(lines[1].contains("\x1b[38;5;"));
        assert!(lines[2].contains("\x1b[38;5;"));

        // Check that the gradient is applied (different gray codes)
        // The exact codes depend on the intensity calculation
        assert_ne!(lines[0], lines[1]);
        assert_ne!(lines[1], lines[2]);
    }

    #[test]
    fn test_dim_line_mild() {
        let line = "Some text";
        let dimmed = dim_line_mild(line);

        // Should contain dim code
        assert!(dimmed.contains("\x1b[2m"));
        // Should contain reset code
        assert!(dimmed.contains("\x1b[0m"));
        // Should contain the original text
        assert!(dimmed.contains("Some text"));
    }
}
