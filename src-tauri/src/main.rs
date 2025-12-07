// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use companion_chat_lib::cli::{parse_args, is_cli_mode, AgentConfig};
use companion_chat_lib::agent::{Agent, load_api_settings};
use colored::*;

fn main() {
    let cli = parse_args();
    
    if is_cli_mode(&cli) {
        // CLI Agent Mode
        run_cli_agent(&cli);
    } else {
        // GUI Mode
        companion_chat_lib::run()
    }
}

fn run_cli_agent(cli: &companion_chat_lib::cli::Cli) {
    let config = match AgentConfig::from_cli(cli) {
        Some(c) => c,
        None => {
            eprintln!("{}", "Erreur: Configuration invalide".red());
            std::process::exit(1);
        }
    };

    // Load API settings
    let (api_key, provider) = match load_api_settings() {
        Ok((key, prov)) => (key, prov),
        Err(e) => {
            eprintln!("{} {}", "Erreur:".red().bold(), e);
            eprintln!("{}", "Conseil: Lancez 'companion-chat' sans arguments pour ouvrir le GUI et configurer votre clé API.".yellow());
            std::process::exit(1);
        }
    };

    // Create and run the agent
    let agent = Agent::new(config, api_key, provider);
    
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    
    if let Err(e) = runtime.block_on(agent.run()) {
        eprintln!("\n{} {}", "Erreur:".red().bold(), e);
        std::process::exit(1);
    }
}
