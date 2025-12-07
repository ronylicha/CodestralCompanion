use crate::cli::ChatConfig;
use crate::indexer::CodebaseIndex;
use crate::differ::{parse_ai_response, confirm};
use crate::mistral_client::{MistralClient, ApiProvider, Message};
use crate::agent::load_api_settings;
use crate::chat_storage::{ChatStorage, SavedChat};
use colored::*;
use std::io::{self, Write};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal;
use chrono::Utc;

const CHAT_SYSTEM_PROMPT: &str = r#"Tu es un assistant de programmation expert intégré dans un terminal. Tu analyses des codebases et proposes des modifications.

RÈGLES IMPORTANTES:
1. Réponds TOUJOURS en français
2. Sois concis et précis
3. Pour proposer des modifications, utilise les balises suivantes:

Pour modifier un fichier existant:
<file path="chemin/relatif/fichier.ext">
<<<<<<< ORIGINAL
code original à remplacer
=======
nouveau code
>>>>>>> MODIFIED
</file>

Pour créer un nouveau fichier:
<new_file path="chemin/nouveau.ext">
contenu
</new_file>

Si tu ne proposes pas de modifications, réponds simplement en texte.
"#;

const MAX_CONTEXT_TOKENS: usize = 32000;
const MODES: [ChatMode; 4] = [ChatMode::Ask, ChatMode::Plan, ChatMode::Code, ChatMode::Auto];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChatMode {
    Ask,
    Plan,
    Code,
    Auto,
}

impl ChatMode {
    fn next(&self) -> ChatMode {
        match self {
            ChatMode::Ask => ChatMode::Plan,
            ChatMode::Plan => ChatMode::Code,
            ChatMode::Code => ChatMode::Auto,
            ChatMode::Auto => ChatMode::Ask,
        }
    }

    fn color_name(&self) -> colored::ColoredString {
        match self {
            ChatMode::Ask => "ASK".blue(),
            ChatMode::Plan => "PLAN".yellow(),
            ChatMode::Code => "CODE".green(),
            ChatMode::Auto => "AUTO".red(),
        }
    }
}

impl std::fmt::Display for ChatMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatMode::Ask => write!(f, "ASK"),
            ChatMode::Plan => write!(f, "PLAN"),
            ChatMode::Code => write!(f, "CODE"),
            ChatMode::Auto => write!(f, "AUTO"),
        }
    }
}

pub struct ChatSession {
    config: ChatConfig,
    client: MistralClient,
    messages: Vec<Message>,
    index: Option<CodebaseIndex>,
    mode: ChatMode,
    storage: ChatStorage,
    current_chat: SavedChat,
}

impl ChatSession {
    pub fn new(config: ChatConfig, api_key: String, provider: ApiProvider) -> Result<Self, String> {
        let storage = ChatStorage::new()?;
        let project_path = config.cwd.to_string_lossy().to_string();
        let current_chat = SavedChat::new(&project_path);
        
        Ok(Self {
            config,
            client: MistralClient::new(api_key, provider),
            messages: vec![Message {
                role: "system".to_string(),
                content: CHAT_SYSTEM_PROMPT.to_string(),
            }],
            index: None,
            mode: ChatMode::Code,
            storage,
            current_chat,
        })
    }

    fn estimate_tokens(&self) -> usize {
        self.messages.iter().map(|m| m.content.len() / 4).sum()
    }

    fn print_status_bar(&self) {
        let tokens = self.estimate_tokens();
        let remaining = MAX_CONTEXT_TOKENS.saturating_sub(tokens);
        
        println!(
            "{}",
            format!(
                "─── {} │ Tokens: ~{}/{} (~{}%) │ Shift+Tab: changer mode ───",
                self.mode.color_name(),
                tokens,
                MAX_CONTEXT_TOKENS,
                (remaining * 100) / MAX_CONTEXT_TOKENS
            ).dimmed()
        );
    }

    fn cycle_mode(&mut self) {
        self.mode = self.mode.next();
        println!("\n{} Mode {} activé", "⚡".bold(), self.mode.color_name());
    }

