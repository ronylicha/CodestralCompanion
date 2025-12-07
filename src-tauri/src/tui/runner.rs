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
}

impl TuiRunner {
    pub fn new(project_path: PathBuf) -> Result<Self, String> {
        let (api_key, provider) = load_api_settings()?;
        
        // Index codebase for context
        let index = CodebaseIndex::index(&project_path, None, &[], 50)?;
        let context = index.build_context(20000);
        let codebase_context = context.first().cloned().unwrap_or_default();
        
        // Load project memory file
        let memory_file = project_path.join(".codestral").join("memory.md");
        let project_memory = if memory_file.exists() {
            std::fs::read_to_string(&memory_file).unwrap_or_default()
        } else {
            String::new()
        };
        
        let system_prompt = format!("{}\n\nCODEBASE:\n{}", SYSTEM_PROMPT, codebase_context);
        
        Ok(Self {
            app: App::new(project_path),
            client: MistralClient::new(api_key, provider),
            system_prompt,
            project_memory,
            memory_file,
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
                            // Tab with Alt+Shift cycles mode
                            KeyCode::Tab if key.modifiers == (KeyModifiers::ALT | KeyModifiers::SHIFT) => {
                                self.app.cycle_mode();
                            }
                            KeyCode::BackTab if key.modifiers.contains(KeyModifiers::ALT) => {
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
                                self.app.scroll = u16::MAX;
                            }
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
        
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
}

/// Multi-question form with Tab navigation
pub struct QuestionForm {
    pub questions: Vec<String>,
    pub answers: Vec<String>,
    pub current_field: usize,
    pub cursor_pos: usize,
}

impl QuestionForm {
    pub fn new(questions: Vec<String>) -> Self {
        let count = questions.len();
        Self {
            questions,
            answers: vec![String::new(); count],
            current_field: 0,
            cursor_pos: 0,
        }
    }

    pub fn next_field(&mut self) {
        if self.current_field < self.questions.len() - 1 {
            self.current_field += 1;
            self.cursor_pos = self.answers[self.current_field].len();
        }
    }

    pub fn prev_field(&mut self) {
        if self.current_field > 0 {
            self.current_field -= 1;
            self.cursor_pos = self.answers[self.current_field].len();
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.answers[self.current_field].insert(self.cursor_pos, c);
        self.cursor_pos += 1;
    }

    pub fn delete_char(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.answers[self.current_field].remove(self.cursor_pos);
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
                let field_height = 3u16;
                for (i, (question, answer)) in form.questions.iter().zip(form.answers.iter()).enumerate() {
                    let y = inner.y + (i as u16 * field_height);
                    if y + field_height > inner.y + inner.height {
                        break;
                    }

                    let is_current = i == form.current_field;
                    let border_color = if is_current { Color::Yellow } else { Color::DarkGray };

                    let field_area = Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: field_height,
                    };

                    let field_block = Block::default()
                        .title(format!(" {} ({}/{}) ", question, i + 1, form.questions.len()))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(border_color));

                    let field_inner = field_block.inner(field_area);
                    frame.render_widget(field_block, field_area);

                    // Draw answer with cursor if current
                    let text = if is_current {
                        let before = &answer[..form.cursor_pos.min(answer.len())];
                        let cursor = answer.get(form.cursor_pos..form.cursor_pos + 1).unwrap_or(" ");
                        let after = &answer[form.cursor_pos.saturating_add(1).min(answer.len())..];
                        Line::from(vec![
                            Span::raw(before),
                            Span::styled(cursor, Style::default().bg(Color::White).fg(Color::Black)),
                            Span::raw(after),
                        ])
                    } else {
                        Line::from(answer.as_str())
                    };

                    let para = Paragraph::new(text);
                    frame.render_widget(para, field_inner);
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
                        KeyCode::Char(c) => form.insert_char(c),
                        KeyCode::Backspace => form.delete_char(),
                        KeyCode::Left => {
                            if form.cursor_pos > 0 {
                                form.cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if form.cursor_pos < form.answers[form.current_field].len() {
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

    async fn send_message(&mut self) -> Result<(), String> {
        let input = self.app.input.clone();
        self.app.add_user_message(input.clone());
        
        // AUTO mode loop - continue until [TERMINÉ] or user cancels
        loop {
            self.app.loading = true;
            
            // Check if we need to compact context
            if self.app.tokens > COMPACT_THRESHOLD {
                self.compact_context().await?;
            }

            // Build messages with project memory
            let base_prompt = if !self.project_memory.is_empty() {
                format!("{}\n\nPROJECT MEMORY:\n{}", self.system_prompt, self.project_memory)
            } else {
                self.system_prompt.clone()
            };
            
            let mut messages = vec![Message {
                role: "system".to_string(),
                content: if self.app.mode == ChatMode::Auto {
                    format!("{}{}", base_prompt, AUTO_MODE_SUFFIX)
                } else {
                    base_prompt
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
                    self.app.scroll = u16::MAX;
                    
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
                Err(e) => {
                    self.app.loading = false;
                    self.app.add_ai_message(format!("Erreur: {}", e));
                    break;
                }
            }
        }

        Ok(())
    }

    async fn compact_context(&mut self) -> Result<(), String> {
        // Get all messages except system
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
            // Clear old messages and add summary
            self.app.messages.clear();
            self.app.messages.push(crate::tui::app::ChatMessage {
                role: "assistant".to_string(),
                content: format!("📝 Contexte compacté:\n{}", summary),
                is_user: false,
            });
            
            // Recalculate tokens
            self.app.tokens = self.app.messages.iter()
                .map(|m| m.content.len() / 4)
                .sum();
        }
        
        Ok(())
    }
}

pub async fn run_tui(project_path: PathBuf) -> Result<(), String> {
    let mut runner = TuiRunner::new(project_path)?;
    runner.run().await
}
