use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;
use regex::Regex;

/// Tool call parsed from AI response
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub params: std::collections::HashMap<String, String>,
}

/// Tool execution result
#[derive(Debug)]
pub struct ToolResult {
    pub name: String,
    pub success: bool,
    pub output: String,
    pub needs_confirmation: bool,
}

/// Dangerous commands that require user confirmation
const DANGEROUS_COMMANDS: &[&str] = &[
    "rm", "rmdir", "sudo", "chmod", "chown", "dd", "mkfs",
    "kill", "pkill", "killall", "shutdown", "reboot", "halt",
    "format", "fdisk", "parted", "mount", "umount",
];

/// Check if a command is potentially dangerous
pub fn is_dangerous_command(command: &str) -> bool {
    let first_word = command.split_whitespace().next().unwrap_or("");
    
    // Check if command starts with dangerous word
    for dangerous in DANGEROUS_COMMANDS {
        if first_word == *dangerous || first_word.ends_with(&format!("/{}", dangerous)) {
            return true;
        }
    }
    
    // Check for piped dangerous commands
    for part in command.split('|').chain(command.split("&&")).chain(command.split(";")) {
        let trimmed = part.trim();
        let first = trimmed.split_whitespace().next().unwrap_or("");
        for dangerous in DANGEROUS_COMMANDS {
            if first == *dangerous || first.ends_with(&format!("/{}", dangerous)) {
                return true;
            }
        }
    }
    
    false
}

/// Check if path is within project directory
pub fn is_path_within_project(path: &Path, project_root: &Path) -> bool {
    match path.canonicalize() {
        Ok(canonical) => canonical.starts_with(project_root),
        Err(_) => {
            // Path doesn't exist yet, check parent
            if let Some(parent) = path.parent() {
                if parent.as_os_str().is_empty() {
                    // Relative path, assume OK
                    true
                } else {
                    is_path_within_project(parent, project_root)
                }
            } else {
                false
            }
        }
    }
}

/// Resolve path relative to project root
pub fn resolve_path(path_str: &str, project_root: &Path) -> PathBuf {
    let path = Path::new(path_str);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
}

/// Parse tool calls from AI response
pub fn parse_tool_calls(response: &str) -> Vec<ToolCall> {
    let mut tools = Vec::new();
    
    // Pattern: <tool_call>...</tool_call>
    let tool_call_re = Regex::new(r"(?s)<tool_call>(.*?)</tool_call>").unwrap();
    let name_re = Regex::new(r"(?s)<name>(.*?)</name>").unwrap();
    let params_re = Regex::new(r"(?s)<params>(.*?)</params>").unwrap();
    let param_re = Regex::new(r"(?s)<(\w+)>(.*?)</\1>").unwrap();
    
    for cap in tool_call_re.captures_iter(response) {
        let content = &cap[1];
        
        let name = name_re.captures(content)
            .map(|c| c[1].trim().to_string())
            .unwrap_or_default();
        
        let mut params = std::collections::HashMap::new();
        
        if let Some(params_cap) = params_re.captures(content) {
            let params_content = &params_cap[1];
            for param_cap in param_re.captures_iter(params_content) {
                let key = param_cap[1].to_string();
                let value = param_cap[2].trim().to_string();
                params.insert(key, value);
            }
        }
        
        if !name.is_empty() {
            tools.push(ToolCall { name, params });
        }
    }
    
    tools
}

/// Execute a tool and return the result
pub fn execute_tool(tool: &ToolCall, project_root: &Path) -> ToolResult {
    match tool.name.as_str() {
        "read_file" => execute_read_file(tool, project_root),
        "write_file" => execute_write_file(tool, project_root),
        "list_directory" => execute_list_directory(tool, project_root),
        "search_in_files" => execute_search_in_files(tool, project_root),
        "execute_bash" => execute_bash(tool, project_root),
        _ => ToolResult {
            name: tool.name.clone(),
            success: false,
            output: format!("Unknown tool: {}", tool.name),
            needs_confirmation: false,
        },
    }
}

