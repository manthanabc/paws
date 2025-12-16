//! Tests for tool operation output formatting.

use std::fmt::Write;
use std::path::PathBuf;

use paws_domain::{Environment, FSRead, Metrics, ToolKind, ToolValue};

use crate::operation::*;
use crate::{
    Content, FsCreateOutput, FsRemoveOutput, FsUndoOutput, HttpResponse, Match, MatchResult,
    PatchOutput, ReadOutput, ResponseContext, SearchResult, ShellOutput, compute_hash,
};

fn fixture_environment() -> Environment {
    use fake::{Fake, Faker};
    let max_bytes: f64 = 250.0 * 1024.0; // 250 KB
    let fixture: Environment = Faker.fake();
    fixture
        .max_search_lines(25)
        .max_search_result_bytes(max_bytes.ceil() as usize)
        .fetch_truncation_limit(55)
        .max_read_size(10)
        .stdout_max_prefix_length(10)
        .stdout_max_suffix_length(10)
        .max_file_size(256 << 10) // 256 KiB
}

fn to_value(output: paws_domain::ToolOutput) -> String {
    let values = output.values;
    let mut result = String::new();
    values.into_iter().for_each(|value| match value {
        ToolValue::Text(txt) => {
            writeln!(result, "{}", txt).unwrap();
        }
        ToolValue::Image(image) => {
            writeln!(result, "Image with mime type: {}", image.mime_type()).unwrap();
        }
        ToolValue::Empty => {
            writeln!(result, "Empty value").unwrap();
        }
        ToolValue::AI { value, .. } => {
            writeln!(result, "{}", value).unwrap();
        }
    });

    result
}

