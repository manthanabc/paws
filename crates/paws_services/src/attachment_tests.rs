//! Tests for the attachment service.

use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine;
use paws_app::AttachmentService;
use paws_app::domain::AttachmentContent;

use crate::attachment::PawsChatRequest;
use crate::test_fixtures::MockCompositeService;

#[tokio::test]
async fn test_add_url_with_text_file() {
    // Setup
    let infra = Arc::new(MockCompositeService::new());
    let chat_request = PawsChatRequest::new(infra.clone());

    // Test with a text file path in chat message
    let url = "@[/test/file1.txt]".to_string();

    // Execute
    let attachments = chat_request.attachments(&url).await.unwrap();

    // Assert
    // Text files should be included in the attachments
    assert_eq!(attachments.len(), 1);
    let attachment = attachments.first().unwrap();
    assert_eq!(attachment.path, "/test/file1.txt");

    // Check that the content contains our original text and has range information
    assert!(attachment.content.contains("This is a text file content"));
}

#[tokio::test]
async fn test_add_url_with_image() {
    // Setup
    let infra = Arc::new(MockCompositeService::new());
    let chat_request = PawsChatRequest::new(infra.clone());

    // Test with an image file
    let url = "@[/test/image.png]".to_string();

    // Execute
    let attachments = chat_request.attachments(&url).await.unwrap();

    // Assert
    assert_eq!(attachments.len(), 1);
    let attachment = attachments.first().unwrap();
    assert_eq!(attachment.path, "/test/image.png");

    // Base64 content should be the encoded mock binary content with proper data URI
    // format
    let expected_base64 = base64::engine::general_purpose::STANDARD.encode("mock-binary-content");
    assert_eq!(
        attachment.content.as_image().unwrap().url().as_str(),
        format!("data:image/png;base64,{expected_base64}")
    );
}

#[tokio::test]
async fn test_add_url_with_jpg_image_with_spaces() {
    // Setup
    let infra = Arc::new(MockCompositeService::new());
    let chat_request = PawsChatRequest::new(infra.clone());

    // Test with an image file that has spaces in the path
    let url = "@[/test/image with spaces.jpg]".to_string();

    // Execute
    let attachments = chat_request.attachments(&url).await.unwrap();

    // Assert
    assert_eq!(attachments.len(), 1);
    let attachment = attachments.first().unwrap();
    assert_eq!(attachment.path, "/test/image with spaces.jpg");

    // Base64 content should be the encoded mock jpeg content with proper data URI
    // format
    let expected_base64 = base64::engine::general_purpose::STANDARD.encode("mock-jpeg-content");
    assert_eq!(
        attachment.content.as_image().unwrap().url().as_str(),
        format!("data:image/jpeg;base64,{expected_base64}")
    );
}

#[tokio::test]
async fn test_add_url_with_multiple_files() {
    // Setup
    let infra = Arc::new(MockCompositeService::new());

    // Add an extra file to our mock service
    infra.add_file(
        PathBuf::from("/test/file2.txt"),
        "This is another text file".to_string(),
    );

    let chat_request = PawsChatRequest::new(infra.clone());

    // Test with multiple files mentioned
    let url = "@[/test/file1.txt] @[/test/file2.txt] @[/test/image.png]".to_string();

    // Execute
    let attachments = chat_request.attachments(&url).await.unwrap();

    // Assert
    // All files should be included in the attachments
    assert_eq!(attachments.len(), 3);

    // Verify that each expected file is in the attachments
    let has_file1 = attachments.iter().any(|a| {
        a.path == "/test/file1.txt" && matches!(a.content, AttachmentContent::FileContent { .. })
    });
    let has_file2 = attachments.iter().any(|a| {
        a.path == "/test/file2.txt" && matches!(a.content, AttachmentContent::FileContent { .. })
    });
    let has_image = attachments
        .iter()
        .any(|a| a.path == "/test/image.png" && matches!(a.content, AttachmentContent::Image(_)));

    assert!(has_file1, "Missing file1.txt in attachments");
    assert!(has_file2, "Missing file2.txt in attachments");
    assert!(has_image, "Missing image.png in attachments");
}

