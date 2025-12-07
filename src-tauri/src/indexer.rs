use std::path::{Path, PathBuf};
use std::fs;
use ignore::WalkBuilder;
use indicatif::{ProgressBar, ProgressStyle};

/// Supported file extensions for code analysis
const DEFAULT_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "kt", "swift",
    "c", "cpp", "h", "hpp", "cs", "rb", "php", "vue", "svelte",
    "html", "css", "scss", "sass", "less", "json", "yaml", "yml",
    "toml", "md", "sql", "sh", "bash", "zsh", "fish",
];

/// Maximum file size to read (100KB)
const MAX_FILE_SIZE: u64 = 100_000;

#[derive(Debug, Clone)]
pub struct IndexedFile {
    pub path: PathBuf,
    pub relative_path: String,
    pub content: String,
    pub extension: String,
    pub size: u64,
}

#[derive(Debug)]
pub struct CodebaseIndex {
    pub root: PathBuf,
    pub files: Vec<IndexedFile>,
    pub total_tokens_estimate: usize,
}

impl CodebaseIndex {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            files: Vec::new(),
            total_tokens_estimate: 0,
        }
    }

    /// Index a codebase directory
    pub fn index(
        root: &Path,
        include_extensions: Option<&[String]>,
        exclude_dirs: &[String],
        max_files: usize,
    ) -> Result<Self, String> {
        let root = root.canonicalize().map_err(|e| format!("Invalid path: {}", e))?;
        
        let mut index = CodebaseIndex::new(root.clone());
        
        // Build the walker respecting .gitignore
        let mut builder = WalkBuilder::new(&root);
        builder.hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true);
        
        // Add default excludes
        let default_excludes: Vec<String> = vec![
            "node_modules", "target", "dist", "build", ".git", "__pycache__",
            "vendor", ".venv", "venv", ".idea", ".vscode", "coverage",
        ].into_iter().map(|s| s.to_string()).collect();
        
        let mut all_excludes = default_excludes;
        all_excludes.extend(exclude_dirs.iter().cloned());

        // Collect files first to show progress
        let entries: Vec<_> = builder.build()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .collect();

        let pb = ProgressBar::new(entries.len().min(max_files) as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} fichiers indexés")
            .unwrap()
            .progress_chars("#>-"));

        let mut file_count = 0;

        for entry in entries {
            if file_count >= max_files {
                break;
            }

            let path = entry.path();
            
            // Check if in excluded directory
            let path_str = path.to_string_lossy();
            if all_excludes.iter().any(|exc| path_str.contains(exc.as_str())) {
                continue;
            }

            // Check extension
            let ext = path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let should_include = if let Some(exts) = include_extensions {
                exts.iter().any(|e| e.to_lowercase() == ext)
            } else {
                DEFAULT_EXTENSIONS.contains(&ext.as_str())
            };

            if !should_include {
                continue;
            }

            // Check file size
            let metadata = match fs::metadata(path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            if metadata.len() > MAX_FILE_SIZE {
                continue;
            }

            // Read content
            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue, // Skip binary files
            };

            let relative_path = path.strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            // Estimate tokens (rough: 1 token ≈ 4 chars)
            let token_estimate = content.len() / 4;
            index.total_tokens_estimate += token_estimate;

            index.files.push(IndexedFile {
                path: path.to_path_buf(),
                relative_path,
                content,
                extension: ext,
                size: metadata.len(),
            });

            file_count += 1;
            pb.inc(1);
        }

        pb.finish_with_message(format!("{} fichiers indexés", index.files.len()));

        Ok(index)
    }

    /// Get a summary of the indexed codebase
    pub fn summary(&self) -> String {
        let mut by_ext: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        
        for file in &self.files {
            *by_ext.entry(&file.extension).or_insert(0) += 1;
        }

        let mut summary = format!("📁 Codebase: {}\n", self.root.display());
        summary.push_str(&format!("📄 {} fichiers indexés\n", self.files.len()));
        summary.push_str(&format!("🔤 ~{} tokens estimés\n\n", self.total_tokens_estimate));
        
        summary.push_str("Par type:\n");
        let mut sorted: Vec<_> = by_ext.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        
        for (ext, count) in sorted.iter().take(10) {
            summary.push_str(&format!("  .{}: {}\n", ext, count));
        }

        summary
    }

    /// Build context for AI with file contents (chunked if needed)
    pub fn build_context(&self, max_tokens: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();
        let mut current_tokens = 0;

        for file in &self.files {
            let file_header = format!("\n--- {} ---\n", file.relative_path);
            let file_tokens = (file_header.len() + file.content.len()) / 4;

            if current_tokens + file_tokens > max_tokens && !current_chunk.is_empty() {
                chunks.push(current_chunk);
                current_chunk = String::new();
                current_tokens = 0;
            }

            current_chunk.push_str(&file_header);
            current_chunk.push_str(&file.content);
            current_tokens += file_tokens;
        }

        if !current_chunk.is_empty() {
            chunks.push(current_chunk);
        }

        chunks
    }
}
