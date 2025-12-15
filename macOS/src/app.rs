use std::sync::mpsc::Sender;

use ratatui::widgets::ListState;
use uuid::Uuid;
use ratatui::layout::Rect;

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::fs::File;
use std::path::Path;
use serde_json;

/// Who sent the message.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum MessageFrom {
    User,
    Assistant,
}

/// Single message in a branch.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub from: MessageFrom,
    pub content: String,
}

/// A single conversation branch.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Branch {
    pub id: usize,              // Unique branch identifier
    pub name: String,           // Branch display name ("main", "branch-1", ...)
    pub messages: Vec<Message>, // All messages in this branch
}

/// One chat session.
#[derive(Serialize, Deserialize, Debug, Clone)]
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
    /// Vertical scroll offset for the input box (0 = bottom)
    pub input_scroll: usize,
    // Screen area of the input box, used for mouse hit-testing and scrolling behavior.
    pub input_area: Option<Rect>, 
    // Clickable area for the sidebar toggle button (collapse / expand).
    pub toggle_sidebar_area: Option<Rect>,
    // Clickable area for the "New Chat" button in the sidebar header.
    pub new_chat_area: Option<Rect>,
    pub session_hitboxes: Vec<(usize, Rect)>,
    pub edit_area: Option<(usize, Rect)>,
}

impl App {
    /// Create a new App.
    pub fn new() -> Self {
        let mut list_state = ListState::default();

        let mut sessions = Self::load_logs().unwrap_or_default();

        if sessions.is_empty() {
            sessions.push(Session {
                id: Uuid::new_v4().to_string(),
                title: "Session 1".to_string(),
                branches: vec![Branch {
                    id: 0,
                    name: "main".to_string(),
                    messages: vec![],
                }],
                active_branch: 0,
            });
        }

        let active_idx = sessions.len() - 1;
    

        // let sessions = vec![initial_session];

        list_state.select(Some(active_idx)); // Select the first (only) session.

        Self {
            sessions,
            list_state,
            active_idx: active_idx,
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
            input_scroll: 0,
            input_area: None, 
            toggle_sidebar_area: None,
            new_chat_area: None,
            session_hitboxes: Vec::new(),
            edit_area: None,
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

    /// Save current branch as a JSON file in /logs.
    pub fn save_to_logs(&mut self) -> Result<(), Box<dyn Error>> {
        // the saving address of history conversation 
        let log_dir: &str = "logs";

        let session = &mut self.sessions[self.active_idx];
        let branch = &mut session.branches[session.active_branch];

        fs::create_dir_all(log_dir)?;
        let path = Path::new(log_dir).join(format!("{}_{}.json", session.title, branch.name));
        let file = File::create(&path)?;
        serde_json::to_writer_pretty(file, branch)?;
        // println!("💾 Saved: {}", path.display());
        Ok(())
    }

    /// Build conversation history as a prompt string.
    pub(crate) fn history_string(&mut self) -> String {
        let session = &mut self.sessions[self.active_idx];
        let branch = &mut session.branches[session.active_branch];

        branch.messages
            .iter()
            .map(|m| format!("{}: {}", 
                match m.from {
                    MessageFrom::User => "User",
                    MessageFrom::Assistant => "Assistant",
                },
            m.content))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Loads all sessions and branches from /logs.
    pub fn load_logs() -> Result<Vec<Session>, Box<dyn std::error::Error>> {
        let log_dir: &str = "logs";
        let mut sessions_map: std::collections::HashMap<String, Vec<Branch>> = std::collections::HashMap::new();

        // Make sure directory exists
        if !Path::new(log_dir).exists() {
            return Ok(vec![]);
        }

        // Iterate all JSON files
        for entry in fs::read_dir(log_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            // File name example: "Session 1_main.json"
            let filename = path.file_stem().unwrap().to_string_lossy();

            // split into "Session 1" and "main"
            let parts: Vec<&str> = filename.split('_').collect();
            if parts.len() != 2 {
                continue;
            }

            let session_title = parts[0].to_string();
            let branch_name = parts[1].to_string();

            // Deserialize file into Branch
            let file = File::open(&path)?;
            let mut branch: Branch = serde_json::from_reader(file)?;

            // Fix branch name if needed
            branch.name = branch_name;

            // Insert into map
            sessions_map
                .entry(session_title)
                .or_default()
                .push(branch);
        }

        // Convert map into Vec<Session>
        let mut sessions: Vec<Session> = vec![];

        for (title, branches) in sessions_map {
            let id = Uuid::new_v4().to_string();
            sessions.push(Session {
                id,
                title,
                branches: branches.clone(),
                active_branch: branches.len()-1,
            });
        }

        Ok(sessions)
    }
} 