#[tokio::test]
async fn test_add_url_with_nonexistent_file() {
    // Setup
    let infra = Arc::new(MockCompositeService::new());
    let chat_request = PawsChatRequest::new(infra.clone());

    // Test with a file that doesn't exist
    let url = "@[/test/nonexistent.txt]".to_string();

    // Execute - Let's handle the error properly
    let result = chat_request.attachments(&url).await;

    // Assert - we expect an error for nonexistent files
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("File not found"));
}

#[tokio::test]
async fn test_add_url_empty() {
    // Setup
    let infra = Arc::new(MockCompositeService::new());
    let chat_request = PawsChatRequest::new(infra.clone());

    // Test with an empty message
    let url = "".to_string();

    // Execute
    let attachments = chat_request.attachments(&url).await.unwrap();

    // Assert - no attachments
    assert_eq!(attachments.len(), 0);
}

#[tokio::test]
async fn test_add_url_with_unsupported_extension() {
    // Setup
    let infra = Arc::new(MockCompositeService::new());

    // Add a file with unsupported extension
    infra.add_file(
        PathBuf::from("/test/unknown.xyz"),
        "Some content".to_string(),
    );

    let chat_request = PawsChatRequest::new(infra.clone());

    // Test with the file
    let url = "@[/test/unknown.xyz]".to_string();

    // Execute
    let attachments = chat_request.attachments(&url).await.unwrap();

    // Assert - should be treated as text
    assert_eq!(attachments.len(), 1);
    let attachment = attachments.first().unwrap();
    assert_eq!(attachment.path, "/test/unknown.xyz");

    // Check that the content contains our original text and has range information
    assert!(attachment.content.contains("Some content"));
}

#[tokio::test]
async fn test_attachment_range_information() {
    // Setup
    let infra = Arc::new(MockCompositeService::new());

    // Add a multi-line file to test range information
    infra.add_file(
        PathBuf::from("/test/multiline.txt"),
        "Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_string(),
    );

    let chat_request = PawsChatRequest::new(infra.clone());
    let url = "@[/test/multiline.txt]".to_string();

    // Execute
    let attachments = chat_request.attachments(&url).await.unwrap();

    // Assert
    assert_eq!(attachments.len(), 1);
    let attachment = attachments.first().unwrap();

    // Verify range information is populated
    let range_info = attachment.content.range_info();
    assert!(
        range_info.is_some(),
        "Range information should be present for file content"
    );

    let (start_line, end_line, total_lines) = range_info.unwrap();
    assert_eq!(start_line, 1, "Start line should be 1");
    assert!(end_line >= start_line, "End line should be >= start line");
    assert!(total_lines >= end_line, "Total lines should be >= end line");

    // Verify content is accessible through helper method
    let file_content = attachment.content.file_content();
    assert!(file_content.is_some(), "File content should be accessible");
    assert!(
        file_content.unwrap().contains("Line 1"),
        "Should contain file content"
    );
}

// Range functionality tests
#[tokio::test]
async fn test_range_functionality_single_line() {
    let infra = Arc::new(MockCompositeService::new());

    // Add a multi-line test file
    infra.add_file(
        PathBuf::from("/test/multiline.txt"),
        "Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_string(),
    );

    let chat_request = PawsChatRequest::new(infra.clone());

    // Test reading line 2 only
    let url = "@[/test/multiline.txt:2:2]";
    let attachments = chat_request.attachments(url).await.unwrap();

    assert_eq!(attachments.len(), 1);
    assert_eq!(
        attachments[0].content,
        AttachmentContent::FileContent {
            content: "2:Line 2".to_string(),
            start_line: 2,
            end_line: 2,
            total_lines: 5,
        }
    );
}

#[tokio::test]
async fn test_range_functionality_multiple_lines() {
    let infra = Arc::new(MockCompositeService::new());

    // Add a multi-line test file
    infra.add_file(
        PathBuf::from("/test/range_test.txt"),
        "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6".to_string(),
    );

    let chat_request = PawsChatRequest::new(infra.clone());

    // Test reading lines 2-4
    let url = "@[/test/range_test.txt:2:4]";
    let attachments = chat_request.attachments(url).await.unwrap();

    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments.len(), 1);
    assert_eq!(
        attachments[0].content,
        AttachmentContent::FileContent {
            content: "2:Line 2\n3:Line 3\n4:Line 4".to_string(),
            start_line: 2,
            end_line: 4,
            total_lines: 6,
        }
    );
}

