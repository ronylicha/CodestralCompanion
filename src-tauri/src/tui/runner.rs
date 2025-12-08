use std::io;
use std::path::PathBuf;
use std::time::Duration;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use crate::tui::app::App;
use crate::tui::ui;
use crate::tui::tools;
use crate::tui::mcp::McpManager;
use crate::mistral_client::{MistralClient, ApiProvider, Message};
use crate::agent::load_api_settings;
use crate::indexer::CodebaseIndex;
use crate::persistent_index::PersistentIndex;
use crate::differ::parse_ai_response;
use crate::chat::ChatMode;

const SYSTEM_PROMPT: &str = r#"Tu es un assistant de programmation expert. Tu analyses des codebases et proposes des modifications.

RÈGLES:
1. Réponds TOUJOURS en français
2. Sois précis et concis
3. UTILISE LES OUTILS disponibles pour accéder aux fichiers et exécuter des commandes
4. Pour modifier un fichier existant (sans outil):

<file path="chemin/fichier.ext">
<<<<<<< ORIGINAL
code original
=======
nouveau code
>>>>>>> MODIFIED
</file>

Pour créer un fichier (sans outil):
<new_file path="chemin/nouveau.ext">
contenu
</new_file>

BASE DE DONNÉES SQLite:
- Tu as accès à un index SQLite du projet qui contient tous les fichiers avec leur contenu
- L'index est automatiquement mis à jour lorsque des fichiers changent (vérification par hash SHA256)
- La liste des fichiers indexés est fournie ci-dessous
- Utilise ces informations pour comprendre la structure du projet
"#;

const AUTO_MODE_SUFFIX: &str = r#"

MODE AUTO ACTIVÉ:
- Tu DOIS continuer à travailler jusqu'à ce que la tâche soit COMPLÈTEMENT terminée
- Fais TOUTES les modifications nécessaires en une seule réponse
- Ne demande PAS de confirmation, applique directement
- Si la tâche nécessite plusieurs étapes, fais-les TOUTES
- Termine ta réponse par [TERMINÉ] quand la tâche est complète
- Si tu dois continuer, termine par [CONTINUE] et je te relancerai automatiquement
"#;

const COMPACT_PROMPT: &str = "Résume en 2-3 phrases les échanges précédents pour garder le contexte essentiel. Sois très concis.";

const MAX_TOKENS: usize = 32000;
const COMPACT_THRESHOLD: usize = (MAX_TOKENS * 90) / 100; // 90%

/// Command menu items
pub const COMMANDS: &[(&str, &str)] = &[
    ("new", "Nouvelle conversation"),
    ("resume", "Reprendre une conversation"),
    ("save", "Sauvegarder la conversation"),
    ("memory", "Éditer les instructions projet (vim)"),
    ("questions", "Test formulaire tabbé"),
    ("clear", "Effacer l'historique"),
    ("reindex", "Réindexer le projet"),
    ("ask", "Mode ASK - Questions simples"),
    ("plan", "Mode PLAN - Planification"),
    ("code", "Mode CODE - Modifications avec confirmation"),
    ("auto", "Mode AUTO - Application automatique"),
    ("exit", "Sauvegarder et quitter"),
    ("quit", "Quitter sans sauvegarder"),
];

pub struct TuiRunner {
    app: App,
    client: MistralClient,
    system_prompt: String,
    project_memory: String,
    memory_file: PathBuf,
    show_command_menu: bool,
    command_filter: String,
    selected_command: usize,
    persistent_index: Option<PersistentIndex>,
    mcp_manager: McpManager,
}

impl TuiRunner {
    pub fn new(project_path: PathBuf) -> Result<Self, String> {
        let (api_key, provider) = load_api_settings()?;
        
        // Index codebase for context (in-memory, quick)
        let index = CodebaseIndex::index(&project_path, None, &[], 50)?;
        let context = index.build_context(20000);
        let codebase_context = context.first().cloned().unwrap_or_default();
        
        // Open or create persistent SQLite index
        let persistent_index = PersistentIndex::open(&project_path).ok();
        
        // Build SQLite index info for system prompt
        let sqlite_info = if let Some(ref pindex) = persistent_index {
            if let Ok(files) = pindex.list_files() {
                let file_list: Vec<String> = files.iter()
                    .take(100)
                    .map(|f| format!("- {} ({})", f.relative_path, f.extension))
                    .collect();
                if !file_list.is_empty() {
                    format!("\n\nINDEX SQLITE ({} fichiers):\n{}", 
                        files.len(), 
                        file_list.join("\n"))
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        
        // Load project memory file
        let memory_file = project_path.join(".codestral").join("memory.md");
        let project_memory = if memory_file.exists() {
            std::fs::read_to_string(&memory_file).unwrap_or_default()
        } else {
            String::new()
        };
        
        let mut system_prompt = format!("{}\n\n{}\n\nCODEBASE:\n{}{}", 
            SYSTEM_PROMPT, 
            tools::get_tools_documentation(),
            codebase_context, 
            sqlite_info
        );
        
        // Initialize MCP servers - create default config if not exists
        let mcp_config_path = project_path.join(".codestral").join("mcp_servers.json");
        if !mcp_config_path.exists() {
            let _ = crate::tui::mcp::McpConfig::create_default(&project_path);
        }
        
        let mut mcp_manager = McpManager::new();
        let started_servers = mcp_manager.start_from_config(&project_path);
        
        // Add MCP tools documentation to system prompt
        let mcp_docs = mcp_manager.get_tools_documentation();
        if !mcp_docs.is_empty() {
            system_prompt = format!("{}\n{}", system_prompt, mcp_docs);
        }
        
        Ok(Self {
            app: App::new(project_path),
            client: MistralClient::new(api_key, provider),
            system_prompt,
            project_memory,
            memory_file,
            show_command_menu: false,
            command_filter: String::new(),
            selected_command: 0,
            persistent_index,
            mcp_manager,
        })
    }

    pub async fn run(&mut self) -> Result<(), String> {
        // Setup terminal
        enable_raw_mode().map_err(|e| e.to_string())?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen).map_err(|e| e.to_string())?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).map_err(|e| e.to_string())?;

        let result = self.run_loop(&mut terminal).await;

        // Restore terminal
        disable_raw_mode().map_err(|e| e.to_string())?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen).map_err(|e| e.to_string())?;
        terminal.show_cursor().map_err(|e| e.to_string())?;

        result
    }

