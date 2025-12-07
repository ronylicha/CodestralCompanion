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
    // Try to read from the store file in the app data directory
    let config_dir = dirs::config_dir()
        .ok_or("Cannot find config directory")?
        .join("com.rony.companion-chat");
    
    let settings_path = config_dir.join("settings.json");
    
    if !settings_path.exists() {
        return Err("Paramètres API non configurés. Lancez d'abord l'application en mode GUI pour configurer votre clé API.".to_string());
    }

    let content = fs::read_to_string(&settings_path)
        .map_err(|e| format!("Cannot read settings: {}", e))?;
    
    let json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid settings format: {}", e))?;

    let config = json.get("config").ok_or("No config found in settings")?;
    
    let api_key = config.get("api_key")
        .and_then(|v| v.as_str())
        .ok_or("API key not found")?
        .to_string();

    if api_key.is_empty() {
        return Err("Clé API vide. Configurez-la dans l'application GUI.".to_string());
    }

    let provider_str = config.get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("MistralAi");

    let provider = match provider_str {
        "Codestral" => ApiProvider::Codestral,
        _ => ApiProvider::MistralAi,
    };

    Ok((api_key, provider))
}