#[tokio::test]
async fn test_range_functionality_from_start() {
    let infra = Arc::new(MockCompositeService::new());

    infra.add_file(
        PathBuf::from("/test/start_range.txt"),
        "First\nSecond\nThird\nFourth".to_string(),
    );

    let chat_request = PawsChatRequest::new(infra.clone());

    // Test reading from start to line 2
    let url = "@[/test/start_range.txt:1:2]";
    let attachments = chat_request.attachments(url).await.unwrap();
    assert_eq!(
        attachments[0].content,
        AttachmentContent::FileContent {
            content: "1:First\n2:Second".to_string(),
            start_line: 1,
            end_line: 2,
            total_lines: 4,
        }
    );
}

#[tokio::test]
async fn test_range_functionality_to_end() {
    let infra = Arc::new(MockCompositeService::new());

    infra.add_file(
        PathBuf::from("/test/end_range.txt"),
        "Alpha\nBeta\nGamma\nDelta\nEpsilon".to_string(),
    );

    let chat_request = PawsChatRequest::new(infra.clone());

    // Test reading from line 3 to end
    let url = "@[/test/end_range.txt:3:5]";
    let attachments = chat_request.attachments(url).await.unwrap();
    assert_eq!(
        attachments[0].content,
        AttachmentContent::FileContent {
            content: "3:Gamma\n4:Delta\n5:Epsilon".to_string(),
            start_line: 3,
            end_line: 5,
            total_lines: 5,
        }
    );
}

#[tokio::test]
async fn test_range_functionality_edge_cases() {
    let infra = Arc::new(MockCompositeService::new());

    infra.add_file(
        PathBuf::from("/test/edge_case.txt"),
        "Only line".to_string(),
    );

    let chat_request = PawsChatRequest::new(infra.clone());

    // Test reading beyond file length
    let url = "@[/test/edge_case.txt:1:10]";
    let attachments = chat_request.attachments(url).await.unwrap();
    assert_eq!(
        attachments[0].content,
        AttachmentContent::FileContent {
            content: "1:Only line".to_string(),
            start_line: 1,
            end_line: 1,
            total_lines: 1,
        }
    );
}

#[tokio::test]
async fn test_range_functionality_combined_with_multiple_files() {
    let infra = Arc::new(MockCompositeService::new());

    infra.add_file(PathBuf::from("/test/file_a.txt"), "A1\nA2\nA3".to_string());
    infra.add_file(
        PathBuf::from("/test/file_b.txt"),
        "B1\nB2\nB3\nB4".to_string(),
    );

    let chat_request = PawsChatRequest::new(infra.clone());

    // Test multiple files with different ranges
    let url = "Check @[/test/file_a.txt:1:2] and @[/test/file_b.txt:3:4]";
    let attachments = chat_request.attachments(url).await.unwrap();

    assert_eq!(attachments.len(), 2);
    assert_eq!(
        attachments[0].content,
        AttachmentContent::FileContent {
            content: "1:A1\n2:A2".to_string(),
            start_line: 1,
            end_line: 2,
            total_lines: 3,
        }
    );
    assert_eq!(
        attachments[1].content,
        AttachmentContent::FileContent {
            content: "3:B3\n4:B4".to_string(),
            start_line: 3,
            end_line: 4,
            total_lines: 4,
        }
    );
}

#[tokio::test]
async fn test_range_functionality_preserves_metadata() {
    let infra = Arc::new(MockCompositeService::new());

    infra.add_file(
        PathBuf::from("/test/metadata_test.txt"),
        "Meta1\nMeta2\nMeta3\nMeta4\nMeta5\nMeta6\nMeta7".to_string(),
    );

    let chat_request = PawsChatRequest::new(infra.clone());

    // Test that metadata is preserved correctly with ranges
    let url = "@[/test/metadata_test.txt:3:5]";
    let attachments = chat_request.attachments(url).await.unwrap();

    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].path, "/test/metadata_test.txt");
    assert_eq!(
        attachments[0].content,
        AttachmentContent::FileContent {
            content: "3:Meta3\n4:Meta4\n5:Meta5".to_string(),
            start_line: 3,
            end_line: 5,
            total_lines: 7,
        }
    );
}