    async fn run_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), String> {
        loop {
            // Increment spinner for animation
            if self.app.loading {
                self.app.spinner_frame = self.app.spinner_frame.wrapping_add(1);
            }
            
            // Draw UI
            terminal.draw(|f| {
                if self.show_command_menu {
                    self.draw_with_command_menu(f);
                } else {
                    ui::draw(f, &self.app);
                }
            }).map_err(|e| e.to_string())?;

            // Check for pending questions from AI - show tabbed form
            if !self.app.pending_questions.is_empty() {
                let questions = std::mem::take(&mut self.app.pending_questions);
                if let Ok(Some(responses)) = self.show_question_form(questions, terminal).await {
                    // Send responses as new message
                    self.app.add_user_message(responses.clone());
                    self.app.loading = true;
                    self.app.scroll = 0;
                    terminal.draw(|f| ui::draw(f, &self.app)).map_err(|e| e.to_string())?;
                    self.send_message_internal(responses).await?;
                }
            }

            // Handle events
            if event::poll(Duration::from_millis(100)).map_err(|e| e.to_string())? {
                if let Event::Key(key) = event::read().map_err(|e| e.to_string())? {
                    if self.show_command_menu {
                        if let Some(action) = self.handle_command_menu_key(key.code) {
                            match action {
                                CommandAction::New => {
                                    // Save current and start fresh
                                    self.save_conversation();
                                    self.app.messages.clear();
                                }
                                CommandAction::Resume => {
                                    // Show resume menu
                                    self.show_resume_menu(terminal).await?;
                                    // Clear terminal and flush events
                                    terminal.clear().map_err(|e| e.to_string())?;
                                    while event::poll(Duration::from_millis(50)).unwrap_or(false) {
                                        let _ = event::read();
                                    }
                                }
                                CommandAction::Save => {
                                    self.save_conversation();
                                }
                                CommandAction::Exit => {
                                    // Save and quit
                                    self.save_conversation();
                                    self.app.should_quit = true;
                                }
                                CommandAction::Memory => {
                                    // Exit TUI temporarily for editor
                                    disable_raw_mode().map_err(|e| e.to_string())?;
                                    execute!(terminal.backend_mut(), LeaveAlternateScreen).map_err(|e| e.to_string())?;
                                    self.open_memory_editor();
                                    enable_raw_mode().map_err(|e| e.to_string())?;
                                    execute!(terminal.backend_mut(), EnterAlternateScreen).map_err(|e| e.to_string())?;
                                    terminal.clear().map_err(|e| e.to_string())?;
                                    // Flush events
                                    while event::poll(Duration::from_millis(10)).unwrap_or(false) {
                                        let _ = event::read();
                                    }
                                }
                                CommandAction::Questions => {
                                    // Demo tabbed form
                                    let questions = vec![
                                        "Quel est le nom du projet?".to_string(),
                                        "Quel langage utilisez-vous?".to_string(),
                                        "Décrivez le problème à résoudre:".to_string(),
                                    ];
                                    if let Ok(Some(response)) = self.show_question_form(questions, terminal).await {
                                        self.app.add_user_message(response);
                                    }
                                }
                                CommandAction::Reindex => {
                                    // Reindex project to SQLite with progress
                                    self.reindex_with_progress(terminal).await?;
                                }
                            }
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.app.should_quit = true;
                            }
                            KeyCode::Esc => {
                                self.app.should_quit = true;
                            }
                            // BackTab (Shift+Tab) or Alt+M cycles mode
                            KeyCode::BackTab => {
                                self.app.cycle_mode();
                            }
                            KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::ALT) => {
                                self.app.cycle_mode();
                            }
                            KeyCode::Char('/') if self.app.input.is_empty() => {
                                self.show_command_menu = true;
                                self.command_filter.clear();
                                self.selected_command = 0;
                            }
                            KeyCode::Enter => {
                                if !self.app.input.is_empty() {
                                    // Store input and clear immediately for visual feedback
                                    let input = self.app.input.clone();
                                    self.app.input.clear();
                                    self.app.cursor_pos = 0;
                                    self.app.add_user_message(input.clone());
                                    self.app.loading = true;
                                    self.app.scroll = 0; // Scroll to bottom
                                    
                                    // Redraw immediately to show user message + thinking indicator
                                    terminal.draw(|f| ui::draw(f, &self.app)).map_err(|e| e.to_string())?;
                                    
                                    // Now send to API (this will block but user sees their message)
                                    self.send_message_internal(input).await?;
                                }
                            }
                            KeyCode::Char(c) => {
                                self.app.insert_char(c);
                            }
                            KeyCode::Backspace => {
                                self.app.delete_char();
                            }
                            KeyCode::Left => {
                                self.app.move_cursor_left();
                            }
                            KeyCode::Right => {
                                self.app.move_cursor_right();
                            }
                            KeyCode::Up => {
                                self.app.history_up();
                            }
                            KeyCode::Down => {
                                self.app.history_down();
                            }
                            KeyCode::PageUp => {
                                for _ in 0..5 { self.app.scroll_up(); }
                            }
                            KeyCode::PageDown => {
                                for _ in 0..5 { self.app.scroll_down(); }
                            }
                            _ => {}
                        }
                    }
                }
            }

            if self.app.should_quit {
                break;
            }
        }

        Ok(())
    }

    fn handle_command_menu_key(&mut self, key: KeyCode) -> Option<CommandAction> {
        match key {
            KeyCode::Esc => {
                self.show_command_menu = false;
                None
            }
            KeyCode::Enter => {
                let action = self.execute_selected_command();
                self.show_command_menu = false;
                action
            }
            KeyCode::Up => {
                if self.selected_command > 0 {
                    self.selected_command -= 1;
                }
                None
            }
            KeyCode::Down => {
                let filtered = self.filtered_commands();
                if self.selected_command < filtered.len().saturating_sub(1) {
                    self.selected_command += 1;
                }
                None
            }
            KeyCode::Char(c) => {
                self.command_filter.push(c);
                self.selected_command = 0;
                None
            }
            KeyCode::Backspace => {
                self.command_filter.pop();
                self.selected_command = 0;
                None
            }
            _ => None
        }
    }

    fn filtered_commands(&self) -> Vec<(&str, &str)> {
        COMMANDS.iter()
            .filter(|(cmd, _)| cmd.contains(&self.command_filter.as_str()))
            .cloned()
            .collect()
    }

    fn execute_selected_command(&mut self) -> Option<CommandAction> {
        let filtered = self.filtered_commands();
        let action = if let Some((cmd, _)) = filtered.get(self.selected_command) {
            match *cmd {
                "quit" => {
                    self.app.should_quit = true;
                    None
                }
                "clear" => {
                    self.app.messages.clear();
                    None
                }
                "new" => Some(CommandAction::New),
                "resume" => Some(CommandAction::Resume),
                "save" => Some(CommandAction::Save),
                "memory" => Some(CommandAction::Memory),
                "questions" => Some(CommandAction::Questions),
                "exit" => Some(CommandAction::Exit),
                "reindex" => Some(CommandAction::Reindex),
                "ask" => { self.app.mode = ChatMode::Ask; None }
                "plan" => { self.app.mode = ChatMode::Plan; None }
                "code" => { self.app.mode = ChatMode::Code; None }
                "auto" => { self.app.mode = ChatMode::Auto; None }
                _ => None
            }
        } else {
            None
        };
        self.command_filter.clear();
        action
    }

    fn save_conversation(&self) {
        use crate::chat_storage::{ChatStorage, SavedChat};
        
        if let Ok(storage) = ChatStorage::new() {
            let mut chat = SavedChat::new(&self.app.project_path.to_string_lossy());
            for msg in &self.app.messages {
                chat.messages.push(crate::mistral_client::Message {
                    role: msg.role.clone(),
                    content: msg.content.clone(),
                });
            }
            chat.auto_title();
            let _ = storage.save(&chat);
        }
    }

    fn reindex_to_sqlite(&mut self) -> usize {
        use walkdir::WalkDir;
        
        // Recreate persistent index
        let project_path = self.app.project_path.clone();
        self.persistent_index = PersistentIndex::open(&project_path).ok();
        
        let Some(ref pindex) = self.persistent_index else {
            return 0;
        };
        
        let extensions = ["rs", "py", "js", "ts", "tsx", "jsx", "go", "java", "c", "cpp", "h", "hpp", 
                          "php", "rb", "swift", "kt", "scala", "vue", "svelte", "html", "css", "scss",
                          "json", "yaml", "yml", "toml", "md", "sql"];
        let mut count = 0;
        
        for entry in WalkDir::new(&project_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            
            // Skip hidden directories and common exclusions
            if path.components().any(|c| {
                let s = c.as_os_str().to_string_lossy();
                s.starts_with('.') || s == "node_modules" || s == "target" || s == "dist" || s == "build"
            }) {
                continue;
            }
            
            // Check extension
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !extensions.contains(&ext) {
                continue;
            }
            
            // Read and index
            if let Ok(content) = std::fs::read_to_string(path) {
                let relative = path.strip_prefix(&project_path)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| path.to_string_lossy().to_string());
                
                if pindex.index_file(path, &relative, &content).is_ok() {
                    count += 1;
                }
            }
        }
        
        count
    }

    /// Reindex with TUI progress bar
    async fn reindex_with_progress(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), String> {
        use walkdir::WalkDir;
        use ratatui::layout::{Constraint, Direction, Layout, Rect};
        use ratatui::style::{Color, Style};
        use ratatui::widgets::{Block, Borders, Clear, Gauge, Paragraph};
        
        // Recreate persistent index
        let project_path = self.app.project_path.clone();
        self.persistent_index = PersistentIndex::open(&project_path).ok();
        
        let Some(ref pindex) = self.persistent_index else {
            self.app.add_ai_message("❌ Impossible d'ouvrir l'index SQLite.".to_string());
            return Ok(());
        };
        
        let extensions = ["rs", "py", "js", "ts", "tsx", "jsx", "go", "java", "c", "cpp", "h", "hpp", 
                          "php", "rb", "swift", "kt", "scala", "vue", "svelte", "html", "css", "scss",
                          "json", "yaml", "yml", "toml", "md", "sql"];
        
        // First pass: count files to index
        let files_to_index: Vec<_> = WalkDir::new(&project_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                let path = e.path();
                // Skip exclusions
                !path.components().any(|c| {
                    let s = c.as_os_str().to_string_lossy();
                    s.starts_with('.') || s == "node_modules" || s == "target" || s == "dist" || s == "build"
                })
            })
            .filter(|e| {
                let ext = e.path().extension().and_then(|e| e.to_str()).unwrap_or("");
                extensions.contains(&ext)
            })
            .collect();
        
        let total = files_to_index.len();
        let mut indexed = 0;
        
        for (i, entry) in files_to_index.iter().enumerate() {
            let path = entry.path();
            let relative = path.strip_prefix(&project_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string_lossy().to_string());
            
            // Draw progress bar
            terminal.draw(|frame| {
                ui::draw(frame, &self.app);
                
                let area = frame.area();
                let progress_width = 60.min(area.width.saturating_sub(4));
                let progress_height = 5;
                let progress_area = Rect {
                    x: (area.width - progress_width) / 2,
                    y: (area.height - progress_height) / 2,
                    width: progress_width,
                    height: progress_height,
                };
                
                frame.render_widget(Clear, progress_area);
                
                let block = Block::default()
                    .title(" Réindexation ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan));
                let inner = block.inner(progress_area);
                frame.render_widget(block, progress_area);
                
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Length(1)])
                    .split(inner);
                
                let ratio = if total > 0 { (i + 1) as f64 / total as f64 } else { 1.0 };
                let gauge = Gauge::default()
                    .ratio(ratio)
                    .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
                    .label(format!("{}/{}", i + 1, total));
                frame.render_widget(gauge, chunks[0]);
                
                let file_display = if relative.len() > (progress_width as usize - 4) {
                    format!("...{}", &relative[relative.len().saturating_sub(progress_width as usize - 7)..])
                } else {
                    relative.clone()
                };
                let file_label = Paragraph::new(file_display);
                frame.render_widget(file_label, chunks[1]);
            }).map_err(|e| e.to_string())?;
            
            // Index the file
            if let Ok(content) = std::fs::read_to_string(path) {
                if pindex.index_file(path, &relative, &content).is_ok() {
                    indexed += 1;
                }
            }
        }
        
        // Refresh system prompt
        self.refresh_system_prompt();
        
        self.app.add_ai_message(format!("✅ {} fichiers indexés dans SQLite.", indexed));
        Ok(())
    }

    /// Incremental reindex: only update files that have changed (hash mismatch)
    fn incremental_reindex(&mut self) -> usize {
        use walkdir::WalkDir;
        
        let Some(ref pindex) = self.persistent_index else {
            return 0;
        };
        
        let project_path = self.app.project_path.clone();
        let extensions = ["rs", "py", "js", "ts", "tsx", "jsx", "go", "java", "c", "cpp", "h", "hpp", 
                          "php", "rb", "swift", "kt", "scala", "vue", "svelte", "html", "css", "scss",
                          "json", "yaml", "yml", "toml", "md", "sql"];
        let mut updated = 0;
        
        for entry in WalkDir::new(&project_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            
            // Skip exclusions
            if path.components().any(|c| {
                let s = c.as_os_str().to_string_lossy();
                s.starts_with('.') || s == "node_modules" || s == "target" || s == "dist" || s == "build"
            }) {
                continue;
            }
            
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !extensions.contains(&ext) {
                continue;
            }
            
            if let Ok(content) = std::fs::read_to_string(path) {
                let relative = path.strip_prefix(&project_path)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| path.to_string_lossy().to_string());
                
                // Only reindex if hash changed
                if pindex.needs_reindex(&relative, &content) {
                    if pindex.index_file(path, &relative, &content).is_ok() {
                        updated += 1;
                    }
                }
            }
        }
        
        updated
    }

    /// Refresh system prompt with current SQLite index info
    fn refresh_system_prompt(&mut self) {
        let codebase_context = {
            let index = CodebaseIndex::index(&self.app.project_path, None, &[], 50).ok();
            index.map(|i| i.build_context(20000).first().cloned().unwrap_or_default())
                .unwrap_or_default()
        };
        
        let sqlite_info = if let Some(ref pindex) = self.persistent_index {
            if let Ok(files) = pindex.list_files() {
                let file_list: Vec<String> = files.iter()
                    .take(100)
                    .map(|f| format!("- {} ({})", f.relative_path, f.extension))
                    .collect();
                if !file_list.is_empty() {
                    format!("\n\nINDEX SQLITE ({} fichiers):\n{}", files.len(), file_list.join("\n"))
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        
        self.system_prompt = format!("{}\n\nCODEBASE:\n{}{}", SYSTEM_PROMPT, codebase_context, sqlite_info);
    }

    /// Detect file paths in user input and inject their content from SQLite
    fn inject_file_contents(&self, user_input: &str) -> String {
        let Some(ref pindex) = self.persistent_index else {
            return String::new();
        };
        
        // Get list of indexed files
        let files = match pindex.list_files() {
            Ok(f) => f,
            Err(_) => return String::new(),
        };
        
        let mut injected = Vec::new();
        let input_lower = user_input.to_lowercase();
        
        // Check if user message mentions any indexed file
        for file in &files {
            let filename = file.relative_path.split('/').last().unwrap_or(&file.relative_path);
            let path_lower = file.relative_path.to_lowercase();
            
            // Check if file is mentioned (by full path, partial path, or filename)
            if input_lower.contains(&path_lower) || input_lower.contains(&filename.to_lowercase()) {
                // Retrieve content from SQLite
                if let Ok(Some(content)) = pindex.get_content(&file.relative_path) {
                    // Limit content size (max 5000 chars per file)
                    let truncated = if content.len() > 5000 {
                        format!("{}...\n[Contenu tronqué à 5000 caractères]", &content[..5000])
                    } else {
                        content
                    };
                    injected.push(format!(
                        "📁 FICHIER DEMANDÉ: {}\n```{}\n{}\n```",
                        file.relative_path,
                        file.extension,
                        truncated
                    ));
                }
            }
        }
        
        if injected.is_empty() {
            String::new()
        } else {
            format!("Voici le contenu des fichiers mentionnés:\n\n{}", injected.join("\n\n"))
        }
    }

    fn open_memory_editor(&mut self) {
        use std::process::Command;
        use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
        use crossterm::execute;
        use crossterm::terminal::LeaveAlternateScreen;
        
        // Create directory if needed
        if let Some(parent) = self.memory_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        
        // Create file with template if it doesn't exist
        if !self.memory_file.exists() {
            let template = r#"# Instructions Projet

Ces instructions sont lues avec chaque prompt pour ce projet.
Écrivez ici les règles, conventions, et contexte spécifique au projet.

## Exemple:
- Toujours utiliser TypeScript strict
- Préférer les composants fonctionnels React
- Utiliser Tailwind pour le CSS
"#;
            let _ = std::fs::write(&self.memory_file, template);
        }
        
        // Open editor (try vim, then nano, then vi)
        // Terminal state is managed by caller
        let editors = ["vim", "nvim", "nano", "vi"];
        for editor in editors {
            if Command::new(editor)
                .arg(&self.memory_file)
                .status()
                .is_ok()
            {
                break;
            }
        }
        
        // Reload memory
        if let Ok(content) = std::fs::read_to_string(&self.memory_file) {
            self.project_memory = content;
        }
    }

    async fn show_resume_menu(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), String> {
        use crate::chat_storage::ChatStorage;
        use ratatui::layout::{Constraint, Direction, Layout, Rect};
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::{Block, Borders, Clear, List, ListItem};
        
        let storage = ChatStorage::new()?;
        let chats = storage.list()?;
        
        if chats.is_empty() {
            self.app.add_ai_message("📭 Aucune conversation sauvegardée".to_string());
            return Ok(());
        }
        
        let mut selected: usize = 0;
        
        loop {
            terminal.draw(|frame| {
                // Draw normal UI
                ui::draw(frame, &self.app);
                
                // Draw overlay menu
                let area = frame.area();
                let menu_width = 60.min(area.width.saturating_sub(4));
                let menu_height = (chats.len() + 2).min(15) as u16;
                
                let menu_area = Rect {
                    x: (area.width - menu_width) / 2,
                    y: (area.height - menu_height) / 2,
                    width: menu_width,
                    height: menu_height,
                };
                
                frame.render_widget(Clear, menu_area);
                
                let block = Block::default()
                    .title(" Reprendre une conversation ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan));
                
                let inner = block.inner(menu_area);
                frame.render_widget(block, menu_area);
                
                let items: Vec<ListItem> = chats.iter()
                    .enumerate()
                    .map(|(i, chat)| {
                        let style = if i == selected {
                            Style::default().bg(Color::Rgb(60, 60, 100)).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        let prefix = if i == selected { "▶ " } else { "  " };
                        ListItem::new(Line::from(vec![
                            Span::raw(prefix),
                            Span::styled(&chat.title, style),
                            Span::styled(format!(" ({})", chat.time_ago()), Style::default().fg(Color::DarkGray)),
                        ]))
                    })
                    .collect();
                
                let list = List::new(items);
                frame.render_widget(list, inner);
            }).map_err(|e| e.to_string())?;
            
            if event::poll(Duration::from_millis(100)).map_err(|e| e.to_string())? {
                if let Event::Key(key) = event::read().map_err(|e| e.to_string())? {
                    match key.code {
                        KeyCode::Esc => break,
                        KeyCode::Up => {
                            if selected > 0 {
                                selected -= 1;
                            }
                        }
                        KeyCode::Down => {
                            if selected < chats.len().saturating_sub(1) {
                                selected += 1;
                            }
                        }
                        KeyCode::Enter => {
                            // Load selected chat
                            if let Some(chat) = chats.get(selected) {
                                self.app.messages.clear();
                                for msg in &chat.messages {
                                    self.app.messages.push(crate::tui::app::ChatMessage {
                                        role: msg.role.clone(),
                                        content: msg.content.clone(),
                                        is_user: msg.role == "user",
                                    });
                                }
                                // Reset app state after loading
                                self.app.scroll = 0;
                                self.app.loading = false;
                                self.app.input.clear();
                                self.app.cursor_pos = 0;
                                // Recalculate tokens
                                self.app.tokens = self.app.messages.iter()
                                    .map(|m| m.content.len() / 4)
                                    .sum();
                                
                                // Add UI message for the user (context is understood from history)
                                self.app.add_ai_message("📜 Conversation reprise. L'historique a été chargé.".to_string());
                            }
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
        
        // Ensure command menu is closed
        self.show_command_menu = false;
        
        Ok(())
    }
}

enum CommandAction {
    New,
    Resume,
    Save,
    Memory,
    Questions,
    Exit,
    Reindex,
}

/// Multi-question form with Tab navigation and optional choices
pub struct QuestionForm {
    pub questions: Vec<String>,
    pub choices: Vec<Vec<String>>,  // Optional choices for each question
    pub selected_choice: Vec<Option<usize>>,  // Selected choice index (None = custom input mode)
    pub answers: Vec<String>,
    pub current_field: usize,
    pub cursor_pos: usize,
    pub in_choice_mode: bool,  // true = selecting from choices, false = typing custom
}

impl QuestionForm {
    pub fn new(questions: Vec<String>) -> Self {
        let count = questions.len();
        Self {
            questions,
            choices: vec![Vec::new(); count],
            selected_choice: vec![None; count],
            answers: vec![String::new(); count],
            current_field: 0,
            cursor_pos: 0,
            in_choice_mode: false,
        }
    }

    pub fn with_choices(questions: Vec<String>, choices: Vec<Vec<String>>) -> Self {
        let count = questions.len();
        Self {
            questions,
            choices: choices.into_iter().chain(std::iter::repeat(Vec::new())).take(count).collect(),
            selected_choice: vec![None; count],
            answers: vec![String::new(); count],
            current_field: 0,
            cursor_pos: 0,
            in_choice_mode: true,  // Start in choice mode if choices available
        }
    }

    pub fn current_choices(&self) -> &[String] {
        &self.choices[self.current_field]
    }

    pub fn has_choices(&self) -> bool {
        !self.choices[self.current_field].is_empty()
    }

    pub fn next_field(&mut self) {
        if self.current_field < self.questions.len() - 1 {
            self.current_field += 1;
            self.cursor_pos = self.answers[self.current_field].chars().count();
            self.in_choice_mode = self.has_choices() && self.selected_choice[self.current_field].is_some();
        }
    }

    pub fn prev_field(&mut self) {
        if self.current_field > 0 {
            self.current_field -= 1;
            self.cursor_pos = self.answers[self.current_field].chars().count();
            self.in_choice_mode = self.has_choices() && self.selected_choice[self.current_field].is_some();
        }
    }

    pub fn select_choice_up(&mut self) {
        if !self.has_choices() { return; }
        self.in_choice_mode = true;
        let choices = &self.choices[self.current_field];
        match self.selected_choice[self.current_field] {
            None => self.selected_choice[self.current_field] = Some(0),
            Some(i) if i > 0 => self.selected_choice[self.current_field] = Some(i - 1),
            _ => {}
        }
        // Update answer to selected choice
        if let Some(i) = self.selected_choice[self.current_field] {
            self.answers[self.current_field] = choices[i].clone();
        }
    }

    pub fn select_choice_down(&mut self) {
        if !self.has_choices() { return; }
        self.in_choice_mode = true;
        let choices = &self.choices[self.current_field];
        match self.selected_choice[self.current_field] {
            None => self.selected_choice[self.current_field] = Some(0),
            Some(i) if i < choices.len() - 1 => self.selected_choice[self.current_field] = Some(i + 1),
            _ => {}
        }
        // Update answer to selected choice
        if let Some(i) = self.selected_choice[self.current_field] {
            self.answers[self.current_field] = choices[i].clone();
        }
    }

    pub fn insert_char(&mut self, c: char) {
        // Switch to custom input mode
        self.in_choice_mode = false;
        self.selected_choice[self.current_field] = None;
        
        let answer = &mut self.answers[self.current_field];
        let byte_pos = answer.char_indices()
            .nth(self.cursor_pos)
            .map(|(i, _)| i)
            .unwrap_or(answer.len());
        answer.insert(byte_pos, c);
        self.cursor_pos += 1;
    }

    pub fn delete_char(&mut self) {
        if self.cursor_pos > 0 {
            self.in_choice_mode = false;
            self.selected_choice[self.current_field] = None;
            
            self.cursor_pos -= 1;
            let answer = &mut self.answers[self.current_field];
            let byte_pos = answer.char_indices()
                .nth(self.cursor_pos)
                .map(|(i, _)| i)
                .unwrap_or(0);
            answer.remove(byte_pos);
        }
    }

    pub fn format_responses(&self) -> String {
        self.questions.iter()
            .zip(self.answers.iter())
            .map(|(q, a)| format!("**{}**\n{}", q, a))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

impl TuiRunner {
    /// Show a tabbed form for multiple questions
    pub async fn show_question_form(&mut self, questions: Vec<String>, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<Option<String>, String> {
        use ratatui::layout::{Constraint, Direction, Layout, Rect};
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::{Block, Borders, Clear, Paragraph};

        let mut form = QuestionForm::new(questions);

        loop {
            terminal.draw(|frame| {
                // Draw normal UI behind
                ui::draw(frame, &self.app);

                // Draw form overlay
                let area = frame.area();
                let form_width = 70.min(area.width.saturating_sub(4));
                let form_height = ((form.questions.len() * 3) + 4).min(20) as u16;

                let form_area = Rect {
                    x: (area.width - form_width) / 2,
                    y: (area.height - form_height) / 2,
                    width: form_width,
                    height: form_height,
                };

                frame.render_widget(Clear, form_area);

                let block = Block::default()
                    .title(" Questions (Tab: suivant, Shift+Tab: précédent, Enter: valider) ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan));

                let inner = block.inner(form_area);
                frame.render_widget(block, form_area);

                // Draw each question/answer field
                let field_height = if form.has_choices() { 5u16 } else { 3u16 };
                for (i, (question, answer)) in form.questions.iter().zip(form.answers.iter()).enumerate() {
                    let y = inner.y + (i as u16 * field_height);
                    if y + field_height > inner.y + inner.height {
                        break;
                    }

                    let is_current = i == form.current_field;
                    let border_color = if is_current { Color::Yellow } else { Color::DarkGray };
                    let choices = &form.choices[i];
                    let has_choices = !choices.is_empty();

                    let field_area = Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: field_height,
                    };

                    let title_suffix = if has_choices { " (↑↓: choix, ou tapez)" } else { "" };
                    let field_block = Block::default()
                        .title(format!(" {} ({}/{}){}  ", question, i + 1, form.questions.len(), title_suffix))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(border_color));

                    let field_inner = field_block.inner(field_area);
                    frame.render_widget(field_block, field_area);

                    // Draw answer with cursor if current (UTF-8 safe)
                    let chars: Vec<char> = answer.chars().collect();
                    let text = if is_current {
                        let before: String = chars[..form.cursor_pos.min(chars.len())].iter().collect();
                        let cursor_char: String = chars.get(form.cursor_pos).map(|c| c.to_string()).unwrap_or_else(|| " ".to_string());
                        let after: String = if form.cursor_pos + 1 < chars.len() {
                            chars[form.cursor_pos + 1..].iter().collect()
                        } else {
                            String::new()
                        };
                        Line::from(vec![
                            Span::raw(before),
                            Span::styled(cursor_char, Style::default().bg(Color::White).fg(Color::Black)),
                            Span::raw(after),
                        ])
                    } else {
                        Line::from(answer.as_str())
                    };

                    let para = Paragraph::new(text);
                    frame.render_widget(para, field_inner);

                    // Draw choices below if available
                    if has_choices && is_current {
                        let choices_text: String = choices.iter().enumerate()
                            .map(|(idx, c)| {
                                let prefix = if form.selected_choice[i] == Some(idx) { "▶ " } else { "  " };
                                format!("{}{}", prefix, c)
                            })
                            .collect::<Vec<_>>()
                            .join(" │ ");
                        let choices_line = Line::from(Span::styled(choices_text, Style::default().fg(Color::DarkGray)));
                        let choices_area = Rect {
                            x: field_inner.x,
                            y: field_inner.y + 1,
                            width: field_inner.width,
                            height: 1,
                        };
                        let choices_para = Paragraph::new(choices_line);
                        frame.render_widget(choices_para, choices_area);
                    }
                }
            }).map_err(|e| e.to_string())?;

            if event::poll(Duration::from_millis(100)).map_err(|e| e.to_string())? {
                if let Event::Key(key) = event::read().map_err(|e| e.to_string())? {
                    match key.code {
                        KeyCode::Esc => return Ok(None),
                        KeyCode::Enter => {
                            // Submit all answers
                            return Ok(Some(form.format_responses()));
                        }
                        KeyCode::Tab => form.next_field(),
                        KeyCode::BackTab => form.prev_field(),
                        KeyCode::Up => form.select_choice_up(),
                        KeyCode::Down => form.select_choice_down(),
                        KeyCode::Char(c) => form.insert_char(c),
                        KeyCode::Backspace => form.delete_char(),
                        KeyCode::Left => {
                            if form.cursor_pos > 0 {
                                form.cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if form.cursor_pos < form.answers[form.current_field].chars().count() {
                                form.cursor_pos += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

impl TuiRunner {
    fn draw_with_command_menu(&self, frame: &mut ratatui::Frame) {
        use ratatui::layout::{Constraint, Direction, Layout, Rect};
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};

        // Draw normal UI first
        ui::draw(frame, &self.app);

        // Draw command menu overlay
        let area = frame.area();
        let menu_width = 40.min(area.width.saturating_sub(4));
        let menu_height = 12.min(area.height.saturating_sub(4));
        
        let menu_area = Rect {
            x: (area.width - menu_width) / 2,
            y: (area.height - menu_height) / 2,
            width: menu_width,
            height: menu_height,
        };

        // Clear background
        frame.render_widget(Clear, menu_area);

        let block = Block::default()
            .title(" Commandes ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(menu_area);
        frame.render_widget(block, menu_area);

        // Filter input
        let filter_line = Line::from(vec![
            Span::raw("/"),
            Span::styled(&self.command_filter, Style::default().fg(Color::Yellow)),
            Span::styled("_", Style::default().bg(Color::White)),
        ]);
        let filter_para = Paragraph::new(filter_line);
        
        let menu_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner);
        
        frame.render_widget(filter_para, menu_layout[0]);

        // Command list with scroll
        let filtered = self.filtered_commands();
        let visible_height = menu_layout[1].height as usize;
        
        // Calculate scroll offset to keep selected item visible
        let scroll_offset = if self.selected_command >= visible_height {
            self.selected_command - visible_height + 1
        } else {
            0
        };
        
        let items: Vec<ListItem> = filtered.iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_height)
            .map(|(i, (cmd, desc))| {
                let style = if i == self.selected_command {
                    Style::default().bg(Color::Rgb(60, 60, 100)).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let prefix = if i == self.selected_command { "▶ " } else { "  " };
                ListItem::new(Line::from(vec![
                    Span::raw(prefix),
                    Span::styled(format!("/{:<10}", cmd), Style::default().fg(Color::Cyan)),
                    Span::styled(format!(" {}", desc), style),
                ]))
            })
            .collect();

        let list = List::new(items);
        frame.render_widget(list, menu_layout[1]);
    }

    /// Internal method called after user message is already added and displayed
    async fn send_message_internal(&mut self, input: String) -> Result<(), String> {
        // Detect file contents from SQLite if user mentions files (will be added to system prompt)
        let file_context = self.inject_file_contents(&input);
        
        // AUTO mode loop - continue until [TERMINÉ] or user cancels
        loop {
            self.app.loading = true;
            
            // Check if we need to compact context
            if self.app.tokens > COMPACT_THRESHOLD {
                self.compact_context().await?;
            }

            // Build messages with project memory and file context
            let mut base_prompt = if !self.project_memory.is_empty() {
                format!("{}\n\nPROJECT MEMORY:\n{}", self.system_prompt, self.project_memory)
            } else {
                self.system_prompt.clone()
            };
            
            // Add file context if any files were mentioned
            if !file_context.is_empty() {
                base_prompt = format!("{}\n\n{}", base_prompt, file_context);
            }
            
            let mut messages = vec![Message {
                role: "system".to_string(),
                content: if self.app.mode == ChatMode::Auto {
                    format!("{}{}", base_prompt, AUTO_MODE_SUFFIX)
                } else {
                    base_prompt
                },
            }];
            messages.extend(self.app.to_api_messages());

            // Send to API with retry
            let mut last_error = String::new();
            let mut api_response: Option<String> = None;
            
            for attempt in 0..4 {
                if attempt > 0 {
                    // Exponential backoff: 1s, 2s, 4s
                    let delay = std::time::Duration::from_secs(1 << (attempt - 1));
                    tokio::time::sleep(delay).await;
                }
                
                match self.client.chat(messages.clone()).await {
                    Ok(response) => {
                        api_response = Some(response);
                        break;
                    }
                    Err(e) => {
                        last_error = e.to_string();
                        // Continue to retry
                    }
                }
            }
            
            match api_response {
                Some(response) => {
                    self.app.loading = false;
                    
                    // Parse tool calls from response
                    let tool_calls = tools::parse_tool_calls(&response);
                    
                    // If there are tool calls, execute them
                    if !tool_calls.is_empty() {
                        let mut tool_results = Vec::new();
                        let mut has_dangerous = false;
                        let mut dangerous_commands: Vec<String> = Vec::new();
                        
                        for tool_call in &tool_calls {
                            // Check if it's an MCP tool (starts with mcp_)
                            if tool_call.name.starts_with("mcp_") {
                                // Parse: mcp_servername_toolname
                                let parts: Vec<&str> = tool_call.name.strip_prefix("mcp_").unwrap_or("").splitn(2, '_').collect();
                                if parts.len() == 2 {
                                    let server_name = parts[0];
                                    let mcp_tool_name = parts[1];
                                    
                                    // Convert params to JSON Value
                                    let args = serde_json::json!(tool_call.params);
                                    
                                    match self.mcp_manager.call_tool(server_name, mcp_tool_name, args) {
                                        Ok(output) => {
                                            tool_results.push(format!(
                                                "<tool_result>\n<name>{}</name>\n<success>true</success>\n<output>\n{}\n</output>\n</tool_result>",
                                                tool_call.name, output
                                            ));
                                        }
                                        Err(e) => {
                                            tool_results.push(format!(
                                                "<tool_result>\n<name>{}</name>\n<success>false</success>\n<output>\n{}\n</output>\n</tool_result>",
                                                tool_call.name, e
                                            ));
                                        }
                                    }
                                }
                            } else {
                                // Regular local tool
                                let result = tools::execute_tool(tool_call, &self.app.project_path);
                                
                                if result.needs_confirmation {
                                    has_dangerous = true;
                                    if let Some(cmd) = tool_call.params.get("command") {
                                        dangerous_commands.push(cmd.clone());
                                    }
                                } else {
                                    tool_results.push(tools::format_tool_result(&result));
                                }
                            }
                        }
                        
                        // Show response with tool calls to user
                        self.app.add_ai_message(response.clone());
                        self.app.scroll = 0;
                        
                        // If we have results, add them and continue the loop
                        if !tool_results.is_empty() {
                            let results_message = tool_results.join("\n\n");
                            self.app.add_user_message(format!("Résultats des outils:\n{}", results_message));
                            // Continue loop to let AI process results
                            continue;
                        }
                        
                        // If dangerous commands, show warning (user must manually respond)
                        if has_dangerous {
                            self.app.add_ai_message(format!(
                                "⚠️ Commandes dangereuses détectées. Tapez 'oui' pour confirmer l'exécution de:\n{}",
                                dangerous_commands.join("\n")
                            ));
                            break;
                        }
                    }
                    
                    // Parse and apply changes if applicable
                    let changes = parse_ai_response(&response, &self.app.project_path);
                    
                    if !changes.is_empty() && self.app.mode != ChatMode::Ask {
                        // In AUTO or CODE mode with confirmation
                        if self.app.mode == ChatMode::Auto {
                            for change in &changes.modifications {
                                let _ = change.apply();
                            }
                            for new_file in &changes.new_files {
                                let _ = new_file.apply();
                            }
                        }
                    }
                    
                    self.app.add_ai_message(response.clone());
                    self.app.scroll = 0;
                    
                    // Detect questions in response (lines ending with ?)
                    let detected_questions: Vec<String> = response
                        .lines()
                        .filter(|line| {
                            let trimmed = line.trim();
                            trimmed.ends_with('?') && trimmed.len() > 10
                        })
                        .map(|line| line.trim().to_string())
                        .collect();
                    
                    if !detected_questions.is_empty() {
                        self.app.pending_questions = detected_questions;
                    }
                    
                    // In AUTO mode, check if we should continue
                    if self.app.mode == ChatMode::Auto {
                        if response.contains("[TERMINÉ]") || response.contains("[TERMINE]") {
                            // Task complete
                            break;
                        } else if response.contains("[CONTINUE]") {
                            // Continue automatically - add a "continue" message
                            self.app.add_user_message("Continue.".to_string());
                            // Don't break, loop again
                        } else {
                            // No marker, assume done
                            break;
                        }
                    } else {
                        // Not in AUTO mode, single response
                        break;
                    }
                }
                None => {
                    self.app.loading = false;
                    self.app.add_ai_message(format!("Erreur après 4 tentatives: {}", last_error));
                    break;
                }
            }
        }

        Ok(())
    }

    async fn compact_context(&mut self) -> Result<(), String> {
        // Pop the last message (current user input) to preserve it
        let last_message = self.app.messages.pop();
        
        // Get all remaining messages except system for summary
        let history: String = self.app.messages.iter()
            .map(|m| format!("{}: {}", if m.is_user { "User" } else { "AI" }, m.content))
            .collect::<Vec<_>>()
            .join("\n");
        
        // Ask AI to summarize
        let compact_messages = vec![
            Message {
                role: "system".to_string(),
                content: COMPACT_PROMPT.to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!("Historique à résumer:\n{}", history),
            },
        ];
        
        if let Ok(summary) = self.client.chat(compact_messages).await {
            self.app.messages.clear();
            self.app.messages.push(crate::tui::app::ChatMessage {
                role: "assistant".to_string(),
                content: format!("📝 Contexte compacté:\n{}", summary),
                is_user: false,
            });
            
            // Restore the last message if it existed
            if let Some(msg) = last_message {
                self.app.messages.push(msg);
            }
            
            // Recalculate tokens
            self.app.tokens = self.app.messages.iter()
                .map(|m| m.content.len() / 4)
                .sum();
            
            // Force scroll to bottom to show new context/user message
            self.app.scroll = 0;
        }
        
        Ok(())
    }
}

pub async fn run_tui(project_path: PathBuf) -> Result<(), String> {
    let mut runner = TuiRunner::new(project_path)?;
    runner.run().await
}
