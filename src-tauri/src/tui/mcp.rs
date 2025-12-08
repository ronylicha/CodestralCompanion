use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// MCP Server configuration (matches standard MCP config format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Command to run (e.g., "npx")
    pub command: String,
    /// Arguments (e.g., ["-y", "@upstash/context7-mcp", "--api-key", "KEY"])
    pub args: Vec<String>,
    /// Environment variables
    pub env: Option<HashMap<String, String>>,
}

/// MCP configuration file structure (matches standard format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

impl McpConfig {
    /// Load MCP configuration from project directory
    pub fn load(project_path: &Path) -> Option<Self> {
        let config_path = project_path.join(".codestral").join("mcp_servers.json");
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path).ok()?;
            serde_json::from_str(&content).ok()
        } else {
            None
        }
    }
    
    /// Create default configuration file
    pub fn create_default(project_path: &Path) -> std::io::Result<()> {
        let config_dir = project_path.join(".codestral");
        std::fs::create_dir_all(&config_dir)?;
        
        let default_config = McpConfig {
            mcp_servers: HashMap::from([
                ("context7".to_string(), McpServerConfig {
                    command: "npx".to_string(),
                    args: vec!["-y".to_string(), "@upstash/context7-mcp".to_string()],
                    env: None,
                }),
            ]),
        };
        
        let json = serde_json::to_string_pretty(&default_config)?;
        std::fs::write(config_dir.join("mcp_servers.json"), json)?;
        Ok(())
    }
}

/// MCP Tool definition received from server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Option<Value>,
}

/// Active MCP server process
pub struct McpServer {
    name: String,
    process: Child,
    request_id: u64,
    tools: Vec<McpTool>,
}

impl McpServer {
    /// Start an MCP server process
    pub fn start(name: &str, config: &McpServerConfig, project_path: &Path) -> Result<Self, String> {
        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args)
            .current_dir(project_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        
        // Add environment variables if any
        if let Some(env) = &config.env {
            for (key, value) in env {
                cmd.env(key, value);
            }
        }
        
        let process = cmd.spawn()
            .map_err(|e| format!("Failed to start MCP server '{}': {}", name, e))?;
        
        let mut server = McpServer {
            name: name.to_string(),
            process,
            request_id: 0,
            tools: Vec::new(),
        };
        
        // Initialize the server
        server.initialize()?;
        
        // Get available tools
        server.list_tools()?;
        
        Ok(server)
    }
    