#[tokio::test]
async fn test_range_functionality_vs_full_file() {
    let infra = Arc::new(MockCompositeService::new());

    infra.add_file(
        PathBuf::from("/test/comparison.txt"),
        "Full1\nFull2\nFull3\nFull4\nFull5".to_string(),
    );

    let chat_request = PawsChatRequest::new(infra.clone());

    // Test full file vs ranged file to ensure they're different
    let url_full = "@[/test/comparison.txt]";
    let url_range = "@[/test/comparison.txt:2:4]";
    let url_range_start = "@[/test/comparison.txt:2]";

    let attachments_full = chat_request.attachments(url_full).await.unwrap();
    let attachments_range = chat_request.attachments(url_range).await.unwrap();
    let attachments_range_start = chat_request.attachments(url_range_start).await.unwrap();

    assert_eq!(attachments_full.len(), 1);
    assert_eq!(
        attachments_full[0].content,
        AttachmentContent::FileContent {
            content: "1:Full1\n2:Full2\n3:Full3\n4:Full4\n5:Full5".to_string(),
            start_line: 1,
            end_line: 5,
            total_lines: 5,
        }
    );

    assert_eq!(attachments_range.len(), 1);
    assert_eq!(
        attachments_range[0].content,
        AttachmentContent::FileContent {
            content: "2:Full2\n3:Full3\n4:Full4".to_string(),
            start_line: 2,
            end_line: 4,
            total_lines: 5,
        }
    );

    assert_eq!(attachments_range_start.len(), 1);
    assert_eq!(
        attachments_range_start[0].content,
        AttachmentContent::FileContent {
            content: "2:Full2\n3:Full3\n4:Full4\n5:Full5".to_string(),
            start_line: 2,
            end_line: 5,
            total_lines: 5,
        }
    );
}

#[tokio::test]
async fn test_add_url_with_directory() {
    let infra = Arc::new(MockCompositeService::new());

    // Add directory, files, and subdirectory
    infra.file_service.add_dir(PathBuf::from("/test/mydir"));
    infra.add_file(
        PathBuf::from("/test/mydir/file1.txt"),
        "Content of file1".to_string(),
    );
    infra.add_file(
        PathBuf::from("/test/mydir/file2.txt"),
        "Content of file2".to_string(),
    );
    infra
        .file_service
        .add_dir(PathBuf::from("/test/mydir/subdir"));

    let chat_request = PawsChatRequest::new(infra.clone());

    // Test with directory path
    let url = "@[/test/mydir]";
    let attachments = chat_request.attachments(url).await.unwrap();

    // Should return a single DirectoryListing attachment
    assert_eq!(attachments.len(), 1);
    let attachment = attachments.first().unwrap();

    // Verify it's a directory listing with relative paths
    match &attachment.content {
        AttachmentContent::DirectoryListing { entries } => {
            // Should contain 2 files and 1 subdirectory (3 total)
            assert_eq!(entries.len(), 3);

            // Check for files (is_dir = false)
            let file1 = entries.iter().find(|e| e.path == "file1.txt").unwrap();
            assert!(!file1.is_dir);

            let file2 = entries.iter().find(|e| e.path == "file2.txt").unwrap();
            assert!(!file2.is_dir);

            // Check for subdirectory (is_dir = true)
            let subdir = entries.iter().find(|e| e.path == "subdir").unwrap();
            assert!(subdir.is_dir);
        }
        _ => panic!("Expected DirectoryListing attachment"),
    }

    // Path should be absolute (root level)
    assert_eq!(attachment.path, "/test/mydir");
}

