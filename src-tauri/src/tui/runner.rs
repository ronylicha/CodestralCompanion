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
use crate::mistral_client::{MistralClient, ApiProvider, Message};
use crate::agent::load_api_settings;
use crate::indexer::CodebaseIndex;
use crate::differ::parse_ai_response;
use crate::chat::ChatMode;

const SYSTEM_PROMPT: &str = r#"Tu es un assistant de programmation expert. Tu analyses des codebases et proposes des modifications.

RÈGLES:
1. Réponds TOUJOURS en français
2. Sois précis et concis
3. Pour modifier un fichier:

<file path="chemin/fichier.ext">
<<<<<<< ORIGINAL
code original
=======
nouveau code
>>>>>>> MODIFIED
</file>

Pour créer un fichier:
<new_file path="chemin/nouveau.ext">
contenu
</new_file>
"#;

const AUTO_MODE_SUFFIX: &str = "\n\nMODE AUTO: Tu dois continuer à travailler jusqu'à ce que la tâche soit complètement terminée. Si tu as besoin de faire plusieurs modifications, fais-les toutes. Ne t'arrête pas avant d'avoir terminé.";

/// Command menu items
pub const COMMANDS: &[(&str, &str)] = &[
    ("new", "Nouvelle conversation"),
    ("resume", "Reprendre une conversation"),
    ("clear", "Effacer l'historique"),
    ("reindex", "Réindexer le projet"),
    ("ask", "Mode ASK - Questions simples"),
    ("plan", "Mode PLAN - Planification"),
    ("code", "Mode CODE - Modifications avec confirmation"),
    ("auto", "Mode AUTO - Application automatique"),
    ("quit", "Quitter"),
];

pub struct TuiRunner {
    app: App,
    client: MistralClient,
    system_prompt: String,
    show_command_menu: bool,
    command_filter: String,
    selected_command: usize,
}

impl TuiRunner {
    pub fn new(project_path: PathBuf) -> Result<Self, String> {
        let (api_key, provider) = load_api_settings()?;
        
        // Index codebase for context
        let index = CodebaseIndex::index(&project_path, None, &[], 50)?;
        let context = index.build_context(20000);
        let codebase_context = context.first().cloned().unwrap_or_default();
        
        let system_prompt = format!("{}\n\nCODEBASE:\n{}", SYSTEM_PROMPT, codebase_context);
        
        Ok(Self {
            app: App::new(project_path),
            client: MistralClient::new(api_key, provider),
            system_prompt,
            show_command_menu: false,
            command_filter: String::new(),
            selected_command: 0,
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
            // Draw UI
            terminal.draw(|f| {
                if self.show_command_menu {
                    self.draw_with_command_menu(f);
                } else {
                    ui::draw(f, &self.app);
                }
            }).map_err(|e| e.to_string())?;

            // Handle events
            if event::poll(Duration::from_millis(100)).map_err(|e| e.to_string())? {
                if let Event::Key(key) = event::read().map_err(|e| e.to_string())? {
                    if self.show_command_menu {
                        self.handle_command_menu_key(key.code);
                    } else {
                        match key.code {
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                self.app.should_quit = true;
                            }
                            KeyCode::Esc => {
                                self.app.should_quit = true;
                            }
                            KeyCode::BackTab => {
                                self.app.cycle_mode();
                            }
                            KeyCode::Char('/') if self.app.input.is_empty() => {
                                self.show_command_menu = true;
                                self.command_filter.clear();
                                self.selected_command = 0;
                            }
                            KeyCode::Enter => {
                                if !self.app.input.is_empty() {
                                    self.send_message().await?;
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

    fn handle_command_menu_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc => {
                self.show_command_menu = false;
            }
            KeyCode::Enter => {
                self.execute_selected_command();
                self.show_command_menu = false;
            }
            KeyCode::Up => {
                if self.selected_command > 0 {
                    self.selected_command -= 1;
                }
            }
            KeyCode::Down => {
                let filtered = self.filtered_commands();
                if self.selected_command < filtered.len().saturating_sub(1) {
                    self.selected_command += 1;
                }
            }
            KeyCode::Char(c) => {
                self.command_filter.push(c);
                self.selected_command = 0;
            }
            KeyCode::Backspace => {
                self.command_filter.pop();
                self.selected_command = 0;
            }
            _ => {}
        }
    }

    fn filtered_commands(&self) -> Vec<(&str, &str)> {
        COMMANDS.iter()
            .filter(|(cmd, _)| cmd.contains(&self.command_filter.as_str()))
            .cloned()
            .collect()
    }

    fn execute_selected_command(&mut self) {
        let filtered = self.filtered_commands();
        if let Some((cmd, _)) = filtered.get(self.selected_command) {
            match *cmd {
                "quit" => self.app.should_quit = true,
                "clear" => {
                    self.app.messages.clear();
                }
                "ask" => self.app.mode = ChatMode::Ask,
                "plan" => self.app.mode = ChatMode::Plan,
                "code" => self.app.mode = ChatMode::Code,
                "auto" => self.app.mode = ChatMode::Auto,
                _ => {}
            }
        }
        self.command_filter.clear();
    }

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

        // Command list
        let filtered = self.filtered_commands();
        let items: Vec<ListItem> = filtered.iter()
            .enumerate()
            .map(|(i, (cmd, desc))| {
                let style = if i == self.selected_command {
                    Style::default().bg(Color::Rgb(60, 60, 100)).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!("/{:<10}", cmd), Style::default().fg(Color::Cyan)),
                    Span::styled(format!(" {}", desc), style),
                ]))
            })
            .collect();

        let list = List::new(items);
        frame.render_widget(list, menu_layout[1]);
    }

    async fn send_message(&mut self) -> Result<(), String> {
        let input = self.app.input.clone();
        self.app.add_user_message(input.clone());
        self.app.loading = true;

        // Build messages
        let mut messages = vec![Message {
            role: "system".to_string(),
            content: if self.app.mode == ChatMode::Auto {
                format!("{}{}", self.system_prompt, AUTO_MODE_SUFFIX)
            } else {
                self.system_prompt.clone()
            },
        }];
        messages.extend(self.app.to_api_messages());

        // Send to API
        match self.client.chat(messages).await {
            Ok(response) => {
                self.app.loading = false;
                
                // Parse and apply changes if applicable
                let changes = parse_ai_response(&response, &self.app.project_path);
                
                if !changes.is_empty() && self.app.mode != ChatMode::Ask {
                    // In AUTO mode, apply immediately
                    if self.app.mode == ChatMode::Auto {
                        for change in &changes.modifications {
                            let _ = change.apply();
                        }
                        for new_file in &changes.new_files {
                            let _ = new_file.apply();
                        }
                    }
                }
                
                self.app.add_ai_message(response);
                
                // Auto-scroll to bottom
                self.app.scroll = u16::MAX;
            }
            Err(e) => {
                self.app.loading = false;
                self.app.add_ai_message(format!("Erreur: {}", e));
            }
        }

        Ok(())
    }
}

pub async fn run_tui(project_path: PathBuf) -> Result<(), String> {
    let mut runner = TuiRunner::new(project_path)?;
    runner.run().await
}
