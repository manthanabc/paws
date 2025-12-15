use std::cmp::min;
use std::path::{Path, PathBuf};

use console::strip_ansi_codes;
use derive_setters::Setters;
use paws_common::display::DiffFormat;
use paws_common::template::Element;
use paws_domain::{
    Environment, FSPatch, FSRead, FSRemove, FSSearch, FSUndo, FSWrite,
    FileOperation, LineNumbers, Metrics, NetFetch, PlanCreate, ToolKind,
};

use crate::truncation::{
    Stderr, Stdout, TruncationMode, truncate_fetch_content, truncate_search_output,
    truncate_shell_output,
};
use crate::utils::{compute_hash, format_display_path};
use crate::{
    FsCreateOutput, FsRemoveOutput, FsUndoOutput, HttpResponse, PatchOutput, PlanCreateOutput,
    ReadOutput, ResponseContext, SearchResult, ShellOutput,
};

#[derive(Debug, Default, Setters)]
#[setters(into, strip_option)]
pub struct TempContentFiles {
    stdout: Option<PathBuf>,
    stderr: Option<PathBuf>,
}

#[derive(Debug, derive_more::From)]
pub enum ToolOperation {
    FsRead {
        input: FSRead,
        output: ReadOutput,
    },
    ImageRead {
        output: paws_domain::Image,
    },
    FsCreate {
        input: FSWrite,
        output: FsCreateOutput,
    },
    FsRemove {
        input: FSRemove,
        output: FsRemoveOutput,
    },
    FsSearch {
        input: FSSearch,
        output: Option<SearchResult>,
    },
    FsPatch {
        input: FSPatch,
        output: PatchOutput,
    },
    FsUndo {
        input: FSUndo,
        output: FsUndoOutput,
    },
    NetFetch {
        input: NetFetch,
        output: HttpResponse,
    },
    Shell {
        output: ShellOutput,
    },
    FollowUp {
        output: Option<String>,
    },
    PlanCreate {
        input: PlanCreate,
        output: PlanCreateOutput,
    },
    Skill {
        #[allow(dead_code)]
        input: paws_domain::SkillFetch,
        output: paws_domain::Skill,
    },
}

/// Trait for stream elements that can be converted to XML elements
pub trait StreamElement {
    fn stream_name(&self) -> &'static str;
    fn head_content(&self) -> &str;
    fn tail_content(&self) -> Option<&str>;
    fn total_lines(&self) -> usize;
    fn head_end_line(&self) -> usize;
    fn tail_start_line(&self) -> Option<usize>;
    fn tail_end_line(&self) -> Option<usize>;
}

impl StreamElement for Stdout {
    fn stream_name(&self) -> &'static str {
        "stdout"
    }

    fn head_content(&self) -> &str {
        &self.head
    }

    fn tail_content(&self) -> Option<&str> {
        self.tail.as_deref()
    }

    fn total_lines(&self) -> usize {
        self.total_lines
    }

    fn head_end_line(&self) -> usize {
        self.head_end_line
    }

    fn tail_start_line(&self) -> Option<usize> {
        self.tail_start_line
    }

    fn tail_end_line(&self) -> Option<usize> {
        self.tail_end_line
    }
}

impl StreamElement for Stderr {
    fn stream_name(&self) -> &'static str {
        "stderr"
    }

    fn head_content(&self) -> &str {
        &self.head
    }

    fn tail_content(&self) -> Option<&str> {
        self.tail.as_deref()
    }

    fn total_lines(&self) -> usize {
        self.total_lines
    }

    fn head_end_line(&self) -> usize {
        self.head_end_line
    }

    fn tail_start_line(&self) -> Option<usize> {
        self.tail_start_line
    }

    fn tail_end_line(&self) -> Option<usize> {
        self.tail_end_line
    }
}