    fn save_current_chat(&mut self) {
        // Copy messages (skip system prompt)
        self.current_chat.messages = self.messages.iter()
            .filter(|m| m.role != "system")
            .cloned()
            .collect();
        self.current_chat.updated_at = Utc::now();
        self.current_chat.auto_title();
        
        if let Err(e) = self.storage.save(&self.current_chat) {
            eprintln!("{} Erreur sauvegarde: {}", "⚠️".yellow(), e);
        }
    }

    fn new_chat(&mut self) {
        // Save current if has messages
        if self.messages.len() > 1 {
            self.save_current_chat();
        }
        
        let project_path = self.config.cwd.to_string_lossy().to_string();
        self.current_chat = SavedChat::new(&project_path);
        self.messages.truncate(1); // Keep system message
        
        println!("{}", "📝 Nouvelle conversation démarrée".green().bold());
    }

    fn show_resume_list(&self) -> Option<String> {
        let project_path = self.config.cwd.to_string_lossy().to_string();
        match self.storage.list_for_project(&project_path) {
            Ok(chats) if !chats.is_empty() => {
                println!("\n{}", "📋 CONVERSATIONS SAUVEGARDÉES".bold());
                println!("{}", "─".repeat(50).dimmed());
                
                for (i, chat) in chats.iter().take(10).enumerate() {
                    let msg_count = chat.messages.len();
                    println!(
                        "  {} {} {} ({})",
                        format!("[{}]", i + 1).cyan(),
                        chat.title.bold(),
                        chat.time_ago().dimmed(),
                        format!("{} msgs", msg_count).dimmed()
                    );
                }
                
                println!("\n{}", "Entrez le numéro pour reprendre (ou Entrée pour annuler):".dimmed());
                print!("> ");
                io::stdout().flush().unwrap();
                
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                
                if let Ok(num) = input.trim().parse::<usize>() {
                    if num > 0 && num <= chats.len() {
                        return Some(chats[num - 1].id.clone());
                    }
                }
                None
            }
            Ok(_) => {
                println!("{}", "Aucune conversation sauvegardée pour ce projet.".yellow());
                None
            }
            Err(e) => {
                println!("{} {}", "Erreur:".red(), e);
                None
            }
        }
    }

    fn resume_chat(&mut self, id: &str) {
        match self.storage.load(id) {
            Ok(chat) => {
                // Rebuild messages with system prompt
                self.messages = vec![Message {
                    role: "system".to_string(),
                    content: CHAT_SYSTEM_PROMPT.to_string(),
                }];
                self.messages.extend(chat.messages.clone());
                self.current_chat = chat;
                
                println!("{} \"{}\"", "✅ Conversation reprise:".green(), self.current_chat.title);
                
                // Show last 3 messages for context
                let recent: Vec<_> = self.messages.iter().rev().take(4).collect();
                for msg in recent.into_iter().rev() {
                    if msg.role == "user" {
                        println!("  {} {}", "Vous:".cyan(), &msg.content[..msg.content.len().min(60)]);
                    } else if msg.role == "assistant" {
                        let preview = &msg.content[..msg.content.len().min(60)];
                        println!("  {} {}...", "IA:".green(), preview);
                    }
                }
            }
            Err(e) => {
                println!("{} {}", "Erreur:".red(), e);
            }
        }
    }