fn execute_read_file(tool: &ToolCall, project_root: &Path) -> ToolResult {
    let path_str = tool.params.get("path").cloned().unwrap_or_default();
    let path = resolve_path(&path_str, project_root);
    
    if !is_path_within_project(&path, project_root) {
        return ToolResult {
            name: tool.name.clone(),
            success: false,
            output: format!("Access denied: {} is outside project directory", path_str),
            needs_confirmation: false,
        };
    }
    
    match fs::read_to_string(&path) {
        Ok(content) => ToolResult {
            name: tool.name.clone(),
            success: true,
            output: content,
            needs_confirmation: false,
        },
        Err(e) => ToolResult {
            name: tool.name.clone(),
            success: false,
            output: format!("Error reading file: {}", e),
            needs_confirmation: false,
        },
    }
}

fn execute_write_file(tool: &ToolCall, project_root: &Path) -> ToolResult {
    let path_str = tool.params.get("path").cloned().unwrap_or_default();
    let content = tool.params.get("content").cloned().unwrap_or_default();
    let path = resolve_path(&path_str, project_root);
    
    if !is_path_within_project(&path, project_root) {
        return ToolResult {
            name: tool.name.clone(),
            success: false,
            output: format!("Access denied: {} is outside project directory", path_str),
            needs_confirmation: false,
        };
    }
    
    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    
    match fs::write(&path, &content) {
        Ok(_) => ToolResult {
            name: tool.name.clone(),
            success: true,
            output: format!("File written: {} ({} bytes)", path_str, content.len()),
            needs_confirmation: false,
        },
        Err(e) => ToolResult {
            name: tool.name.clone(),
            success: false,
            output: format!("Error writing file: {}", e),
            needs_confirmation: false,
        },
    }
}

fn execute_list_directory(tool: &ToolCall, project_root: &Path) -> ToolResult {
    let path_str = tool.params.get("path").cloned().unwrap_or(".".to_string());
    let path = resolve_path(&path_str, project_root);
    
    if !is_path_within_project(&path, project_root) {
        return ToolResult {
            name: tool.name.clone(),
            success: false,
            output: format!("Access denied: {} is outside project directory", path_str),
            needs_confirmation: false,
        };
    }
    
    match fs::read_dir(&path) {
        Ok(entries) => {
            let mut items: Vec<String> = entries
                .filter_map(|e| e.ok())
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    if is_dir {
                        format!("{}/", name)
                    } else {
                        name
                    }
                })
                .collect();
            items.sort();
            ToolResult {
                name: tool.name.clone(),
                success: true,
                output: items.join("\n"),
                needs_confirmation: false,
            }
        }
        Err(e) => ToolResult {
            name: tool.name.clone(),
            success: false,
            output: format!("Error listing directory: {}", e),
            needs_confirmation: false,
        },
    }
}

fn execute_search_in_files(tool: &ToolCall, project_root: &Path) -> ToolResult {
    let query = tool.params.get("query").cloned().unwrap_or_default();
    let path_str = tool.params.get("path").cloned().unwrap_or(".".to_string());
    let path = resolve_path(&path_str, project_root);
    
    if !is_path_within_project(&path, project_root) {
        return ToolResult {
            name: tool.name.clone(),
            success: false,
            output: format!("Access denied: {} is outside project directory", path_str),
            needs_confirmation: false,
        };
    }
    
    // Use grep for searching
    let output = Command::new("grep")
        .args(["-rn", "--include=*", &query])
        .current_dir(&path)
        .output();
    
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let result = if stdout.is_empty() {
                "No matches found".to_string()
            } else {
                // Limit output to first 50 matches
                stdout.lines().take(50).collect::<Vec<_>>().join("\n")
            };
            ToolResult {
                name: tool.name.clone(),
                success: true,
                output: result,
                needs_confirmation: false,
            }
        }
        Err(e) => ToolResult {
            name: tool.name.clone(),
            success: false,
            output: format!("Error searching: {}", e),
            needs_confirmation: false,
        },
    }
}

