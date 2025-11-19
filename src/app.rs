use ratatui::widgets::ListState;
use uuid::Uuid;

/// Who sent the message.
#[derive(Clone, Copy)]
pub enum MessageFrom {
    User,
    Assistant,
}

/// Single message in a session.
#[derive(Clone)]
pub struct Message {
    pub from: MessageFrom,
    pub content: String,
}

/// One chat session (similar to a chat "room").
#[derive(Clone)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub messages: Vec<Message>,
}

/// Current input mode of the TUI (similar to Vim).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    /// Normal mode: keys are interpreted as commands (q, n, j, k, i, etc.).
    Normal,
    /// Insert mode: keys are inserted into the input buffer.
    Insert,
}

/// Global application state used by the TUI.
pub struct App {
    /// All sessions shown in the left sidebar.
    pub sessions: Vec<Session>,
    /// List selection state for the session list.
    pub list_state: ListState,
    /// Index of the currently active session in `sessions`.
    pub active_idx: usize,
    /// Current text in the input box.
    pub input: String,
    /// Current input mode (Normal or Insert).
    pub input_mode: InputMode,
}

impl App {
    /// Create a new App.
    pub fn new() -> Self {
        let mut list_state = ListState::default();

        // Start with one empty session.
        let initial_session = Session {
            id: Uuid::new_v4().to_string(),
            title: "Session 1".into(),
            messages: Vec::new(),
        };

        let sessions = vec![initial_session];

        list_state.select(Some(0)); // Select the first (only) session.

        Self {
            sessions,
            list_state,
            active_idx: 0,
            input: String::new(),
            // Start in Normal mode.
            input_mode: InputMode::Normal,
        }
    }
    
    /// Create a new empty session and switch to it.
    pub fn new_session(&mut self) {
        let id = Uuid::new_v4().to_string();

        self.sessions.push(Session {
            id,
            title: format!("Session {}", self.sessions.len() + 1),
            messages: Vec::new(),
        });

        // Set the new session as active.
        self.active_idx = self.sessions.len() - 1;
        self.list_state.select(Some(self.active_idx));
    }

    /// Get immutable reference to the active session.
    pub fn active_session(&self) -> &Session {
        &self.sessions[self.active_idx]
    }

    /// Get mutable reference to the active session.
    pub fn active_session_mut(&mut self) -> &mut Session {
        &mut self.sessions[self.active_idx]
    }

    /// Move selection to the previous session (if any).
    pub fn prev_session(&mut self) {
        if self.active_idx > 0 {
            self.active_idx -= 1;
        }
        self.list_state.select(Some(self.active_idx));
    }

    /// Move selection to the next session (if any).
    pub fn next_session(&mut self) {
        if self.active_idx + 1 < self.sessions.len() {
            self.active_idx += 1;
        }
        self.list_state.select(Some(self.active_idx));
    }

    /// Append a user message to the active session.
    pub fn push_user_message(&mut self, content: String) {
        self.active_session_mut().messages.push(Message {
            from: MessageFrom::User,
            content,
        });
    }

    /// Append an assistant message to the active session.
    pub fn push_assistant_message(&mut self, content: String) {
        self.active_session_mut().messages.push(Message {
            from: MessageFrom::Assistant,
            content,
        });
    }
}