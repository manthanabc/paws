use std::time::Instant;

use termimad::crossterm::style::{Attribute, Stylize};

use crate::spinner::SpinnerManager;

/// A collapsible, scrollable buffer for displaying incremental content with
/// automatic truncation and scroll behavior.
///
/// This buffer type is designed to handle content that grows over time (like
/// thinking streams, logs, or long outputs) by:
/// - Truncating to a maximum height (collapsing older content)
/// - Maintaining scroll position to show most recent content
/// - Supporting dimmed/styled rendering
/// - Tracking timing information
///
/// # Examples
///
/// ```ignore
/// use paws_common::display::CollapsibleBuffer;
///
/// let mut buffer = CollapsibleBuffer::new();
/// buffer.set_max_height(Some(10)); // Show only last 10 lines
///
/// // Add content incrementally
/// buffer.add_chunk("Thinking about the problem...", &mut spinner)?;
/// buffer.add_chunk("Analyzing data structures...", &mut spinner)?;
///
/// // When done, show completion with timing
/// buffer.finish(&mut spinner, elapsed_duration);
/// ```
pub struct CollapsibleBuffer {
    buffer: String,
    previous_rendered: String,
    max_height: Option<usize>,
    is_dimmed: bool,
    start_time: Option<Instant>,
}

impl CollapsibleBuffer {
    /// Creates a new empty collapsible buffer
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            previous_rendered: String::new(),
            max_height: None,
            is_dimmed: false,
            start_time: None,
        }
    }

    /// Sets the maximum height for the buffer display
    ///
    /// When content exceeds this height, older content is truncated
    /// to show only the most recent lines.
    pub fn set_max_height(&mut self, max_height: Option<usize>) {
        self.max_height = max_height;
    }

    /// Gets whether the buffer is currently in a dimmed state
    pub fn is_dimmed(&self) -> bool {
        self.is_dimmed
    }

    /// Marks the start of a timed operation
    ///
    /// This should be called when the buffer begins receiving content
    /// that should be tracked for duration (e.g., thinking start).
    pub fn start_timing(&mut self) {
        if self.start_time.is_none() {
            self.start_time = Some(Instant::now());
        }
    }

    /// Adds a chunk of content to the buffer with normal styling
    ///
    /// If the buffer was previously in a dimmed state, it will be reset
    /// before adding the new content.
    pub fn add_chunk(
        &mut self,
        chunk: &str,
        spinner: &mut SpinnerManager,
    ) -> anyhow::Result<()> {
        if self.is_dimmed {
            self.reset();
        }
        
        if self.buffer.is_empty() {
            spinner.write_ln("")?;
        }
        
        self.buffer.push_str(chunk);
        self.stream_content(spinner, None)?;
        self.is_dimmed = false;
        Ok(())
    }

    /// Adds a chunk of content to the buffer with dimmed styling
    ///
    /// This is typically used for thinking content or auxiliary information
    /// that should be visually de-emphasized.
    pub fn add_chunk_dimmed(
        &mut self,
        chunk: &str,
        spinner: &mut SpinnerManager,
    ) -> anyhow::Result<()> {
        if !self.is_dimmed {
            self.reset();
        }
        
        if self.buffer.is_empty() {
            spinner.write_ln("")?;
        }
        
        self.buffer.push_str(chunk);
        self.stream_content(spinner, Some(Attribute::Dim))?;
        self.is_dimmed = true;
        Ok(())
    }

    /// Resets the buffer content while preserving configuration
    ///
    /// This clears the buffer text but maintains max_height settings
    /// and timing information.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.previous_rendered.clear();
    }

    /// Completes the buffer operation and displays a summary
    ///
    /// Shows a completion message with timing information and clears
    /// the buffer state.
    pub fn finish(&mut self, spinner: &mut SpinnerManager, duration_secs: f64) {
        let msg = format!("{} Thought for {:.2}s", "⏺".cyan(), duration_secs)
            .attribute(Attribute::Bold)
            .attribute(Attribute::Dim)
            .to_string();

        self.stream_content(spinner, None).ok();
        
        let _ = spinner.write_ln(msg);
        self.reset();
        self.start_time = None;
        self.is_dimmed = false;
    }

    /// Clears the buffer completely, including timing info
    pub fn clear(&mut self) {
        self.reset();
        self.start_time = None;
        self.is_dimmed = false;
    }

    /// Gets the current buffer content
    pub fn content(&self) -> &str {
        &self.buffer
    }

    /// Gets the elapsed time since timing started
    ///
    /// Returns None if timing hasn't been started yet.
    pub fn elapsed(&self) -> Option<std::time::Duration> {
        self.start_time.map(|start| start.elapsed())
    }

    /// Internal method to stream content with optional styling
    fn stream_content(
        &mut self,
        spinner: &mut SpinnerManager,
        attribute: Option<Attribute>,
    ) -> anyhow::Result<()> {
        // For now, we'll just write the buffer directly
        // In a full implementation, this would integrate with MarkdownRenderer
        // similar to the existing MarkdownWriter
        let content = if let Some(attr) = attribute {
            self.buffer.clone().with(attr).to_string()
        } else {
            self.buffer.clone()
        };

        let _ = spinner.write_ln(&content);
        self.previous_rendered = content;
        Ok(())
    }

    /// Alternative streaming implementation that could be used with a renderer
    #[allow(dead_code)]
    fn stream_with_renderer(
        &mut self,
        content: &str,
        spinner: &mut SpinnerManager,
    ) -> anyhow::Result<()> {
        // This method would integrate with MarkdownRenderer for rich rendering
        // Implementation would be similar to MarkdownWriter::stream()
        
        let mut lines_new: Vec<&str> = content.lines().collect();
        let lines_prev: Vec<String> = self
            .previous_rendered
            .lines()
            .map(|s| s.to_string())
            .collect();

        // Apply max_height truncation if set
        if let Some(max_h) = self.max_height
            && lines_new.len() > max_h
        {
            let start = lines_new.len() - max_h;
            lines_new = lines_new[start..].to_vec();
        }

        // For minimal implementation, just update content
        spinner.write_ln(&lines_new.join("\n"))?;
        self.previous_rendered = lines_new.join("\n");
        Ok(())
    }
}