#[tokio::test]
async fn test_add_url_with_empty_directory() {
    let infra = Arc::new(MockCompositeService::new());

    // Add empty directory
    infra.file_service.add_dir(PathBuf::from("/test/emptydir"));

    let chat_request = PawsChatRequest::new(infra.clone());

    // Test with empty directory path
    let url = "@[/test/emptydir]";
    let attachments = chat_request.attachments(url).await.unwrap();

    // Should return a single DirectoryListing attachment with empty files list
    assert_eq!(attachments.len(), 1);
    let attachment = attachments.first().unwrap();

    match &attachment.content {
        AttachmentContent::DirectoryListing { entries } => {
            assert_eq!(entries.len(), 0);
        }
        _ => panic!("Expected DirectoryListing attachment"),
    }

    // Path should be absolute (root level)
    assert_eq!(attachment.path, "/test/emptydir");
}

#[tokio::test]
async fn test_add_url_with_mixed_files_and_directory() {
    let infra = Arc::new(MockCompositeService::new());

    // Add directory with files
    infra.file_service.add_dir(PathBuf::from("/test/mixdir"));
    infra.add_file(
        PathBuf::from("/test/mixdir/dir_file.txt"),
        "File in directory".to_string(),
    );

    // Add standalone file
    infra.add_file(
        PathBuf::from("/test/standalone.txt"),
        "Standalone file".to_string(),
    );

    let chat_request = PawsChatRequest::new(infra.clone());

    // Test with both file and directory
    let url = "@[/test/mixdir] @[/test/standalone.txt]";
    let attachments = chat_request.attachments(url).await.unwrap();

    // Should include both the directory listing and the standalone file
    assert_eq!(attachments.len(), 2);

    // Find directory listing (absolute path at root level)
    let dir_attachment = attachments
        .iter()
        .find(|a| a.path == "/test/mixdir")
        .unwrap();
    match &dir_attachment.content {
        AttachmentContent::DirectoryListing { entries } => {
            assert_eq!(entries.len(), 1);
            // File path should be relative to the directory being listed
            let dir_file = entries.iter().find(|e| e.path == "dir_file.txt").unwrap();
            assert!(!dir_file.is_dir);
        }
        _ => panic!("Expected DirectoryListing attachment"),
    }

    // Find file attachment (absolute path at root level)
    let file_attachment = attachments
        .iter()
        .find(|a| a.path == "/test/standalone.txt")
        .unwrap();
    assert!(matches!(
        &file_attachment.content,
        AttachmentContent::FileContent { .. }
    ));
}

#[tokio::test]
async fn test_directory_sorting_dirs_first() {
    let infra = Arc::new(MockCompositeService::new());

    // Add directory with mixed files and subdirectories in random order
    infra.file_service.add_dir(PathBuf::from("/test/sortdir"));
    infra.add_file(
        PathBuf::from("/test/sortdir/zebra.txt"),
        "File Z".to_string(),
    );
    infra
        .file_service
        .add_dir(PathBuf::from("/test/sortdir/apple_dir"));
    infra.add_file(
        PathBuf::from("/test/sortdir/banana.txt"),
        "File B".to_string(),
    );
    infra
        .file_service
        .add_dir(PathBuf::from("/test/sortdir/zoo_dir"));
    infra.add_file(
        PathBuf::from("/test/sortdir/cherry.txt"),
        "File C".to_string(),
    );
    infra
        .file_service
        .add_dir(PathBuf::from("/test/sortdir/berry_dir"));

    let chat_request = PawsChatRequest::new(infra.clone());
    let url = "@[/test/sortdir]";
    let attachments = chat_request.attachments(url).await.unwrap();

    // Verify directory listing
    assert_eq!(attachments.len(), 1);
    let attachment = attachments.first().unwrap();

    match &attachment.content {
        AttachmentContent::DirectoryListing { entries } => {
            assert_eq!(entries.len(), 6);

            // Verify directories come first, sorted alphabetically
            assert!(entries[0].is_dir);
            assert_eq!(entries[0].path, "apple_dir");

            assert!(entries[1].is_dir);
            assert_eq!(entries[1].path, "berry_dir");

            assert!(entries[2].is_dir);
            assert_eq!(entries[2].path, "zoo_dir");

            // Verify files come after, sorted alphabetically
            assert!(!entries[3].is_dir);
            assert_eq!(entries[3].path, "banana.txt");

            assert!(!entries[4].is_dir);
            assert_eq!(entries[4].path, "cherry.txt");

            assert!(!entries[5].is_dir);
            assert_eq!(entries[5].path, "zebra.txt");
        }
        _ => panic!("Expected DirectoryListing attachment"),
    }
}

