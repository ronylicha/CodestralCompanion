use similar::{ChangeTag, TextDiff};
use colored::*;
use std::fs;
use std::path::Path;
use std::io::{self, Write};

/// Represents a file modification
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub original: String,
    pub modified: String,
    pub description: String,
}

impl FileChange {
    /// Generate a colored unified diff
    pub fn display_diff(&self) -> String {
        let diff = TextDiff::from_lines(&self.original, &self.modified);
        let mut output = String::new();
        
        output.push_str(&format!("\n{}\n", "─".repeat(60).dimmed()));
        output.push_str(&format!("{} {}\n", "📄".to_string(), self.path.bold()));
        if !self.description.is_empty() {
            output.push_str(&format!("   {}\n", self.description.dimmed()));
        }
        output.push_str(&format!("{}\n", "─".repeat(60).dimmed()));

        for change in diff.iter_all_changes() {
            let sign = match change.tag() {
                ChangeTag::Delete => format!("{}", format!("-{}", change).red()),
                ChangeTag::Insert => format!("{}", format!("+{}", change).green()),
                ChangeTag::Equal => format!(" {}", change),
            };
            output.push_str(&sign);
        }

        output
    }

    /// Apply the change to the filesystem
    pub fn apply(&self) -> Result<(), String> {
        fs::write(&self.path, &self.modified)
            .map_err(|e| format!("Failed to write {}: {}", self.path, e))
    }
}

/// Represents a new file to create
#[derive(Debug, Clone)]
pub struct NewFile {
    pub path: String,
    pub content: String,
    pub description: String,
}

impl NewFile {
    pub fn display(&self) -> String {
        let mut output = String::new();
        
        output.push_str(&format!("\n{}\n", "─".repeat(60).dimmed()));
        output.push_str(&format!("{} {} {}\n", "📄".to_string(), "[NEW]".green().bold(), self.path.bold()));
        if !self.description.is_empty() {
            output.push_str(&format!("   {}\n", self.description.dimmed()));
        }
        output.push_str(&format!("{}\n", "─".repeat(60).dimmed()));
        
        for line in self.content.lines().take(20) {
            output.push_str(&format!("{}\n", format!("+{}", line).green()));
        }
        
        if self.content.lines().count() > 20 {
            output.push_str(&format!("{}\n", "... (truncated)".dimmed()));
        }

        output
    }

    pub fn apply(&self) -> Result<(), String> {
        // Create parent directories if needed
        if let Some(parent) = Path::new(&self.path).parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directories: {}", e))?;
        }
        fs::write(&self.path, &self.content)
            .map_err(|e| format!("Failed to write {}: {}", self.path, e))
    }
}

/// Represents all changes from an agent response
#[derive(Debug, Default)]
pub struct ChangeSet {
    pub plan: Vec<String>,
    pub modifications: Vec<FileChange>,
    pub new_files: Vec<NewFile>,
    pub deletions: Vec<String>,
}

impl ChangeSet {
    pub fn is_empty(&self) -> bool {
        self.modifications.is_empty() && self.new_files.is_empty() && self.deletions.is_empty()
    }

    pub fn display_plan(&self) {
        if !self.plan.is_empty() {
            println!("\n{}", "📋 PLAN D'ACTION".bold().cyan());
            println!("{}", "─".repeat(40).dimmed());
            for (i, step) in self.plan.iter().enumerate() {
                println!("  {}. {}", (i + 1).to_string().bold(), step);
            }
            println!();
        }
    }

    pub fn display_all_changes(&self) {
        for change in &self.modifications {
            println!("{}", change.display_diff());
        }
        for new_file in &self.new_files {
            println!("{}", new_file.display());
        }
        for deletion in &self.deletions {
            println!("\n{} {} {}", "📄".to_string(), "[DELETE]".red().bold(), deletion.bold());
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "{} modifications, {} nouveaux fichiers, {} suppressions",
            self.modifications.len(),
            self.new_files.len(),
            self.deletions.len()
        )
    }
}

/// Ask for user confirmation
pub fn confirm(prompt: &str) -> bool {
    print!("{} [o/N] ", prompt.yellow());
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    
    matches!(input.trim().to_lowercase().as_str(), "o" | "oui" | "y" | "yes")
}

/// Parse AI response to extract changes
/// Expected format:
/// <plan>
/// 1. Step one
/// 2. Step two
/// </plan>
/// 
/// <file path="src/main.rs">
/// <<<<<<< ORIGINAL
/// old code
/// =======
/// new code
/// >>>>>>> MODIFIED
/// </file>
/// 
/// <new_file path="src/new.rs">
/// content
/// </new_file>
pub fn parse_ai_response(response: &str, base_path: &Path) -> ChangeSet {
    let mut changes = ChangeSet::default();

    // Extract plan
    if let Some(plan_start) = response.find("<plan>") {
        if let Some(plan_end) = response.find("</plan>") {
            let plan_content = &response[plan_start + 6..plan_end];
            for line in plan_content.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    // Remove leading numbers like "1. " or "- "
                    let cleaned = trimmed
                        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '-' || c == ' ');
                    if !cleaned.is_empty() {
                        changes.plan.push(cleaned.to_string());
                    }
                }
            }
        }
    }

    // Extract file modifications
    let file_pattern = regex::Regex::new(r#"<file\s+path="([^"]+)">"#).unwrap();
    for cap in file_pattern.captures_iter(response) {
        let path = &cap[1];
        let full_path = base_path.join(path);
        
        // Find the content between <file> and </file>
        let tag_start = cap.get(0).unwrap().end();
        if let Some(relative_end) = response[tag_start..].find("</file>") {
            let content = &response[tag_start..tag_start + relative_end];
            
            // Parse ORIGINAL/MODIFIED markers
            if let Some(orig_start) = content.find("<<<<<<< ORIGINAL") {
                if let Some(sep) = content.find("=======") {
                    if let Some(mod_end) = content.find(">>>>>>> MODIFIED") {
                        let original = content[orig_start + 16..sep].trim();
                        let modified = content[sep + 7..mod_end].trim();
                        
                        // Read current file content
                        let current_content = fs::read_to_string(&full_path).unwrap_or_default();
                        
                        // Replace the original with modified in current content
                        let new_content = current_content.replace(original, modified);
                        
                        if new_content != current_content {
                            changes.modifications.push(FileChange {
                                path: full_path.to_string_lossy().to_string(),
                                original: current_content,
                                modified: new_content,
                                description: String::new(),
                            });
                        }
                    }
                }
            }
        }
    }

    // Extract new files
    let new_file_pattern = regex::Regex::new(r#"<new_file\s+path="([^"]+)">"#).unwrap();
    for cap in new_file_pattern.captures_iter(response) {
        let path = &cap[1];
        let full_path = base_path.join(path);
        
        let tag_start = cap.get(0).unwrap().end();
        if let Some(relative_end) = response[tag_start..].find("</new_file>") {
            let content = response[tag_start..tag_start + relative_end].trim();
            
            changes.new_files.push(NewFile {
                path: full_path.to_string_lossy().to_string(),
                content: content.to_string(),
                description: String::new(),
            });
        }
    }

    changes
}