fn execute_bash(tool: &ToolCall, project_root: &Path) -> ToolResult {
    let command = tool.params.get("command").cloned().unwrap_or_default();
    
    if command.is_empty() {
        return ToolResult {
            name: tool.name.clone(),
            success: false,
            output: "No command provided".to_string(),
            needs_confirmation: false,
        };
    }
    
    // Check if dangerous - return early, needs confirmation
    if is_dangerous_command(&command) {
        return ToolResult {
            name: tool.name.clone(),
            success: false,
            output: format!("DANGEROUS COMMAND DETECTED: {}", command),
            needs_confirmation: true,
        };
    }
    
    // Execute safe command
    let output = Command::new("bash")
        .args(["-c", &command])
        .current_dir(project_root)
        .output();
    
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let combined = if stderr.is_empty() {
                stdout.to_string()
            } else if stdout.is_empty() {
                format!("STDERR:\n{}", stderr)
            } else {
                format!("{}\nSTDERR:\n{}", stdout, stderr)
            };
            ToolResult {
                name: tool.name.clone(),
                success: out.status.success(),
                output: combined,
                needs_confirmation: false,
            }
        }
        Err(e) => ToolResult {
            name: tool.name.clone(),
            success: false,
            output: format!("Error executing command: {}", e),
            needs_confirmation: false,
        },
    }
}

/// Execute a dangerous command after user confirmation
pub fn execute_dangerous_bash(command: &str, project_root: &Path) -> ToolResult {
    let output = Command::new("bash")
        .args(["-c", command])
        .current_dir(project_root)
        .output();
    
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let combined = if stderr.is_empty() {
                stdout.to_string()
            } else if stdout.is_empty() {
                format!("STDERR:\n{}", stderr)
            } else {
                format!("{}\nSTDERR:\n{}", stdout, stderr)
            };
            ToolResult {
                name: "execute_bash".to_string(),
                success: out.status.success(),
                output: combined,
                needs_confirmation: false,
            }
        }
        Err(e) => ToolResult {
            name: "execute_bash".to_string(),
            success: false,
            output: format!("Error executing command: {}", e),
            needs_confirmation: false,
        },
    }
}

/// Format tool result for sending back to AI
pub fn format_tool_result(result: &ToolResult) -> String {
    format!(
        "<tool_result>\n<name>{}</name>\n<success>{}</success>\n<output>\n{}\n</output>\n</tool_result>",
        result.name,
        result.success,
        result.output
    )
}

/// Get tools documentation for system prompt
pub fn get_tools_documentation() -> &'static str {
    r#"
## Available Tools

You can use the following tools by including tool_call blocks in your response:

### read_file
Read the content of a file.
```xml
<tool_call>
<name>read_file</name>
<params>
<path>src/main.rs</path>
</params>
</tool_call>
```

### write_file
Create or overwrite a file.
```xml
<tool_call>
<name>write_file</name>
<params>
<path>src/new_file.rs</path>
<content>
// Your code here
fn hello() {}
</content>
</params>
</tool_call>
```

### list_directory
List files and directories.
```xml
<tool_call>
<name>list_directory</name>
<params>
<path>src/</path>
</params>
</tool_call>
```

### search_in_files
Search for text in project files.
```xml
<tool_call>
<name>search_in_files</name>
<params>
<query>fn main</query>
<path>src/</path>
</params>
</tool_call>
```

### execute_bash
Execute a shell command.
```xml
<tool_call>
<name>execute_bash</name>
<params>
<command>cargo build</command>
</params>
</tool_call>
```

## Important Rules
1. File access is limited to the project directory
2. You can make multiple tool calls in one response
3. After tool calls, you will receive tool_result blocks with outputs
4. Continue your work based on tool results
5. Dangerous commands (rm, sudo, etc.) require user confirmation
"#
}