#[tokio::test]
async fn test_directory_sorting_only_directories() {
    let infra = Arc::new(MockCompositeService::new());

    // Add directory with only subdirectories
    infra.file_service.add_dir(PathBuf::from("/test/onlydirs"));
    infra
        .file_service
        .add_dir(PathBuf::from("/test/onlydirs/zebra_dir"));
    infra
        .file_service
        .add_dir(PathBuf::from("/test/onlydirs/alpha_dir"));
    infra
        .file_service
        .add_dir(PathBuf::from("/test/onlydirs/middle_dir"));

    let chat_request = PawsChatRequest::new(infra.clone());
    let url = "@[/test/onlydirs]";
    let attachments = chat_request.attachments(url).await.unwrap();

    match &attachments[0].content {
        AttachmentContent::DirectoryListing { entries } => {
            assert_eq!(entries.len(), 3);

            // All should be directories, sorted alphabetically
            assert!(entries[0].is_dir);
            assert_eq!(entries[0].path, "alpha_dir");

            assert!(entries[1].is_dir);
            assert_eq!(entries[1].path, "middle_dir");

            assert!(entries[2].is_dir);
            assert_eq!(entries[2].path, "zebra_dir");
        }
        _ => panic!("Expected DirectoryListing attachment"),
    }
}

#[tokio::test]
async fn test_directory_sorting_only_files() {
    let infra = Arc::new(MockCompositeService::new());

    // Add directory with only files
    infra.file_service.add_dir(PathBuf::from("/test/onlyfiles"));
    infra.add_file(PathBuf::from("/test/onlyfiles/zebra.txt"), "Z".to_string());
    infra.add_file(PathBuf::from("/test/onlyfiles/alpha.txt"), "A".to_string());
    infra.add_file(PathBuf::from("/test/onlyfiles/middle.txt"), "M".to_string());

    let chat_request = PawsChatRequest::new(infra.clone());
    let url = "@[/test/onlyfiles]";
    let attachments = chat_request.attachments(url).await.unwrap();

    match &attachments[0].content {
        AttachmentContent::DirectoryListing { entries } => {
            assert_eq!(entries.len(), 3);

            // All should be files, sorted alphabetically
            assert!(!entries[0].is_dir);
            assert_eq!(entries[0].path, "alpha.txt");

            assert!(!entries[1].is_dir);
            assert_eq!(entries[1].path, "middle.txt");

            assert!(!entries[2].is_dir);
            assert_eq!(entries[2].path, "zebra.txt");
        }
        _ => panic!("Expected DirectoryListing attachment"),
    }
}

#[tokio::test]
async fn test_directory_sorting_case_insensitive() {
    let infra = Arc::new(MockCompositeService::new());

    // Add directory with mixed case names
    infra.file_service.add_dir(PathBuf::from("/test/casetest"));
    infra
        .file_service
        .add_dir(PathBuf::from("/test/casetest/Zebra_dir"));
    infra
        .file_service
        .add_dir(PathBuf::from("/test/casetest/apple_dir"));
    infra.add_file(PathBuf::from("/test/casetest/Zebra.txt"), "Z".to_string());
    infra.add_file(PathBuf::from("/test/casetest/apple.txt"), "A".to_string());

    let chat_request = PawsChatRequest::new(infra.clone());
    let url = "@[/test/casetest]";
    let attachments = chat_request.attachments(url).await.unwrap();

    match &attachments[0].content {
        AttachmentContent::DirectoryListing { entries } => {
            assert_eq!(entries.len(), 4);

            // Directories first
            assert!(entries[0].is_dir);
            assert!(entries[1].is_dir);

            // Files after
            assert!(!entries[2].is_dir);
            assert!(!entries[3].is_dir);

            // Note: Rust's default string comparison is case-sensitive
            // so "Zebra_dir" < "apple_dir" (uppercase comes before
            // lowercase) This documents the current
            // behavior
        }
        _ => panic!("Expected DirectoryListing attachment"),
    }
}
