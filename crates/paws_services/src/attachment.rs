use std::sync::Arc;

use paws_app::domain::{
    Attachment, AttachmentContent, DirectoryEntry, FileTag, Image, LineNumbers,
};
use paws_app::utils::format_display_path;
use paws_app::{
    AttachmentService, DirectoryReaderInfra, EnvironmentInfra, FileInfoInfra, FileReaderInfra,
};

use crate::range::resolve_range;

#[derive(Clone)]
pub struct PawsChatRequest<F> {
    infra: Arc<F>,
}

impl<F: FileReaderInfra + EnvironmentInfra + FileInfoInfra + DirectoryReaderInfra>
    PawsChatRequest<F>
{
    pub fn new(infra: Arc<F>) -> Self {
        Self { infra }
    }

    async fn prepare_attachments(&self, paths: Vec<FileTag>) -> anyhow::Result<Vec<Attachment>> {
        futures::future::join_all(paths.into_iter().map(|v| self.populate_attachments(v)))
            .await
            .into_iter()
            .collect::<anyhow::Result<Vec<_>>>()
    }

    async fn populate_attachments(&self, tag: FileTag) -> anyhow::Result<Attachment> {
        let mut path = tag.as_ref().to_path_buf();
        let extension = path.extension().map(|v| v.to_string_lossy().to_string());

        if !path.is_absolute() {
            path = self.infra.get_environment().cwd.join(path);
        }

        // Check if path is a directory (exists but is not a file)
        if self.infra.exists(&path).await? && !self.infra.is_file(&path).await? {
            // List all entries (files and directories) efficiently without reading file
            // contents
            let dir_entries = self.infra.list_directory_entries(&path).await?;

            // Create DirectoryEntry for each entry
            let mut entries: Vec<DirectoryEntry> = dir_entries
                .into_iter()
                .map(|(entry_path, is_dir)| {
                    let normalized_path = format_display_path(&entry_path, &path);
                    DirectoryEntry { path: normalized_path, is_dir }
                })
                .collect();

            // Sort entries: directories first, then by name
            entries.sort_by(|a, b| {
                // Directories come before files
                match (a.is_dir, b.is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.path.cmp(&b.path), // Same type, sort by name
                }
            });

            return Ok(Attachment {
                content: AttachmentContent::DirectoryListing { entries },
                path: path.to_string_lossy().to_string(), // Keep root path absolute
            });
        }

        // Determine file type (text or image with format)
        let mime_type = extension.and_then(|ext| match ext.as_str() {
            "jpeg" | "jpg" => Some("image/jpeg".to_string()),
            "png" => Some("image/png".to_string()),
            "webp" => Some("image/webp".to_string()),
            _ => None,
        });

        //NOTE: Apply the same slicing as file reads for text content
        let content = match mime_type {
            Some(mime_type) => {
                AttachmentContent::Image(Image::new_bytes(self.infra.read(&path).await?, mime_type))
            }
            None => {
                let env = self.infra.get_environment();

                let start = tag.loc.as_ref().and_then(|loc| loc.start);
                let end = tag.loc.as_ref().and_then(|loc| loc.end);
                let (start_line, end_line) = resolve_range(start, end, env.max_read_size);

                let (file_content, file_info) = self
                    .infra
                    .range_read_utf8(&path, start_line, end_line)
                    .await?;

                AttachmentContent::FileContent {
                    content: file_content.to_numbered_from(file_info.start_line as usize),
                    start_line: file_info.start_line,
                    end_line: file_info.end_line,
                    total_lines: file_info.total_lines,
                }
            }
        };

        Ok(Attachment {
            content,
            path: path.to_string_lossy().to_string(), // Keep root path absolute
        })
    }
}

#[async_trait::async_trait]
impl<F: FileReaderInfra + EnvironmentInfra + FileInfoInfra + DirectoryReaderInfra> AttachmentService
    for PawsChatRequest<F>
{
    async fn attachments(&self, url: &str) -> anyhow::Result<Vec<Attachment>> {
        self.prepare_attachments(Attachment::parse_all(url)).await
    }
}