    pub async fn start(&mut self) -> Result<(), String> {
        self.print_header();
        
        // Confirm working directory
        println!("📁 Répertoire: {}", self.config.cwd.display().to_string().cyan());
        print!("{} ", "Correct? [O/n]".yellow());
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        
        if input.trim().to_lowercase() == "n" {
            println!("{}", "Utilisez: companion-chat chat -c /chemin/projet".dimmed());
            return Ok(());
        }

        // Index the codebase
        println!("\n{}", "📂 Indexation...".bold());
        let ext_refs: Vec<String>;
        let include = if let Some(exts) = &self.config.include_extensions {
            ext_refs = exts.clone();
            Some(ext_refs.as_slice())
        } else {
            None
        };

        self.index = Some(CodebaseIndex::index(
            &self.config.cwd,
            include,
            &self.config.exclude_dirs,
            self.config.max_files,
        )?);

        if let Some(idx) = &self.index {
            println!("{}", idx.summary());
            let context = idx.build_context(20000);
            if let Some(first_chunk) = context.first() {
                self.messages[0].content = format!("{}\n\nCODEBASE:\n{}", CHAT_SYSTEM_PROMPT, first_chunk);
            }
        }

        println!("{}", "─".repeat(60).dimmed());
        println!("{}", "💬 Chat interactif. Tapez /aide pour les commandes.".green().bold());
        self.print_status_bar();

        // REPL loop
        loop {
            print!("\n{} ", "Vous >".cyan().bold());
            io::stdout().flush().unwrap();

            let input = self.read_input_with_shortcuts();
            let trimmed = input.trim();
            
            if trimmed.is_empty() {
                continue;
            }

            // Handle commands
            match trimmed.to_lowercase().as_str() {
                "/quit" | "/exit" | "/q" => {
                    self.save_current_chat();
                    println!("\n{}", "👋 Conversation sauvegardée. À bientôt!".green());
                    break;
                }
                "/aide" | "/help" | "/h" => {
                    self.print_help();
                    self.print_status_bar();
                    continue;
                }
                "/new" => {
                    self.new_chat();
                    self.print_status_bar();
                    continue;
                }
                "/resume" | "/r" => {
                    if let Some(id) = self.show_resume_list() {
                        self.resume_chat(&id);
                    }
                    self.print_status_bar();
                    continue;
                }
                "/ask" => { self.mode = ChatMode::Ask; println!("{}", "Mode ASK".blue()); self.print_status_bar(); continue; }
                "/plan" => { self.mode = ChatMode::Plan; println!("{}", "Mode PLAN".yellow()); self.print_status_bar(); continue; }
                "/code" => { self.mode = ChatMode::Code; println!("{}", "Mode CODE".green()); self.print_status_bar(); continue; }
                "/auto" => { self.mode = ChatMode::Auto; println!("{}", "Mode AUTO".red()); self.print_status_bar(); continue; }
                "/reindex" => {
                    println!("{}", "📂 Réindexation...".bold());
                    let ext_refs: Vec<String>;
                    let include = if let Some(exts) = &self.config.include_extensions {
                        ext_refs = exts.clone();
                        Some(ext_refs.as_slice())
                    } else {
                        None
                    };
                    self.index = Some(CodebaseIndex::index(
                        &self.config.cwd,
                        include,
                        &self.config.exclude_dirs,
                        self.config.max_files,
                    )?);
                    if let Some(idx) = &self.index {
                        println!("{}", idx.summary());
                    }
                    self.print_status_bar();
                    continue;
                }
                "/clear" => {
                    self.messages.truncate(1);
                    println!("{}", "🗑️  Historique effacé.".yellow());
                    self.print_status_bar();
                    continue;
                }
                _ => {}
            }

            // Send to AI
            self.messages.push(Message {
                role: "user".to_string(),
                content: trimmed.to_string(),
            });

            print!("{}", "🤖 ".dimmed());
            io::stdout().flush().unwrap();

            match self.client.chat(self.messages.clone()).await {
                Ok(response) => {
                    let changes = parse_ai_response(&response, &self.config.cwd);
                    
                    if !changes.is_empty() && self.mode != ChatMode::Ask {
                        changes.display_plan();
                        changes.display_all_changes();
                        
                        match self.mode {
                            ChatMode::Plan => {
                                println!("\n{}", "(Mode PLAN - pas de modification)".yellow());
                            }
                            ChatMode::Code => {
                                println!();
                                if confirm("Appliquer?") {
                                    self.apply_changes(&changes);
                                } else {
                                    println!("{}", "Ignoré.".yellow());
                                }
                            }
                            ChatMode::Auto => {
                                println!("\n{}", "⚡ Application...".bold());
                                self.apply_changes(&changes);
                            }
                            ChatMode::Ask => {}
                        }
                    } else {
                        println!("{}", response);
                    }

                    self.messages.push(Message {
                        role: "assistant".to_string(),
                        content: response,
                    });
                    
                    // Auto-save periodically
                    if self.messages.len() % 4 == 0 {
                        self.save_current_chat();
                    }
                }
                Err(e) => {
                    println!("{} {}", "Erreur:".red(), e);
                }
            }
            
            self.print_status_bar();
        }

        Ok(())
    }