/// Helper function to create stdout or stderr elements with consistent
/// structure
fn create_stream_element<T: StreamElement>(
    stream: &T,
    full_output_path: Option<&Path>,
) -> Option<Element> {
    if stream.head_content().is_empty() {
        return None;
    }

    let mut elem = Element::new(stream.stream_name()).attr("total_lines", stream.total_lines());

    elem = if let Some(((tail, tail_start), tail_end)) = stream
        .tail_content()
        .zip(stream.tail_start_line())
        .zip(stream.tail_end_line())
    {
        elem.append(
            Element::new("head")
                .attr("display_lines", format!("1-{}", stream.head_end_line()))
                .cdata(stream.head_content()),
        )
        .append(
            Element::new("tail")
                .attr("display_lines", format!("{tail_start}-{tail_end}"))
                .cdata(tail),
        )
    } else {
        elem.cdata(stream.head_content())
    };

    if let Some(path) = full_output_path {
        elem = elem.attr("full_output", path.display());
    }

    Some(elem)
}
impl ToolOperation {
    pub fn into_tool_output(
        self,
        tool_kind: ToolKind,
        content_files: TempContentFiles,
        env: &Environment,
        metrics: &mut Metrics,
    ) -> paws_domain::ToolOutput {
        let tool_name = tool_kind.name();
        match self {
            ToolOperation::FsRead { input, output } => {
                let content = output.content.file_content();
                let content = if input.show_line_numbers {
                    content.to_numbered_from(output.start_line as usize)
                } else {
                    content.to_string()
                };
                let elm = Element::new("file")
                    .attr("path", &input.path)
                    .attr(
                        "display_lines",
                        format!("{}-{}", output.start_line, output.end_line),
                    )
                    .attr("total_lines", content.lines().count())
                    .cdata(content);

                // Track read operations
                tracing::info!(path = %input.path, tool = %tool_name, "File read");
                *metrics = metrics.clone().insert(
                    input.path.clone(),
                    FileOperation::new(tool_kind).content_hash(Some(output.content_hash.clone())),
                );

                paws_domain::ToolOutput::text(elm)
            }
            ToolOperation::ImageRead { output } => paws_domain::ToolOutput::image(output),
            ToolOperation::FsCreate { input, output } => {
                let diff_result = DiffFormat::format(
                    output.before.as_ref().unwrap_or(&"".to_string()),
                    &input.content,
                );
                let diff = console::strip_ansi_codes(diff_result.diff()).to_string();

                *metrics = metrics.clone().insert(
                    input.path.clone(),
                    FileOperation::new(tool_kind)
                        .lines_added(diff_result.lines_added())
                        .lines_removed(diff_result.lines_removed())
                        .content_hash(Some(output.content_hash.clone())),
                );

                let mut elm = if output.before.as_ref().is_some() {
                    Element::new("file_overwritten").append(Element::new("file_diff").cdata(diff))
                } else {
                    Element::new("file_created")
                };

                elm = elm
                    .attr("path", input.path)
                    .attr("total_lines", input.content.lines().count());

                paws_domain::ToolOutput::text(elm)
            }
            ToolOperation::FsRemove { input, output } => {
                // None since file was removed
                let content_hash = None;

                *metrics = metrics.clone().insert(
                    input.path.clone(),
                    FileOperation::new(tool_kind)
                        .lines_removed(output.content.lines().count() as u64)
                        .content_hash(content_hash),
                );

                let display_path = format_display_path(Path::new(&input.path), env.cwd.as_path());
                let elem = Element::new("file_removed")
                    .attr("path", display_path)
                    .attr("status", "completed");
                paws_domain::ToolOutput::text(elem)
            }

            ToolOperation::FsSearch { input, output } => match output {
                Some(out) => {
                    let max_lines = min(
                        env.max_search_lines,
                        input.max_search_lines.unwrap_or(i32::MAX) as usize,
                    );
                    let start_index = input.start_index.unwrap_or(1);
                    let start_index = if start_index > 0 { start_index - 1 } else { 0 };
                    let search_dir = Path::new(&input.path);
                    let truncated_output = truncate_search_output(
                        &out.matches,
                        start_index as usize,
                        max_lines,
                        env.max_search_result_bytes,
                        search_dir,
                    );

                    let display_lines = if truncated_output.start < truncated_output.end {
                        // 1 Line based indexing
                        let new_start = truncated_output.start.saturating_add(1);
                        format!("{}-{}", new_start, truncated_output.end)
                    } else {
                        format!("{}-{}", truncated_output.start, truncated_output.end)
                    };

                    let mut elm = Element::new("search_results")
                        .attr("path", &input.path)
                        .attr("max_bytes_allowed", env.max_search_result_bytes)
                        .attr("total_lines", truncated_output.total)
                        .attr("display_lines", display_lines);

                    elm = elm.attr_if_some("regex", input.regex);
                    elm = elm.attr_if_some("file_pattern", input.file_pattern);

                    match truncated_output.strategy {
                        TruncationMode::Byte => {
                            let reason = format!(
                                "Results truncated due to exceeding the {} bytes size limit. Please use a more specific search pattern",
                                env.max_search_result_bytes
                            );
                            elm = elm.attr("reason", reason);
                        }
                        TruncationMode::Line => {
                            let reason = format!(
                                "Results truncated due to exceeding the {max_lines} lines limit. Please use a more specific search pattern"
                            );
                            elm = elm.attr("reason", reason);
                        }
                        TruncationMode::Full => {}
                    };
                    elm = elm.cdata(truncated_output.data.join("\n"));

                    paws_domain::ToolOutput::text(elm)
                }
                None => {
                    let mut elm = Element::new("search_results").attr("path", &input.path);
                    elm = elm.attr_if_some("regex", input.regex);
                    elm = elm.attr_if_some("file_pattern", input.file_pattern);

                    paws_domain::ToolOutput::text(elm)
                }
            },
            ToolOperation::FsPatch { input, output } => {
                let diff_result = DiffFormat::format(&output.before, &output.after);
                let diff = console::strip_ansi_codes(diff_result.diff()).to_string();

                let elm = Element::new("file_diff")
                    .attr("path", &input.path)
                    .attr("total_lines", output.after.lines().count())
                    .cdata(diff);

                *metrics = metrics.clone().insert(
                    input.path.clone(),
                    FileOperation::new(tool_kind)
                        .lines_added(diff_result.lines_added())
                        .lines_removed(diff_result.lines_removed())
                        .content_hash(Some(output.content_hash.clone())),
                );

                paws_domain::ToolOutput::text(elm)
            }
            ToolOperation::FsUndo { input, output } => {
                // Diff between snapshot state (after_undo) and modified state
                // (before_undo)
                let diff = DiffFormat::format(
                    output.after_undo.as_deref().unwrap_or(""),
                    output.before_undo.as_deref().unwrap_or(""),
                );
                let content_hash = output.after_undo.as_ref().map(|s| compute_hash(s));

                *metrics = metrics.clone().insert(
                    input.path.clone(),
                    FileOperation::new(tool_kind)
                        .lines_added(diff.lines_added())
                        .lines_removed(diff.lines_removed())
                        .content_hash(content_hash),
                );

                match (&output.before_undo, &output.after_undo) {
                    (None, None) => {
                        let elm = Element::new("file_undo")
                            .attr("path", input.path)
                            .attr("status", "no_changes");
                        paws_domain::ToolOutput::text(elm)
                    }
                    (None, Some(after)) => {
                        let elm = Element::new("file_undo")
                            .attr("path", input.path)
                            .attr("status", "created")
                            .attr("total_lines", after.lines().count())
                            .cdata(after);
                        paws_domain::ToolOutput::text(elm)
                    }
                    (Some(before), None) => {
                        let elm = Element::new("file_undo")
                            .attr("path", input.path)
                            .attr("status", "removed")
                            .attr("total_lines", before.lines().count())
                            .cdata(before);
                        paws_domain::ToolOutput::text(elm)
                    }
                    (Some(before), Some(after)) => {
                        // This diff is between modified state (before_undo) and snapshot
                        // state (after_undo)
                        let diff = DiffFormat::format(before, after);

                        let elm = Element::new("file_undo")
                            .attr("path", input.path)
                            .attr("status", "restored")
                            .cdata(strip_ansi_codes(diff.diff()));

                        paws_domain::ToolOutput::text(elm)
                    }
                }
            }
            ToolOperation::NetFetch { input, output } => {
                let content_type = match output.context {
                    ResponseContext::Parsed => "text/markdown".to_string(),
                    ResponseContext::Raw => output.content_type,
                };
                let truncated_content =
                    truncate_fetch_content(&output.content, env.fetch_truncation_limit);
                let mut elm = Element::new("http_response")
                    .attr("url", &input.url)
                    .attr("status_code", output.code)
                    .attr("start_char", 0)
                    .attr(
                        "end_char",
                        env.fetch_truncation_limit.min(output.content.len()),
                    )
                    .attr("total_chars", output.content.len())
                    .attr("content_type", content_type);

                elm = elm.append(Element::new("body").cdata(truncated_content.content));
                if let Some(path) = content_files.stdout {
                    elm = elm.append(Element::new("truncated").text(
                        format!(
                            "Content is truncated to {} chars, remaining content can be read from path: {}",
                            env.fetch_truncation_limit, path.display())
                    ));
                }

                paws_domain::ToolOutput::text(elm)
            }
            ToolOperation::Shell { output } => {
                let mut parent_elem = Element::new("shell_output")
                    .attr("command", &output.output.command)
                    .attr("shell", &output.shell);

                if let Some(exit_code) = output.output.exit_code {
                    parent_elem = parent_elem.attr("exit_code", exit_code);
                }

                let truncated_output = truncate_shell_output(
                    &output.output.stdout,
                    &output.output.stderr,
                    env.stdout_max_prefix_length,
                    env.stdout_max_suffix_length,
                    env.stdout_max_line_length,
                );

                let stdout_elem = create_stream_element(
                    &truncated_output.stdout,
                    content_files.stdout.as_deref(),
                );

                let stderr_elem = create_stream_element(
                    &truncated_output.stderr,
                    content_files.stderr.as_deref(),
                );

                parent_elem = parent_elem.append(stdout_elem);
                parent_elem = parent_elem.append(stderr_elem);

                paws_domain::ToolOutput::text(parent_elem)
            }
            ToolOperation::FollowUp { output } => match output {
                None => {
                    let elm = Element::new("interrupted").text("No feedback provided");
                    paws_domain::ToolOutput::text(elm)
                }
                Some(content) => {
                    let elm = Element::new("feedback").text(content);
                    paws_domain::ToolOutput::text(elm)
                }
            },
            ToolOperation::PlanCreate { input, output } => {
                let elm = Element::new("plan_created")
                    .attr("path", output.path.display().to_string())
                    .attr("plan_name", input.plan_name)
                    .attr("version", input.version);

                paws_domain::ToolOutput::text(elm)
            }
            ToolOperation::Skill { input: _, output } => {
                let mut elm = Element::new("skill_details");

                elm = elm.append({
                    let mut elm = Element::new("command");
                    if let Some(path) = output.path {
                        elm = elm.attr("location", path.display().to_string());
                    }

                    elm.cdata(output.command)
                });

                // Insert Resources
                if !output.resources.is_empty() {
                    elm = elm.append(output.resources.iter().map(|resource| {
                        Element::new("resource").text(resource.display().to_string())
                    }));
                }

                paws_domain::ToolOutput::text(elm)
            }
        }
    }
}
