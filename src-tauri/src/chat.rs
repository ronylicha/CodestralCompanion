use crate::cli::ChatConfig;
use crate::indexer::CodebaseIndex;
use crate::differ::{parse_ai_response, confirm};
use crate::mistral_client::{MistralClient, ApiProvider, Message};
use crate::agent::load_api_settings;
use colored::*;
use std::io::{self, Write};

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

/// Max context for Mistral models (approx)
const MAX_CONTEXT_TOKENS: usize = 32000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChatMode {
    Ask,      // Just ask questions, no modifications
    Plan,     // Propose a plan without modifications
    Code,     // Propose code modifications with confirmation
    Auto,     // Apply modifications automatically
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
    tokens_used: usize,
}

impl ChatSession {
    pub fn new(config: ChatConfig, api_key: String, provider: ApiProvider) -> Self {
        Self {
            config,
            client: MistralClient::new(api_key, provider),
            messages: vec![Message {
                role: "system".to_string(),
                content: CHAT_SYSTEM_PROMPT.to_string(),
            }],
            index: None,
            mode: ChatMode::Code, // Default mode
            tokens_used: 0,
        }
    }

    fn estimate_tokens(&self) -> usize {
        self.messages.iter().map(|m| m.content.len() / 4).sum()
    }

    fn print_status_bar(&self) {
        let tokens = self.estimate_tokens();
        let remaining = MAX_CONTEXT_TOKENS.saturating_sub(tokens);
        let mode_color = match self.mode {
            ChatMode::Ask => "ASK".blue(),
            ChatMode::Plan => "PLAN".yellow(),
            ChatMode::Code => "CODE".green(),
            ChatMode::Auto => "AUTO".red(),
        };
        
        println!();
        println!(
            "{}",
            format!(
                "─── Mode: {} │ Tokens: ~{}/{} (~{}% restant) ───",
                mode_color,
                tokens,
                MAX_CONTEXT_TOKENS,
                (remaining * 100) / MAX_CONTEXT_TOKENS
            ).dimmed()
        );
    }

    pub async fn start(&mut self) -> Result<(), String> {
        self.print_header();
        
        // Confirm working directory
        println!("📁 Répertoire de travail: {}", self.config.cwd.display().to_string().cyan());
        print!("\n{} ", "Ce répertoire est-il correct? [O/n]".yellow());
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        
        if input.trim().to_lowercase() == "n" || input.trim().to_lowercase() == "non" {
            println!("\n{}", "Spécifiez le répertoire avec: companion-chat chat -c /chemin/vers/projet".dimmed());
            return Ok(());
        }

        // Index the codebase
        println!("\n{}", "📂 Indexation du projet...".bold());
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

        // Add codebase context to system message
        if let Some(idx) = &self.index {
            let context = idx.build_context(20000);
            if let Some(first_chunk) = context.first() {
                self.messages[0].content = format!(
                    "{}\n\nCODEBASE:\n{}",
                    CHAT_SYSTEM_PROMPT,
                    first_chunk
                );
            }
        }

        println!("{}", "─".repeat(60).dimmed());
        println!("{}", "💬 Mode chat interactif. Tapez vos instructions.".green().bold());
        println!("{}", "   Commandes: /quit, /aide, /reindex, /clear".dimmed());
        println!("{}", "─".repeat(60).dimmed());
        println!();

        // REPL loop
        loop {
            print!("{} ", "Vous >".cyan().bold());
            io::stdout().flush().unwrap();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                break;
            }

            let trimmed = input.trim();
            
            if trimmed.is_empty() {
                continue;
            }

            // Handle commands
            match trimmed.to_lowercase().as_str() {
                "/quit" | "/exit" | "/q" => {
                    println!("\n{}", "👋 À bientôt!".green());
                    break;
                }
                "/aide" | "/help" | "/h" => {
                    self.print_help();
                    self.print_status_bar();
                    continue;
                }
                "/ask" => {
                    self.mode = ChatMode::Ask;
                    println!("{}", "Mode ASK activé - Questions/réponses simples".blue());
                    self.print_status_bar();
                    continue;
                }
                "/plan" => {
                    self.mode = ChatMode::Plan;
                    println!("{}", "Mode PLAN activé - Propose des plans sans modifier".yellow());
                    self.print_status_bar();
                    continue;
                }
                "/code" => {
                    self.mode = ChatMode::Code;
                    println!("{}", "Mode CODE activé - Propose des modifications avec confirmation".green());
                    self.print_status_bar();
                    continue;
                }
                "/auto" => {
                    self.mode = ChatMode::Auto;
                    println!("{}", "Mode AUTO activé - Applique les modifications automatiquement".red());
                    self.print_status_bar();
                    continue;
                }
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
                    self.messages.truncate(1); // Keep system message
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
                    // Check if response contains file modifications
                    let changes = parse_ai_response(&response, &self.config.cwd);
                    
                    if !changes.is_empty() && self.mode != ChatMode::Ask {
                        // Show plan if any
                        changes.display_plan();
                        
                        // Show diffs
                        changes.display_all_changes();
                        
                        // Apply based on mode
                        match self.mode {
                            ChatMode::Plan => {
                                println!("\n{}", "(Mode PLAN - aucune modification appliquée)".yellow());
                            }
                            ChatMode::Code => {
                                println!();
                                if confirm("Appliquer ces modifications?") {
                                    self.apply_changes(&changes);
                                } else {
                                    println!("{}", "Modifications ignorées.".yellow());
                                }
                            }
                            ChatMode::Auto => {
                                println!("\n{}", "⚡ Application automatique...".bold());
                                self.apply_changes(&changes);
                            }
                            ChatMode::Ask => {} // Already filtered above
                        }
                    } else {
                        // Just print text response
                        println!("{}", response);
                    }

                    // Add response to history
                    self.messages.push(Message {
                        role: "assistant".to_string(),
                        content: response,
                    });
                }
                Err(e) => {
                    println!("{} {}", "Erreur:".red(), e);
                }
            }
            
            self.print_status_bar();
        }

        Ok(())
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
        println!("{}", "║       🤖 CODESTRAL COMPANION - Mode Chat CLI             ║".cyan());
        println!("{}", "╚══════════════════════════════════════════════════════════╝".cyan());
        println!();
    }

    fn print_help(&self) {
        println!();
        println!("{}", "📚 COMMANDES DISPONIBLES".bold());
        println!("{}", "─".repeat(40).dimmed());
        println!("  {} - Quitter", "/quit".cyan());
        println!("  {} - Afficher cette aide", "/aide".cyan());
        println!("  {} - Réindexer le projet", "/reindex".cyan());
        println!("  {} - Effacer l'historique", "/clear".cyan());
        println!();
        println!("{}", "🔄 MODES (changer avec les commandes)".bold());
        println!("{}", "─".repeat(40).dimmed());
        println!("  {} - Questions/réponses simples", "/ask".blue());
        println!("  {} - Propose des plans sans modifier", "/plan".yellow());
        println!("  {} - Modifications avec confirmation", "/code".green());
        println!("  {} - Applique automatiquement", "/auto".red());
        println!();
    }
}

pub async fn run_chat_session(config: ChatConfig) -> Result<(), String> {
    let (api_key, provider) = load_api_settings()?;
    let mut session = ChatSession::new(config, api_key, provider);
    session.start().await
}
