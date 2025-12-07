use crate::mistral_client::Message;
use crate::chat::ChatMode;
use std::path::PathBuf;

/// Application state for TUI
pub struct App {
    /// Current project path
    pub project_path: PathBuf,
    /// Chat messages (role, content)
    pub messages: Vec<ChatMessage>,
    /// Current input text
    pub input: String,
    /// Input cursor position
    pub cursor_pos: usize,
    /// Current mode
    pub mode: ChatMode,
    /// Scroll offset for messages
    pub scroll: u16,
    /// Total estimated tokens
    pub tokens: usize,
    /// Is waiting for AI response
    pub loading: bool,
    /// Should quit
    pub should_quit: bool,
    /// Input history for up/down navigation
    pub input_history: Vec<String>,
    pub history_index: Option<usize>,
}

#[derive(Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub is_user: bool,
}

impl App {
    pub fn new(project_path: PathBuf) -> Self {
        Self {
            project_path,
            messages: Vec::new(),
            input: String::new(),
            cursor_pos: 0,
            mode: ChatMode::Code,
            scroll: 0,
            tokens: 0,
            loading: false,
            should_quit: false,
            input_history: Vec::new(),
            history_index: None,
        }
    }

    pub fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            ChatMode::Ask => ChatMode::Plan,
            ChatMode::Plan => ChatMode::Code,
            ChatMode::Code => ChatMode::Auto,
            ChatMode::Auto => ChatMode::Ask,
        };
    }

    pub fn add_user_message(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: content.clone(),
            is_user: true,
        });
        self.input_history.push(content);
        self.input.clear();
        self.cursor_pos = 0;
        self.history_index = None;
        self.update_tokens();
    }

    pub fn add_ai_message(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content,
            is_user: false,
        });
        self.update_tokens();
    }

    fn update_tokens(&mut self) {
        self.tokens = self.messages.iter()
            .map(|m| m.content.len() / 4)
            .sum();
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }

    pub fn move_cursor_left(&mut self) {
        self.cursor_pos = self.cursor_pos.saturating_sub(1);
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_pos < self.input.len() {
            self.cursor_pos += 1;
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += 1;
    }

    pub fn delete_char(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.input.remove(self.cursor_pos);
        }
    }

    pub fn history_up(&mut self) {
        if self.input_history.is_empty() {
            return;
        }
        match self.history_index {
            None => {
                self.history_index = Some(self.input_history.len() - 1);
            }
            Some(i) if i > 0 => {
                self.history_index = Some(i - 1);
            }
            _ => {}
        }
        if let Some(i) = self.history_index {
            self.input = self.input_history[i].clone();
            self.cursor_pos = self.input.len();
        }
    }

    pub fn history_down(&mut self) {
        if let Some(i) = self.history_index {
            if i < self.input_history.len() - 1 {
                self.history_index = Some(i + 1);
                self.input = self.input_history[i + 1].clone();
            } else {
                self.history_index = None;
                self.input.clear();
            }
            self.cursor_pos = self.input.len();
        }
    }

    pub fn to_api_messages(&self) -> Vec<Message> {
        self.messages.iter()
            .map(|m| Message {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect()
    }
}
