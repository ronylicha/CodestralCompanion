use crate::cli::{AgentConfig, ExecutionMode};
use crate::indexer::CodebaseIndex;
use crate::differ::{parse_ai_response, confirm, ChangeSet};
use crate::mistral_client::{MistralClient, ApiProvider, Message};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;

const SYSTEM_PROMPT: &str = r#"Tu es un assistant de programmation expert. Tu analyses des codebases et proposes des modifications.

RÈGLES IMPORTANTES:
1. Réponds TOUJOURS en français
2. Structure ta réponse avec les balises XML suivantes
3. Sois précis et concis

FORMAT DE RÉPONSE:

<plan>
1. Description de la première étape
2. Description de la deuxième étape
</plan>

Pour modifier un fichier existant:
<file path="chemin/relatif/fichier.ext">
<<<<<<< ORIGINAL
code original à remplacer (exactement comme dans le fichier)
=======
nouveau code qui remplace l'original
>>>>>>> MODIFIED
</file>

Pour créer un nouveau fichier:
<new_file path="chemin/relatif/nouveau_fichier.ext">
contenu complet du nouveau fichier
</new_file>

IMPORTANT: Le code dans ORIGINAL doit correspondre EXACTEMENT au code existant pour que le remplacement fonctionne.
"#;

pub struct Agent {
    config: AgentConfig,
    client: MistralClient,
}

impl Agent {
    pub fn new(config: AgentConfig, api_key: String, provider: ApiProvider) -> Self {
        Self {
            config,
            client: MistralClient::new(api_key, provider),
        }
    }

    pub async fn run(&self) -> Result<(), String> {
        println!("\n{}", "🤖 COMPANION CHAT - Mode Agent".bold().cyan());
        println!("{}", "─".repeat(40).dimmed());
        println!("📁 Projet: {}", self.config.cwd.display());
        println!("📝 Instruction: {}", self.config.instruction.italic());
        println!("⚙️  Mode: {:?}", self.config.mode);
        println!();

        // Phase 1: Index the codebase
        println!("{}", "📂 Indexation du projet...".bold());
        let ext_refs: Vec<String>;
        let include = if let Some(exts) = &self.config.include_extensions {
            ext_refs = exts.clone();
            Some(ext_refs.as_slice())
        } else {
            None
        };

        let index = CodebaseIndex::index(
            &self.config.cwd,
            include,
            &self.config.exclude_dirs,
            self.config.max_files,
        )?;

        println!("{}", index.summary());

        if index.files.is_empty() {
            return Err("Aucun fichier trouvé à analyser".to_string());
        }

        // Phase 2: Build context and send to AI
        println!("{}", "🧠 Analyse en cours...".bold());
        
        let context_chunks = index.build_context(30000); // ~30k tokens max per chunk
        
        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap());
        pb.set_message("Envoi à l'IA...");

        // Build the prompt
        let mut prompt = format!("CODEBASE:\n{}\n\n", context_chunks.first().unwrap_or(&String::new()));
        prompt.push_str(&format!("INSTRUCTION: {}\n", self.config.instruction));
        
        if self.config.mode == ExecutionMode::Plan {
            prompt.push_str("\nNOTE: Mode PLAN uniquement. Propose un plan détaillé sans fournir de modifications de code.");
        }

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: SYSTEM_PROMPT.to_string(),
            },
            Message {
                role: "user".to_string(),
                content: prompt,
            },
        ];

        let response = self.client.chat(messages).await.map_err(|e| e.to_string())?;
        pb.finish_and_clear();

        // Phase 3: Parse and display changes
        let changes = parse_ai_response(&response, &self.config.cwd);
        
        changes.display_plan();

        if self.config.mode == ExecutionMode::Plan {
            println!("{}", "✅ Plan généré (mode plan, aucune modification appliquée)".green());
            return Ok(());
        }

        if changes.is_empty() {
            println!("{}", "ℹ️  Aucune modification de fichier proposée.".yellow());
            return Ok(());
        }

        println!("\n{}", format!("📊 Changements proposés: {}", changes.summary()).bold());
        changes.display_all_changes();

        // Phase 4: Apply changes based on mode
        if self.config.dry_run {
            println!("\n{}", "🔍 Mode dry-run: aucune modification appliquée".yellow());
            return Ok(());
        }

        match self.config.mode {
            ExecutionMode::Auto => {
                self.apply_all_changes(&changes)?;
            }
            ExecutionMode::Interactive => {
                self.apply_changes_interactive(&changes)?;
            }
            ExecutionMode::Plan => unreachable!(),
        }

        Ok(())
    }

    fn apply_all_changes(&self, changes: &ChangeSet) -> Result<(), String> {
        println!("\n{}", "⚡ Application automatique des changements...".bold());
        
        for change in &changes.modifications {
            change.apply()?;
            println!("  {} {}", "✓".green(), change.path);
        }
        
        for new_file in &changes.new_files {
            new_file.apply()?;
            println!("  {} {} (nouveau)", "✓".green(), new_file.path);
        }

        println!("\n{}", "✅ Toutes les modifications ont été appliquées!".green().bold());
        Ok(())
    }

    fn apply_changes_interactive(&self, changes: &ChangeSet) -> Result<(), String> {
        println!();

        for change in &changes.modifications {
            println!("{}", change.display_diff());
            if confirm("Appliquer cette modification?") {
                change.apply()?;
                println!("  {}", "✓ Appliqué".green());
            } else {
                println!("  {}", "✗ Ignoré".yellow());
            }
        }

        for new_file in &changes.new_files {
            println!("{}", new_file.display());
            if confirm("Créer ce fichier?") {
                new_file.apply()?;
                println!("  {}", "✓ Créé".green());
            } else {
                println!("  {}", "✗ Ignoré".yellow());
            }
        }

        println!("\n{}", "✅ Terminé!".green().bold());
        Ok(())
    }
}

