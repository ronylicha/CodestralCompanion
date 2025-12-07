/// ASCII Art logo for Codestral
pub const LOGO: &str = r#"
 ▄████▄   ▒█████  ▓█████▄ ▓█████   ██████ ▄▄▄█████▓ ██▀███   ▄▄▄       ██▓    
▒██▀ ▀█  ▒██▒  ██▒▒██▀ ██▌▓█   ▀ ▒██    ▒ ▓  ██▒ ▓▒▓██ ▒ ██▒▒████▄    ▓██▒    
▒▓█    ▄ ▒██░  ██▒░██   █▌▒███   ░ ▓██▄   ▒ ▓██░ ▒░▓██ ░▄█ ▒▒██  ▀█▄  ▒██░    
▒▓▓▄ ▄██▒▒██   ██░░▓█▄   ▌▒▓█  ▄   ▒   ██▒░ ▓██▓ ░ ▒██▀▀█▄  ░██▄▄▄▄██ ▒██░    
▒ ▓███▀ ░░ ████▓▒░░▒████▓ ░▒████▒▒██████▒▒  ▒██▒ ░ ░██▓ ▒██▒ ▓█   ▓██▒░██████▒
░ ░▒ ▒  ░░ ▒░▒░▒░  ▒▒▓  ▒ ░░ ▒░ ░▒ ▒▓▒ ▒ ░  ▒ ░░   ░ ▒▓ ░▒▓░ ▒▒   ▓▒█░░ ▒░▓  ░
  ░  ▒     ░ ▒ ▒░  ░ ▒  ▒  ░ ░  ░░ ░▒  ░ ░    ░      ░▒ ░ ▒░  ▒   ▒▒ ░░ ░ ▒  ░
"#;

/// Compact logo for header (3 lines)
pub const LOGO_COMPACT: [&str; 3] = [
    "▄▄▄▄▄▄",
    "██████",
    "▀▀▀▀▀▀",
];

/// Pixel art style Mistral icon
pub const MISTRAL_ICON: [&str; 5] = [
    "  ▄▄  ",
    " ████ ",
    "██████",
    " ████ ",
    "  ▀▀  ",
];

/// Orange color for Mistral brand
pub const MISTRAL_COLOR: ratatui::style::Color = ratatui::style::Color::Rgb(255, 127, 0);