#[test]
fn test_fs_read_basic() {
    let content = "Hello, world!\nThis is a test file.";
    let hash = crate::compute_hash(content);
    let fixture = ToolOperation::FsRead {
        input: FSRead {
            path: "/home/user/test.txt".to_string(),
            start_line: None,
            end_line: None,
            show_line_numbers: true,
        },
        output: ReadOutput {
            content: Content::file(content),
            start_line: 1,
            end_line: 2,
            total_lines: 2,
            content_hash: hash,
        },
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Read,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_read_basic_special_chars() {
    let content = "struct Foo<T>{ name: T }";
    let hash = crate::compute_hash(content);
    let fixture = ToolOperation::FsRead {
        input: FSRead {
            path: "/home/user/test.txt".to_string(),
            start_line: None,
            end_line: None,
            show_line_numbers: true,
        },
        output: ReadOutput {
            content: Content::file(content),
            start_line: 1,
            end_line: 1,
            total_lines: 1,
            content_hash: hash,
        },
    };

    let env = fixture_environment();
    let actual = fixture.into_tool_output(
        ToolKind::Read,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_read_with_explicit_range() {
    let content = "Line 1\nLine 2\nLine 3";
    let hash = crate::compute_hash(content);
    let fixture = ToolOperation::FsRead {
        input: FSRead {
            path: "/home/user/test.txt".to_string(),
            start_line: Some(2),
            end_line: Some(3),
            show_line_numbers: true,
        },
        output: ReadOutput {
            content: Content::file(content),
            start_line: 2,
            end_line: 3,
            total_lines: 5,
            content_hash: hash,
        },
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Read,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_read_with_truncation_path() {
    let content = "Truncated content";
    let hash = crate::compute_hash(content);
    let fixture = ToolOperation::FsRead {
        input: FSRead {
            path: "/home/user/large_file.txt".to_string(),
            start_line: None,
            end_line: None,
            show_line_numbers: true,
        },
        output: ReadOutput {
            content: Content::file(content),
            start_line: 1,
            end_line: 100,
            total_lines: 200,
            content_hash: hash,
        },
    };

    let env = fixture_environment();
    let truncation_path =
        TempContentFiles::default().stdout(PathBuf::from("/tmp/truncated_content.txt"));

    let actual = fixture.into_tool_output(
        ToolKind::Read,
        truncation_path,
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_create_basic() {
    let content = "Hello, world!";
    let fixture = ToolOperation::FsCreate {
        input: paws_domain::FSWrite {
            path: "/home/user/new_file.txt".to_string(),
            content: content.to_string(),
            overwrite: false,
        },
        output: FsCreateOutput {
            path: "/home/user/new_file.txt".to_string(),
            before: None,

            content_hash: compute_hash(content),
        },
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Write,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_create_overwrite() {
    let content = "New content for the file";
    let fixture = ToolOperation::FsCreate {
        input: paws_domain::FSWrite {
            path: "/home/user/existing_file.txt".to_string(),
            content: content.to_string(),
            overwrite: true,
        },
        output: FsCreateOutput {
            path: "/home/user/existing_file.txt".to_string(),
            before: Some("Old content".to_string()),

            content_hash: compute_hash(content),
        },
    };

    let env = fixture_environment();
    let actual = fixture.into_tool_output(
        ToolKind::Write,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_shell_output_no_truncation() {
    let fixture = ToolOperation::Shell {
        output: ShellOutput {
            output: paws_domain::CommandOutput {
                command: "echo hello".to_string(),
                stdout: "hello\nworld".to_string(),
                stderr: "".to_string(),
                exit_code: Some(0),
            },
            shell: "/bin/bash".to_string(),
        },
    };

    let env = fixture_environment();
    let actual = fixture.into_tool_output(
        ToolKind::Write,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_shell_output_stdout_truncation_only() {
    // Create stdout with more lines than the truncation limit
    let mut stdout_lines = Vec::new();
    for i in 1..=25 {
        stdout_lines.push(format!("stdout line {}", i));
    }
    let stdout = stdout_lines.join("\n");

    let fixture = ToolOperation::Shell {
        output: ShellOutput {
            output: paws_domain::CommandOutput {
                command: "long_command".to_string(),
                stdout,
                stderr: "".to_string(),
                exit_code: Some(0),
            },
            shell: "/bin/bash".to_string(),
        },
    };

    let env = fixture_environment();
    let truncation_path =
        TempContentFiles::default().stdout(PathBuf::from("/tmp/stdout_content.txt"));
    let actual = fixture.into_tool_output(
        ToolKind::Shell,
        truncation_path,
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_shell_output_stderr_truncation_only() {
    // Create stderr with more lines than the truncation limit
    let mut stderr_lines = Vec::new();
    for i in 1..=25 {
        stderr_lines.push(format!("stderr line {}", i));
    }
    let stderr = stderr_lines.join("\n");

    let fixture = ToolOperation::Shell {
        output: ShellOutput {
            output: paws_domain::CommandOutput {
                command: "error_command".to_string(),
                stdout: "".to_string(),
                stderr,
                exit_code: Some(1),
            },
            shell: "/bin/bash".to_string(),
        },
    };

    let env = fixture_environment();
    let truncation_path =
        TempContentFiles::default().stderr(PathBuf::from("/tmp/stderr_content.txt"));
    let actual = fixture.into_tool_output(
        ToolKind::Shell,
        truncation_path,
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_shell_output_both_stdout_stderr_truncation() {
    // Create both stdout and stderr with more lines than the truncation limit
    let mut stdout_lines = Vec::new();
    for i in 1..=25 {
        stdout_lines.push(format!("stdout line {}", i));
    }
    let stdout = stdout_lines.join("\n");

    let mut stderr_lines = Vec::new();
    for i in 1..=30 {
        stderr_lines.push(format!("stderr line {}", i));
    }
    let stderr = stderr_lines.join("\n");

    let fixture = ToolOperation::Shell {
        output: ShellOutput {
            output: paws_domain::CommandOutput {
                command: "complex_command".to_string(),
                stdout,
                stderr,
                exit_code: Some(0),
            },
            shell: "/bin/bash".to_string(),
        },
    };

    let env = fixture_environment();
    let truncation_path = TempContentFiles::default()
        .stdout(PathBuf::from("/tmp/stdout_content.txt"))
        .stderr(PathBuf::from("/tmp/stderr_content.txt"));
    let actual = fixture.into_tool_output(
        ToolKind::Shell,
        truncation_path,
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_shell_output_exact_boundary_stdout() {
    // Create stdout with exactly the truncation limit (prefix + suffix = 20 lines)
    let mut stdout_lines = Vec::new();
    for i in 1..=20 {
        stdout_lines.push(format!("stdout line {}", i));
    }
    let stdout = stdout_lines.join("\n");

    let fixture = ToolOperation::Shell {
        output: ShellOutput {
            output: paws_domain::CommandOutput {
                command: "boundary_command".to_string(),
                stdout,
                stderr: "".to_string(),
                exit_code: Some(0),
            },
            shell: "/bin/bash".to_string(),
        },
    };

    let env = fixture_environment();
    let actual = fixture.into_tool_output(
        ToolKind::Shell,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_shell_output_single_line_each() {
    let fixture = ToolOperation::Shell {
        output: ShellOutput {
            output: paws_domain::CommandOutput {
                command: "simple_command".to_string(),
                stdout: "single stdout line".to_string(),
                stderr: "single stderr line".to_string(),
                exit_code: Some(0),
            },
            shell: "/bin/bash".to_string(),
        },
    };

    let env = fixture_environment();
    let actual = fixture.into_tool_output(
        ToolKind::Shell,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_shell_output_empty_streams() {
    let fixture = ToolOperation::Shell {
        output: ShellOutput {
            output: paws_domain::CommandOutput {
                command: "silent_command".to_string(),
                stdout: "".to_string(),
                stderr: "".to_string(),
                exit_code: Some(0),
            },
            shell: "/bin/bash".to_string(),
        },
    };

    let env = fixture_environment();
    let actual = fixture.into_tool_output(
        ToolKind::Shell,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_shell_output_line_number_calculation() {
    // Test specific line number calculations for 1-based indexing
    let mut stdout_lines = Vec::new();
    for i in 1..=15 {
        stdout_lines.push(format!("stdout {}", i));
    }
    let stdout = stdout_lines.join("\n");

    let mut stderr_lines = Vec::new();
    for i in 1..=12 {
        stderr_lines.push(format!("stderr {}", i));
    }
    let stderr = stderr_lines.join("\n");

    let fixture = ToolOperation::Shell {
        output: ShellOutput {
            output: paws_domain::CommandOutput {
                command: "line_test_command".to_string(),
                stdout,
                stderr,
                exit_code: Some(0),
            },
            shell: "/bin/bash".to_string(),
        },
    };

    let env = fixture_environment();
    let truncation_path = TempContentFiles::default()
        .stdout(PathBuf::from("/tmp/stdout_content.txt"))
        .stderr(PathBuf::from("/tmp/stderr_content.txt"));
    let actual = fixture.into_tool_output(
        ToolKind::Shell,
        truncation_path,
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_search_output() {
    // Create a large number of search matches to trigger truncation
    let mut matches = Vec::new();
    let total_lines = 50;
    for i in 1..=total_lines {
        matches.push(Match {
            path: "/home/user/project/foo.txt".to_string(),
            result: Some(MatchResult::Found {
                line: format!("Match line {}: Test", i),
                line_number: i,
            }),
        });
    }

    let fixture = ToolOperation::FsSearch {
        input: paws_domain::FSSearch {
            path: "/home/user/project".to_string(),
            regex: Some("search".to_string()),
            start_index: Some(6),
            max_search_lines: Some(30), // This will be limited by env.max_search_lines (25)
            file_pattern: Some("*.txt".to_string()),
        },
        output: Some(SearchResult { matches }),
    };

    let env = fixture_environment(); // max_search_lines is 25

    let actual = fixture.into_tool_output(
        ToolKind::Search,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_search_max_output() {
    // Create a large number of search matches to trigger truncation
    let mut matches = Vec::new();
    let total_lines = 50; // Total lines found.
    for i in 1..=total_lines {
        matches.push(Match {
            path: "/home/user/project/foo.txt".to_string(),
            result: Some(MatchResult::Found {
                line: format!("Match line {}: Test", i),
                line_number: i,
            }),
        });
    }

    let fixture = ToolOperation::FsSearch {
        input: paws_domain::FSSearch {
            path: "/home/user/project".to_string(),
            regex: Some("search".to_string()),
            start_index: Some(6),
            max_search_lines: Some(30), // This will be limited by env.max_search_lines (25)
            file_pattern: Some("*.txt".to_string()),
        },
        output: Some(SearchResult { matches }),
    };

    let mut env = fixture_environment();
    // Total lines found are 50, but we limit to 10 for this test
    env.max_search_lines = 10;

    let actual = fixture.into_tool_output(
        ToolKind::Search,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_search_min_lines_but_max_line_length() {
    // Create a large number of search matches to trigger truncation
    let mut matches = Vec::new();
    let total_lines = 50; // Total lines found.
    for i in 1..=total_lines {
        matches.push(Match {
            path: "/home/user/project/foo.txt".to_string(),
            result: Some(MatchResult::Found {
                line: format!("Match line {}: {}", i, "AB".repeat(50)),
                line_number: i,
            }),
        });
    }

    let fixture = ToolOperation::FsSearch {
        input: paws_domain::FSSearch {
            path: "/home/user/project".to_string(),
            regex: Some("search".to_string()),
            start_index: Some(6),
            max_search_lines: Some(30), // This will be limited by env.max_search_lines (20)
            file_pattern: Some("*.txt".to_string()),
        },
        output: Some(SearchResult { matches }),
    };

    let mut env = fixture_environment();
    // Total lines found are 50, but we limit to 20 for this test
    env.max_search_lines = 20;
    let max_bytes: f64 = 0.001 * 1024.0 * 1024.0;
    env.max_search_result_bytes = max_bytes.ceil() as usize; // limit to 0.001 MB

    let actual = fixture.into_tool_output(
        ToolKind::Search,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_search_very_lengthy_one_line_match() {
    let mut matches = Vec::new();
    let total_lines = 1; // Total lines found.
    for i in 1..=total_lines {
        matches.push(Match {
            path: "/home/user/project/foo.txt".to_string(),
            result: Some(MatchResult::Found {
                line: format!(
                    "Match line {}: {}",
                    i,
                    "abcdefghijklmnopqrstuvwxyz".repeat(40)
                ),
                line_number: i,
            }),
        });
    }

    let fixture = ToolOperation::FsSearch {
        input: paws_domain::FSSearch {
            path: "/home/user/project".to_string(),
            regex: Some("search".to_string()),
            start_index: Some(6),
            max_search_lines: Some(30), // This will be limited by env.max_search_lines (20)
            file_pattern: Some("*.txt".to_string()),
        },
        output: Some(SearchResult { matches }),
    };

    let mut env = fixture_environment();
    // Total lines found are 50, but we limit to 20 for this test
    env.max_search_lines = 20;
    let max_bytes: f64 = 0.001 * 1024.0 * 1024.0;
    env.max_search_result_bytes = max_bytes.ceil() as usize; // limit to 0.001 MB

    let actual = fixture.into_tool_output(
        ToolKind::Search,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_search_no_matches() {
    let fixture = ToolOperation::FsSearch {
        input: paws_domain::FSSearch {
            path: "/home/user/empty_project".to_string(),
            regex: Some("nonexistent".to_string()),
            start_index: None,
            max_search_lines: None,
            file_pattern: None,
        },
        output: None,
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Search,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_create_with_warning() {
    let content = "Content with warning";
    let fixture = ToolOperation::FsCreate {
        input: paws_domain::FSWrite {
            path: "/home/user/file_with_warning.txt".to_string(),
            content: content.to_string(),
            overwrite: false,
        },
        output: FsCreateOutput {
            path: "/home/user/file_with_warning.txt".to_string(),
            before: None,

            content_hash: compute_hash(content),
        },
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Write,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_remove_success() {
    let fixture = ToolOperation::FsRemove {
        input: paws_domain::FSRemove { path: "/home/user/file_to_delete.txt".to_string() },
        output: FsRemoveOutput { content: "content".to_string() },
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Remove,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_search_with_results() {
    let fixture = ToolOperation::FsSearch {
        input: paws_domain::FSSearch {
            path: "/home/user/project".to_string(),
            regex: Some("Hello".to_string()),
            start_index: None,
            max_search_lines: None,
            file_pattern: Some("*.txt".to_string()),
        },
        output: Some(SearchResult {
            matches: vec![
                Match {
                    path: "file1.txt".to_string(),
                    result: Some(MatchResult::Found {
                        line_number: 1,
                        line: "Hello world".to_string(),
                    }),
                },
                Match {
                    path: "file2.txt".to_string(),
                    result: Some(MatchResult::Found {
                        line_number: 3,
                        line: "Hello universe".to_string(),
                    }),
                },
            ],
        }),
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Search,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_search_no_results() {
    let fixture = ToolOperation::FsSearch {
        input: paws_domain::FSSearch {
            path: "/home/user/project".to_string(),
            regex: Some("NonExistentPattern".to_string()),
            start_index: None,
            max_search_lines: None,
            file_pattern: None,
        },
        output: None,
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Search,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_patch_basic() {
    let after_content = "Hello universe\nThis is a test";
    let fixture = ToolOperation::FsPatch {
        input: paws_domain::FSPatch {
            path: "/home/user/test.txt".to_string(),
            search: Some("world".to_string()),
            operation: paws_domain::PatchOperation::Replace,
            content: "universe".to_string(),
        },
        output: PatchOutput {
            before: "Hello world\nThis is a test".to_string(),
            after: after_content.to_string(),
            content_hash: compute_hash(after_content),
        },
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Patch,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_patch_with_warning() {
    let after_content = "line1\nnew line\nline2";
    let fixture = ToolOperation::FsPatch {
        input: paws_domain::FSPatch {
            path: "/home/user/large_file.txt".to_string(),
            search: Some("line1".to_string()),
            operation: paws_domain::PatchOperation::Append,
            content: "\nnew line".to_string(),
        },
        output: PatchOutput {
            before: "line1\nline2".to_string(),
            after: after_content.to_string(),
            content_hash: compute_hash(after_content),
        },
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Patch,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_undo_no_changes() {
    let fixture = ToolOperation::FsUndo {
        input: paws_domain::FSUndo { path: "/home/user/unchanged_file.txt".to_string() },
        output: FsUndoOutput { before_undo: None, after_undo: None },
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Undo,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_undo_file_created() {
    let fixture = ToolOperation::FsUndo {
        input: paws_domain::FSUndo { path: "/home/user/new_file.txt".to_string() },
        output: FsUndoOutput {
            before_undo: None,
            after_undo: Some("New file content\nLine 2\nLine 3".to_string()),
        },
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Undo,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_undo_file_removed() {
    let fixture = ToolOperation::FsUndo {
        input: paws_domain::FSUndo { path: "/home/user/deleted_file.txt".to_string() },
        output: FsUndoOutput {
            before_undo: Some("Original file content\nThat was deleted\nDuring undo".to_string()),
            after_undo: None,
        },
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Undo,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_undo_file_restored() {
    let fixture = ToolOperation::FsUndo {
        input: paws_domain::FSUndo { path: "/home/user/restored_file.txt".to_string() },
        output: FsUndoOutput {
            before_undo: Some("Original content\nBefore changes".to_string()),
            after_undo: Some("Modified content\nAfter restoration".to_string()),
        },
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Undo,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_fs_undo_success() {
    let fixture = ToolOperation::FsUndo {
        input: paws_domain::FSUndo { path: "/home/user/test.txt".to_string() },
        output: FsUndoOutput {
            before_undo: Some("ABC".to_string()),
            after_undo: Some("PQR".to_string()),
        },
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Undo,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_net_fetch_success() {
    let fixture = ToolOperation::NetFetch {
        input: paws_domain::NetFetch { url: "https://example.com".to_string(), raw: Some(false) },
        output: HttpResponse {
            content: "# Example Website\n\nThis is some content from a website.".to_string(),
            code: 200,
            context: ResponseContext::Raw,
            content_type: "text/plain".to_string(),
        },
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Fetch,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_net_fetch_truncated() {
    let env = fixture_environment();
    let truncated_content = "Truncated Content".to_string();
    let long_content = format!(
        "{}{}",
        "A".repeat(env.fetch_truncation_limit),
        &truncated_content
    );
    let fixture = ToolOperation::NetFetch {
        input: paws_domain::NetFetch {
            url: "https://example.com/large-page".to_string(),
            raw: Some(false),
        },
        output: HttpResponse {
            content: long_content,
            code: 200,
            context: ResponseContext::Parsed,
            content_type: "text/html".to_string(),
        },
    };

    let truncation_path =
        TempContentFiles::default().stdout(PathBuf::from("/tmp/paws_fetch_abc123.txt"));

    let actual = fixture.into_tool_output(
        ToolKind::Fetch,
        truncation_path,
        &env,
        &mut Metrics::default(),
    );

    // make sure that the content is truncated
    assert!(
        !actual
            .values
            .first()
            .unwrap()
            .as_str()
            .unwrap()
            .ends_with(&truncated_content)
    );
    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_shell_success() {
    let fixture = ToolOperation::Shell {
        output: ShellOutput {
            output: paws_domain::CommandOutput {
                command: "ls -la".to_string(),
                stdout: "total 8\ndrwxr-xr-x  2 user user 4096 Jan  1 12:00 .\ndrwxr-xr-x 10 user user 4096 Jan  1 12:00 ..".to_string(),
                stderr: "".to_string(),
                exit_code: Some(0),
            },
            shell: "/bin/bash".to_string(),
        },
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Shell,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_follow_up_with_question() {
    let fixture = ToolOperation::FollowUp {
        output: Some("Which file would you like to edit?".to_string()),
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Followup,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_follow_up_no_question() {
    let fixture = ToolOperation::FollowUp { output: None };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Followup,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}

#[test]
fn test_skill_operation() {
    let fixture = ToolOperation::Skill {
        input: paws_domain::SkillFetch { name: "test-skill".to_string() },
        output: paws_domain::Skill::new(
            "test-skill",
            "This is a test skill command with instructions",
            "A test skill for demonstration",
        )
        .path("/home/user/.paws/skills/test-skill")
        .resources(vec![
            PathBuf::from("/home/user/.paws/skills/test-skill/resource1.txt"),
            PathBuf::from("/home/user/.paws/skills/test-skill/resource2.md"),
        ]),
    };

    let env = fixture_environment();

    let actual = fixture.into_tool_output(
        ToolKind::Skill,
        TempContentFiles::default(),
        &env,
        &mut Metrics::default(),
    );

    insta::assert_snapshot!(to_value(actual));
}