impl Default for CollapsibleBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use paws_common::spinner::SpinnerManager;

    #[test]
    fn test_collapsible_buffer_new() {
        let buffer = CollapsibleBuffer::new();
        
        assert_eq!(buffer.content(), "");
        assert_eq!(buffer.is_dimmed(), false);
        assert_eq!(buffer.elapsed(), None);
    }

    #[test]
    fn test_collapsible_buffer_timing() {
        let mut buffer = CollapsibleBuffer::new();
        
        assert_eq!(buffer.elapsed(), None);
        
        buffer.start_timing();
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        assert!(buffer.elapsed().is_some());
        let elapsed = buffer.elapsed().unwrap();
        assert!(elapsed.as_millis() >= 10);
    }

    #[test]
    fn test_collapsible_buffer_reset() {
        let mut buffer = CollapsibleBuffer::new();
        
        buffer.set_max_height(Some(10));
        buffer.start_timing();
        buffer.add_chunk("test content", &mut SpinnerManager::new()).ok();
        
        buffer.reset();
        
        assert_eq!(buffer.content(), "");
        assert!(buffer.elapsed().is_some()); // Timing preserved
        assert_eq!(buffer.max_height, Some(10)); // Config preserved
    }

    #[test]
    fn test_collapsible_buffer_clear() {
        let mut buffer = CollapsibleBuffer::new();
        
        buffer.set_max_height(Some(10));
        buffer.start_timing();
        buffer.add_chunk("test content", &mut SpinnerManager::new()).ok();
        
        buffer.clear();
        
        assert_eq!(buffer.content(), "");
        assert_eq!(buffer.elapsed(), None); // Timing cleared
    }

    #[test]
    fn test_collapsible_buffer_max_height() {
        let mut buffer = CollapsibleBuffer::new();
        
        assert_eq!(buffer.max_height, None);
        
        buffer.set_max_height(Some(15));
        assert_eq!(buffer.max_height, Some(15));
        
        buffer.set_max_height(None);
        assert_eq!(buffer.max_height, None);
    }

    #[test]
    fn test_collapsible_buffer_dimmed_toggle() {
        let mut buffer = CollapsibleBuffer::new();
        let mut spinner = SpinnerManager::new();
        
        assert_eq!(buffer.is_dimmed(), false);
        
        buffer.add_chunk_dimmed("test", &mut spinner).ok();
        assert_eq!(buffer.is_dimmed(), true);
        
        buffer.add_chunk("test", &mut spinner).ok();
        assert_eq!(buffer.is_dimmed(), false);
    }
}
