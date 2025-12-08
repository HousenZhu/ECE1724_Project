use std::sync::mpsc::Sender;

use ratatui::widgets::ListState;
use uuid::Uuid;
use ratatui::layout::Rect;

/// Who sent the message.
#[derive(Clone, Copy)]
pub enum MessageFrom {
    User,
    Assistant,
}

/// Single message in a branch.
#[derive(Clone)]
pub struct Message {
    pub from: MessageFrom,
    pub content: String,
}

/// A single conversation branch.
#[derive(Clone)]
pub struct Branch {
    pub id: usize,              // Unique branch identifier
    pub name: String,           // Branch display name ("main", "branch-1", ...)
    pub messages: Vec<Message>, // All messages in this branch
}

/// One chat session.
#[derive(Clone)]
pub struct Session {
    pub id: String,
    pub title: String,
    
    // Branch system
    pub branches: Vec<Branch>,  // All branches created in this session
    pub active_branch: usize,   // Index of the currently selected branch
}

/// Current input mode of the TUI (similar to Vim).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    /// Normal mode: keys are interpreted as commands (q, n, j, k, i, etc.).
    Normal,
    /// Insert mode: keys are inserted into the input buffer.
    Insert,
}

/// Events coming from background streaming worker.
#[derive(Debug)]
pub enum BackendEvent {
    AssistantChunk { session_idx: usize, branch_idx: usize, chunk: String },
    AssistantDone { session_idx: usize, branch_idx: usize },
}

/// Editing context for "fork branch by editing old message"
pub struct EditContext {
    pub session_idx: usize,     // Which session we are editing in
    pub branch_idx: usize,      // Which branch we are editing
    pub message_idx: usize,     // Message index where the fork begins
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
    /// Whether the "New Session" button is currently focused.
    pub new_button_selected: bool,  
    /// Vertical scroll offset for the message area on the right.
    pub msg_scroll: usize,  
    /// Screen area of the send button in the input panel (if drawn).
    pub send_button_area: Option<Rect>,
    /// Sender used to send backend events (assistant chunks) from worker threads.
    pub backend_tx: Option<Sender<BackendEvent>>,
    /// (session_idx, message_idx) of the currently streaming assistant message.
    pub streaming_assistant: Option<(usize, usize, usize)>,
    /// Whether the left session sidebar is collapsed.
    pub sidebar_collapsed: bool,
    /// Editing context (None if not editing)
    pub edit_ctx: Option<EditContext>,
    /// Hitboxes for user messages in the UI.
    pub user_msg_hitboxes: Vec<(usize, Rect)>,
    /// Which user message index is currently hovered.
    pub hovered_user_msg: Option<usize>,
}

impl App {
    /// Create a new App.
    pub fn new() -> Self {
        let mut list_state = ListState::default();


        // Start with one session that has a single "main" branch.
        let initial_session = Session {
            id: Uuid::new_v4().to_string(),
            title: "Session 1".into(),
            branches: vec![Branch {
                id: 0,
                name: "main".into(),
                messages: Vec::new(),
            }],
            active_branch: 0,
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
            new_button_selected: false,
            // Start at the top of the message list (no scrolling).
            msg_scroll: 0,
            send_button_area: None,
            backend_tx: None,
            streaming_assistant: None,
            sidebar_collapsed: false,
            edit_ctx: None,
            user_msg_hitboxes: Vec::new(),
            hovered_user_msg: None,
        }
    }
    
    /// Create a new empty session and switch to it.
    pub fn new_session(&mut self) {
        let id = Uuid::new_v4().to_string();

        self.sessions.push(Session {
            id,
            title: format!("Session {}", self.sessions.len() + 1),
            branches: vec![Branch {
                id: 0,
                name: "main".into(),
                messages: vec![],
            }],
            active_branch: 0,
        });

        // Set the new session as active.
        self.active_idx = self.sessions.len() - 1;
        self.list_state.select(Some(self.active_idx));
        self.msg_scroll = 0;
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
        if self.sessions.is_empty() {
            return;
        }
        if self.active_idx > 0 {
            self.active_idx -= 1;
        }
        self.msg_scroll = 0;
        self.list_state.select(Some(self.active_idx));
    }

    /// Move selection to the next session (if any).
    pub fn next_session(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        if self.active_idx + 1 < self.sessions.len() {
            self.active_idx += 1;
        }
        self.msg_scroll = 0;
        self.list_state.select(Some(self.active_idx));
    }

    /// Append a user message to the active session.
    pub fn push_user_message(&mut self, content: String) {
        let session = &mut self.sessions[self.active_idx];
        let branch = &mut session.branches[session.active_branch];
        branch.messages.push(Message {
            from: MessageFrom::User,
            content,
        });
    }

    /// Append an assistant message to the active branch of the active session.
    pub fn push_assistant_message(&mut self, content: String) {
        let session = &mut self.sessions[self.active_idx];
        let branch = &mut session.branches[session.active_branch];
        branch.messages.push(Message {
            from: MessageFrom::Assistant,
            content,
        });
    }
 
    /// Append assistant chunk to branch message
    pub fn append_assistant_chunk(
        &mut self,
        session_idx: usize,
        branch_idx: usize,
        chunk: String,
    ) {
        if let Some((s, b, msg_idx)) = self.streaming_assistant {
            if s == session_idx && b == branch_idx {
                if let Some(session) = self.sessions.get_mut(s) {
                    if let Some(branch) = session.branches.get_mut(b) {
                        if let Some(msg) = branch.messages.get_mut(msg_idx) {
                            msg.content.push_str(&chunk);
                        }
                    }
                }
            }
        }
    }

    /// Start streaming assistant message in a specific branch
    pub fn start_streaming_assistant(
        &mut self,
        session_idx: usize,
        branch_idx: usize,
    ) {
        let session = &mut self.sessions[session_idx];
        let branch = &mut session.branches[branch_idx];

        let msg_idx = branch.messages.len();
        branch.messages.push(Message {
            from: MessageFrom::Assistant,
            content: String::new(),
        });

        self.streaming_assistant = Some((session_idx, branch_idx, msg_idx));
    }

    /// Mark streaming as finished for (session_idx, branch_idx).
    pub fn finish_streaming(&mut self, session_idx: usize, branch_idx: usize) {
        if let Some((s, b, _)) = self.streaming_assistant {
            if s == session_idx && b == branch_idx {
                self.streaming_assistant = None;
            }
        }
    }

    /// Switch to the previous branch in the current session (if any).
    pub fn prev_branch(&mut self) {
        let session = &mut self.sessions[self.active_idx];
        if session.branches.is_empty() {
            return;
        }
        if session.active_branch > 0 {
            session.active_branch -= 1;
        }
        // Reset scroll when switching branches
        self.msg_scroll = 0;
    }

    /// Switch to the next branch in the current session (if any).
    pub fn next_branch(&mut self) {
        let session = &mut self.sessions[self.active_idx];
        if session.branches.is_empty() {
            return;
        }
        if session.active_branch + 1 < session.branches.len() {
            session.active_branch += 1;
        }
        // Reset scroll when switching branches
        self.msg_scroll = 0;
    }

    /// Current width of the left sidebar in columns.
    pub fn sidebar_width(&self) -> u16 {
        if self.sidebar_collapsed {
            3
        } else {
            25
        }
    }

    /// Toggle the visibility of the left sidebar.
    pub fn toggle_sidebar(&mut self) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        self.msg_scroll = 0;
    }
} 