    fn read_input_with_shortcuts(&mut self) -> String {
        let mut input = String::new();
        
        // Try to enable raw mode for key detection
        if terminal::enable_raw_mode().is_ok() {
            loop {
                if event::poll(std::time::Duration::from_millis(100)).unwrap_or(false) {
                    if let Ok(Event::Key(key_event)) = event::read() {
                        // Check for Shift+Tab
                        if key_event.code == KeyCode::BackTab || 
                           (key_event.code == KeyCode::Tab && key_event.modifiers.contains(KeyModifiers::SHIFT)) {
                            let _ = terminal::disable_raw_mode();
                            self.cycle_mode();
                            self.print_status_bar();
                            print!("\n{} ", "Vous >".cyan().bold());
                            io::stdout().flush().unwrap();
                            return self.read_input_with_shortcuts();
                        }
                        
                        match key_event.code {
                            KeyCode::Enter => {
                                let _ = terminal::disable_raw_mode();
                                println!();
                                return input;
                            }
                            KeyCode::Char(c) => {
                                input.push(c);
                                print!("{}", c);
                                io::stdout().flush().unwrap();
                            }
                            KeyCode::Backspace => {
                                if !input.is_empty() {
                                    input.pop();
                                    print!("\x08 \x08");
                                    io::stdout().flush().unwrap();
                                }
                            }
                            KeyCode::Esc => {
                                let _ = terminal::disable_raw_mode();
                                return "/quit".to_string();
                            }
                            _ => {}
                        }
                    }
                }
            }
        } else {
            // Fallback to standard input
            io::stdin().read_line(&mut input).unwrap();
            input
        }
    }

    fn apply_changes(&self, changes: &crate::differ::ChangeSet) {
        for change in &changes.modifications {
            if let Err(e) = change.apply() {
                println!("  {} {}", "✗".red(), e);
            } else {
                println!("  {} {}", "✓".green(), change.path);
            }
        }
        for new_file in &changes.new_files {
            if let Err(e) = new_file.apply() {
                println!("  {} {}", "✗".red(), e);
            } else {
                println!("  {} {} (créé)", "✓".green(), new_file.path);
            }
        }
    }

    fn print_header(&self) {
        println!();
        println!("{}", "╔══════════════════════════════════════════════════════════╗".cyan());
        println!("{}", "║       🤖 CODESTRAL COMPANION - Chat CLI                  ║".cyan());
        println!("{}", "╚══════════════════════════════════════════════════════════╝".cyan());
        println!();
    }

    fn print_help(&self) {
        println!();
        println!("{}", "📚 COMMANDES".bold());
        println!("{}", "─".repeat(40).dimmed());
        println!("  {} Quitter   {} Aide", "/quit".cyan(), "/aide".cyan());
        println!("  {} Nouvelle  {} Reprendre", "/new".cyan(), "/resume".cyan());
        println!("  {} Réindexer {} Effacer", "/reindex".cyan(), "/clear".cyan());
        println!();
        println!("{}", "🔄 MODES (Shift+Tab pour cycler)".bold());
        println!("{}", "─".repeat(40).dimmed());
        println!("  {} {} {} {}", "/ask".blue(), "/plan".yellow(), "/code".green(), "/auto".red());
        println!();
    }
}

pub async fn run_chat_session(config: ChatConfig) -> Result<(), String> {
    let (api_key, provider) = load_api_settings()?;
    let mut session = ChatSession::new(config, api_key, provider)?;
    session.start().await
}