/// Load API settings from store
pub fn load_api_settings() -> Result<(String, ApiProvider), String> {
    // tauri-plugin-store saves to data_dir, not config_dir
    let data_dir = dirs::data_dir()
        .ok_or("Cannot find data directory")?
        .join("com.rony.companion-chat");
    
    let settings_path = data_dir.join("settings.json");
    
    // Try to load existing settings
    if settings_path.exists() {
        if let Ok(content) = fs::read_to_string(&settings_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(config) = json.get("config") {
                    let api_key = config.get("api_key")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    
                    if !api_key.is_empty() {
                        let provider_str = config.get("provider")
                            .and_then(|v| v.as_str())
                            .unwrap_or("MistralAi");
                        
                        let provider = match provider_str {
                            "Codestral" => ApiProvider::Codestral,
                            _ => ApiProvider::MistralAi,
                        };
                        
                        return Ok((api_key, provider));
                    }
                }
            }
        }
    }
    
    // No valid API key found - start setup wizard
    setup_api_key_wizard(&data_dir, &settings_path)
}

/// Interactive API key setup wizard
fn setup_api_key_wizard(config_dir: &std::path::Path, settings_path: &std::path::Path) -> Result<(String, ApiProvider), String> {
    use std::io::{self, Write};
    
    println!();
    println!("{}", "╔══════════════════════════════════════════════════════════╗".cyan());
    println!("{}", "║            🔑 Configuration de l'API                     ║".cyan());
    println!("{}", "╚══════════════════════════════════════════════════════════╝".cyan());
    println!();
    
    // Choose provider
    println!("{}", "Choisissez votre endpoint:".bold());
    println!("  {} Mistral AI (api.mistral.ai)", "[1]".cyan());
    println!("  {} Codestral (codestral.mistral.ai)", "[2]".cyan());
    println!();
    
    print!("{} ", "Votre choix [1/2]:".yellow());
    io::stdout().flush().unwrap();
    
    let mut choice = String::new();
    io::stdin().read_line(&mut choice).map_err(|e| e.to_string())?;
    
    let provider = match choice.trim() {
        "2" => {
            println!("{}", "→ Codestral sélectionné".green());
            ApiProvider::Codestral
        }
        _ => {
            println!("{}", "→ Mistral AI sélectionné".green());
            ApiProvider::MistralAi
        }
    };
    
    // Enter API key
    println!();
    println!("{}", "Entrez votre clé API:".bold());
    println!("{}", "(Obtenez-la sur https://console.mistral.ai)".dimmed());
    println!();
    
    print!("{} ", "Clé API:".yellow());
    io::stdout().flush().unwrap();
    
    let mut api_key = String::new();
    io::stdin().read_line(&mut api_key).map_err(|e| e.to_string())?;
    let api_key = api_key.trim().to_string();
    
    if api_key.is_empty() {
        return Err("Clé API vide. Annulé.".to_string());
    }
    
    // Save settings
    fs::create_dir_all(config_dir).map_err(|e| format!("Cannot create config dir: {}", e))?;
    
    let provider_str = match provider {
        ApiProvider::Codestral => "Codestral",
        ApiProvider::MistralAi => "MistralAi",
    };
    
    let settings = serde_json::json!({
        "config": {
            "api_key": api_key,
            "provider": provider_str
        }
    });
    
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Serialize error: {}", e))?;
    
    fs::write(settings_path, json)
        .map_err(|e| format!("Write error: {}", e))?;
    
    println!();
    println!("{}", "✅ Configuration sauvegardée!".green().bold());
    println!();
    
    Ok((api_key, provider))
}

