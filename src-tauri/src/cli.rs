use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "companion-chat")]
#[command(author = "rony")]
#[command(version = "0.1.0")]
#[command(about = "AI-powered coding assistant with GUI and CLI modes")]
pub struct Cli {
    /// Working directory for code analysis
    #[arg(long, short = 'c')]
    pub cwd: Option<PathBuf>,

    /// Instruction for the AI agent
    #[arg(trailing_var_arg = true)]
    pub instruction: Vec<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Plan mode: analyze and propose changes without modifying files
    Plan {
        /// Working directory
        #[arg(long, short = 'c')]
        cwd: PathBuf,
        
        /// Instruction for the AI
        instruction: Vec<String>,
        
        /// File extensions to include (e.g., "rs,ts,py")
        #[arg(long, short = 'e')]
        include: Option<String>,
        
        /// Directories to exclude
        #[arg(long, short = 'x')]
        exclude: Option<Vec<String>>,
        
        /// Maximum files to analyze
        #[arg(long, default_value = "50")]
        max_files: usize,
    },
    
    /// Interactive mode: show diffs and ask for confirmation
    Interactive {
        /// Working directory
        #[arg(long, short = 'c')]
        cwd: PathBuf,
        
        /// Instruction for the AI
        instruction: Vec<String>,
        
        /// File extensions to include
        #[arg(long, short = 'e')]
        include: Option<String>,
        
        /// Directories to exclude
        #[arg(long, short = 'x')]
        exclude: Option<Vec<String>>,
        
        /// Maximum files to analyze
        #[arg(long, default_value = "50")]
        max_files: usize,
    },
    
    /// Auto mode: apply changes immediately after showing diffs
    Auto {
        /// Working directory
        #[arg(long, short = 'c')]
        cwd: PathBuf,
        
        /// Instruction for the AI
        instruction: Vec<String>,
        
        /// File extensions to include
        #[arg(long, short = 'e')]
        include: Option<String>,
        
        /// Directories to exclude
        #[arg(long, short = 'x')]
        exclude: Option<Vec<String>>,
        
        /// Maximum files to analyze
        #[arg(long, default_value = "50")]
        max_files: usize,
        
        /// Dry run - show what would be done without making changes
        #[arg(long)]
        dry_run: bool,
    },
    
    /// Start the GUI application (default if no command given)
    Gui,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExecutionMode {
    Plan,
    Interactive,
    Auto,
}

pub struct AgentConfig {
    pub cwd: PathBuf,
    pub instruction: String,
    pub mode: ExecutionMode,
    pub include_extensions: Option<Vec<String>>,
    pub exclude_dirs: Vec<String>,
    pub max_files: usize,
    pub dry_run: bool,
}

impl AgentConfig {
    pub fn from_cli(cli: &Cli) -> Option<Self> {
        match &cli.command {
            Some(Commands::Plan { cwd, instruction, include, exclude, max_files }) => {
                Some(AgentConfig {
                    cwd: cwd.clone(),
                    instruction: instruction.join(" "),
                    mode: ExecutionMode::Plan,
                    include_extensions: include.as_ref().map(|s| s.split(',').map(|x| x.trim().to_string()).collect()),
                    exclude_dirs: exclude.clone().unwrap_or_default(),
                    max_files: *max_files,
                    dry_run: true, // Plan mode is always dry-run
                })
            }
            Some(Commands::Interactive { cwd, instruction, include, exclude, max_files }) => {
                Some(AgentConfig {
                    cwd: cwd.clone(),
                    instruction: instruction.join(" "),
                    mode: ExecutionMode::Interactive,
                    include_extensions: include.as_ref().map(|s| s.split(',').map(|x| x.trim().to_string()).collect()),
                    exclude_dirs: exclude.clone().unwrap_or_default(),
                    max_files: *max_files,
                    dry_run: false,
                })
            }
            Some(Commands::Auto { cwd, instruction, include, exclude, max_files, dry_run }) => {
                Some(AgentConfig {
                    cwd: cwd.clone(),
                    instruction: instruction.join(" "),
                    mode: ExecutionMode::Auto,
                    include_extensions: include.as_ref().map(|s| s.split(',').map(|x| x.trim().to_string()).collect()),
                    exclude_dirs: exclude.clone().unwrap_or_default(),
                    max_files: *max_files,
                    dry_run: *dry_run,
                })
            }
            Some(Commands::Gui) | None => None,
        }
    }
}

pub fn parse_args() -> Cli {
    Cli::parse()
}

pub fn is_cli_mode(cli: &Cli) -> bool {
    matches!(cli.command, Some(Commands::Plan { .. }) | Some(Commands::Interactive { .. }) | Some(Commands::Auto { .. }))
}