    /// Send a JSON-RPC request to the server
    fn send_request(&mut self, method: &str, params: Option<Value>) -> Result<Value, String> {
        self.request_id += 1;
        
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": method,
            "params": params.unwrap_or(json!({}))
        });
        
        let stdin = self.process.stdin.as_mut()
            .ok_or("Failed to get stdin")?;
        
        let request_str = serde_json::to_string(&request)
            .map_err(|e| format!("Failed to serialize request: {}", e))?;
        
        writeln!(stdin, "{}", request_str)
            .map_err(|e| format!("Failed to write to stdin: {}", e))?;
        stdin.flush()
            .map_err(|e| format!("Failed to flush stdin: {}", e))?;
        
        // Read response
        let stdout = self.process.stdout.as_mut()
            .ok_or("Failed to get stdout")?;
        
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line)
            .map_err(|e| format!("Failed to read response: {}", e))?;
        
        let response: Value = serde_json::from_str(&line)
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        
        if let Some(error) = response.get("error") {
            return Err(format!("RPC error: {}", error));
        }
        
        Ok(response.get("result").cloned().unwrap_or(json!(null)))
    }
    
    /// Initialize the MCP server
    fn initialize(&mut self) -> Result<(), String> {
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "roots": { "listChanged": true },
                "sampling": {}
            },
            "clientInfo": {
                "name": "companion-chat",
                "version": "0.8.0"
            }
        });
        
        self.send_request("initialize", Some(params))?;
        
        // Send initialized notification
        let stdin = self.process.stdin.as_mut()
            .ok_or("Failed to get stdin")?;
        
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        
        writeln!(stdin, "{}", serde_json::to_string(&notification).unwrap())
            .map_err(|e| format!("Failed to send initialized: {}", e))?;
        stdin.flush().ok();
        
        Ok(())
    }
    
    /// List available tools from the server
    fn list_tools(&mut self) -> Result<(), String> {
        let result = self.send_request("tools/list", None)?;
        
        if let Some(tools) = result.get("tools").and_then(|t| t.as_array()) {
            self.tools = tools.iter()
                .filter_map(|t| serde_json::from_value(t.clone()).ok())
                .collect();
        }
        
        Ok(())
    }
    
    /// Get the list of tools from this server
    pub fn get_tools(&self) -> &[McpTool] {
        &self.tools
    }
    
    /// Call a tool on this server
    pub fn call_tool(&mut self, tool_name: &str, arguments: Value) -> Result<String, String> {
        let params = json!({
            "name": tool_name,
            "arguments": arguments
        });
        
        let result = self.send_request("tools/call", Some(params))?;
        
        // Extract content from result
        if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
            let text_parts: Vec<String> = content.iter()
                .filter_map(|item| {
                    if item.get("type")?.as_str()? == "text" {
                        item.get("text")?.as_str().map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            return Ok(text_parts.join("\n"));
        }
        
        Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
    }
    
    /// Get server name
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for McpServer {
    fn drop(&mut self) {
        // Kill the server process when dropped
        let _ = self.process.kill();
    }
}

/// MCP Manager to handle multiple servers
pub struct McpManager {
    servers: Vec<McpServer>,
}

impl McpManager {
    /// Create a new MCP manager
    pub fn new() -> Self {
        Self { servers: Vec::new() }
    }
    
    /// Start all MCP servers from config
    pub fn start_from_config(&mut self, project_path: &Path) -> Vec<String> {
        let mut started = Vec::new();
        
        if let Some(config) = McpConfig::load(project_path) {
            for (name, server_config) in &config.mcp_servers {
                match McpServer::start(name, server_config, project_path) {
                    Ok(server) => {
                        started.push(format!("{} ({} tools)", name, server.tools.len()));
                        self.servers.push(server);
                    }
                    Err(e) => {
                        eprintln!("Failed to start MCP server '{}': {}", name, e);
                    }
                }
            }
        }
        
        started
    }
    
    /// Get all available tools from all servers
    pub fn get_all_tools(&self) -> Vec<(String, McpTool)> {
        let mut all_tools = Vec::new();
        for server in &self.servers {
            for tool in server.get_tools() {
                all_tools.push((server.name().to_string(), tool.clone()));
            }
        }
        all_tools
    }
    
    /// Call a tool by name
    pub fn call_tool(&mut self, server_name: &str, tool_name: &str, arguments: Value) -> Result<String, String> {
        for server in &mut self.servers {
            if server.name() == server_name {
                return server.call_tool(tool_name, arguments);
            }
        }
        Err(format!("Server '{}' not found", server_name))
    }
    
    /// Generate tools documentation for system prompt
    pub fn get_tools_documentation(&self) -> String {
        if self.servers.is_empty() {
            return String::new();
        }
        
        let mut doc = String::from("\n## MCP External Tools\n\n");
        
        for server in &self.servers {
            doc.push_str(&format!("### {} Server\n\n", server.name()));
            
            for tool in server.get_tools() {
                doc.push_str(&format!("#### {}\n", tool.name));
                if let Some(desc) = &tool.description {
                    doc.push_str(&format!("{}\n", desc));
                }
                doc.push_str("```xml\n<tool_call>\n<name>mcp_");
                doc.push_str(server.name());
                doc.push_str("_");
                doc.push_str(&tool.name);
                doc.push_str("</name>\n<params>\n");
                
                // Add schema hints if available
                if let Some(schema) = &tool.input_schema {
                    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
                        for (prop_name, _) in properties {
                            doc.push_str(&format!("<{}>(value)</{}>\n", prop_name, prop_name));
                        }
                    }
                }
                
                doc.push_str("</params>\n</tool_call>\n```\n\n");
            }
        }
        
        doc
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}
