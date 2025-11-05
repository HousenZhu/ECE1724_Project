use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::User => "User",
            Role::Assistant => "Assistant",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub struct MemorySession {
    pub messages: Vec<Message>,
}

impl MemorySession {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn add(&mut self, role: Role, content: String) {
        self.messages.push(Message {
            role,
            content,
            timestamp: Utc::now(),
        });
    }
    
    pub fn build_prompt(&self) -> String {
        let mut s = String::new();
        for msg in &self.messages {
            s.push_str(&format!("{}: {}\n", msg.role.as_str(), msg.content));
        }
        s.push_str("Assistant:");
        s
    }
    
    pub fn history_text(&self) -> String {
        self.messages
            .iter()
            .map(|m| format!("{}: {}", m.role.as_str(), m.content))
            .collect::<Vec<_>>()
            .join("\n")
    }
}