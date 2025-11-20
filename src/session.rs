use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single message from user or assistant.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// One branch of a session, holding messages and a summary.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Session {
    pub id: String,
    pub branch: String,
    pub created_at: u64,
    pub messages: Vec<Message>,
    pub summary: Option<String>,
}

/// Core manager holding all sessions, branches, and current model.
pub struct SessionManager {
    pub session: Session,
    pub branches: HashMap<String, Session>,
    pub model: String,
}

/// Directory for saving message logs.
const LOG_DIR: &str = "logs";

/// Default model name.
const MODEL: &str = "gemma3";

impl SessionManager {
    /// Creates a brand new session with main branch.
    pub fn new() -> Self {
        fs::create_dir_all(LOG_DIR).ok();

        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let id = ts.to_string();

        let main = Session {
            id: id.clone(),
            branch: "main".into(),
            created_at: ts,
            messages: vec![],
            summary: None,
        };

        Self {
            session: main.clone(),
            branches: HashMap::from([("main".into(), main)]),
            model: MODEL.into(),
        }
    }

    /// Save current branch as a JSON file in /logs.
    pub fn save_to_logs(&self) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(LOG_DIR)?;
        let path = Path::new(LOG_DIR).join(format!("{}_{}.json", self.session.id, self.session.branch));
        let file = File::create(&path)?;
        serde_json::to_writer_pretty(file, &self.session)?;
        println!("💾 Saved: {}", path.display());
        Ok(())
    }

    /// Remove ALL stored sessions (nuclear reset).
    pub fn clear_all_sessions(&mut self) {
        if !ask_confirm("⚠️ Delete ALL sessions? This cannot be undone.") {
            println!("❎ Cancelled.");
            return;
        }
        fs::remove_dir_all(LOG_DIR).ok();
        fs::create_dir_all(LOG_DIR).ok();
        *self = SessionManager::new();
        println!("🔥 All sessions removed. Started a new one.");
    }

    /// Delete all branches within the current session EXCEPT 'main'.
    pub fn clear_other_branches(&mut self) {
        if !ask_confirm("⚠️ Remove ALL branches except 'main'?") {
            println!("❎ Cancelled.");
            return;
        }

        let id = self.session.id.clone();

        for (name, _) in self.branches.clone() {
            if name != "main" {
                let _ = std::fs::remove_file(
                    Path::new(LOG_DIR).join(format!("{}_{}.json", id, name)),
                );
                self.branches.remove(&name);
            }
        }

        self.session.branch = "main".into();
        println!("🌿 Only 'main' branch kept.");
    }

    /// Load a session by ID, always loading its main branch.
    pub fn load_session(&mut self, id: Option<&str>) -> Result<(), Box<dyn Error>> {
        let Some(id) = id else {
            println!("⚠️ Provide a session ID.");
            return Ok(());
        };

        let path = Path::new(LOG_DIR).join(format!("{}_main.json", id));
        if !path.exists() {
            println!("❌ Not found: {}", path.display());
            return Ok(());
        }

        let file = File::open(&path)?;
        let main: Session = serde_json::from_reader(file)?;
        self.session = main.clone();
        self.branches = HashMap::from([("main".into(), main)]);
        Ok(())
    }

    /// Handles /branch commands.
    pub fn handle_branch_command(&mut self, input: &str) -> Result<(), Box<dyn Error>> {
        let parts: Vec<&str> = input.split_whitespace().collect();

        match parts.get(1).map(|s| *s) {
            Some("new") => self.branch_new(parts.get(2).unwrap_or(&"")),
            Some("switch") => self.branch_switch(parts.get(2).unwrap_or(&"")),
            Some("list") => { self.branch_list(); Ok(()) }
            Some("current") => { println!("📌 Current branch: {}", self.session.branch); Ok(()) }
            Some("delete") => self.branch_delete(parts.get(2).unwrap_or(&"")),
            Some("rename") => self.branch_rename(parts.get(2).unwrap_or(&""), parts.get(3).unwrap_or(&"")),
            _ => { println!("Usage: /branch [new|switch|list|current|delete|rename]"); Ok(()) }
        }
    }

    fn branch_new(&mut self, name: &str) -> Result<(), Box<dyn Error>> {
        if name.is_empty() {
            println!("⚠️ Missing branch name.");
            return Ok(());
        }
        if self.branches.contains_key(name) {
            println!("⚠️ Branch '{}' exists.", name);
            return Ok(());
        }
        let mut new_branch = self.session.clone();
        new_branch.branch = name.into();
        self.session = new_branch.clone();
        self.branches.insert(name.into(), new_branch);
        self.save_to_logs()?;
        println!("🌱 Branch '{}' created.", name);
        Ok(())
    }

    fn branch_switch(&mut self, name: &str) -> Result<(), Box<dyn Error>> {
        if name.is_empty() {
            println!("⚠️ Missing branch name.");
            return Ok(());
        }
        self.save_to_logs().ok();

        if let Some(existing) = self.branches.get(name).cloned() {
            self.session = existing;
            println!("🔀 Switched to '{}'", name);
            return Ok(());
        }

        let path = Path::new(LOG_DIR).join(format!("{}_{}.json", self.session.id, name));
        if path.exists() {
            let file = File::open(&path)?;
            let loaded: Session = serde_json::from_reader(file)?;
            self.branches.insert(name.into(), loaded.clone());
            self.session = loaded;
            println!("🔀 Loaded '{}'", name);
        } else {
            println!("❌ Branch '{}' not found.", name);
        }
        Ok(())
    }

    fn branch_list(&self) {
        println!("🌿 Branches:");
        for k in self.branches.keys() {
            let star = if *k == self.session.branch { "*" } else { " " };
            println!("{} {}", star, k);
        }
    }

    fn branch_delete(&mut self, name: &str) -> Result<(), Box<dyn Error>> {
        if name == "main" {
            println!("⚠️ Cannot delete 'main'.");
            return Ok(());
        }
        if !ask_confirm(&format!("Delete branch '{}'?", name)) {
            println!("❎ Cancelled.");
            return Ok(());
        }

        self.branches.remove(name);
        let path = Path::new(LOG_DIR).join(format!("{}_{}.json", self.session.id, name));
        let _ = std::fs::remove_file(path);

        if self.session.branch == name {
            if let Some(main) = self.branches.get("main").cloned() {
                self.session = main;
            }
            println!("↩️ Returned to 'main'");
        }
        Ok(())
    }

    fn branch_rename(&mut self, old: &str, new: &str) -> Result<(), Box<dyn Error>> {
        if old.is_empty() || new.is_empty() {
            println!("⚠️ Missing argument.");
            return Ok(());
        }
        if old == "main" {
            println!("⚠️ Cannot rename 'main'.");
            return Ok(());
        }
        if !self.branches.contains_key(old) {
            println!("❌ Unknown branch '{}'", old);
            return Ok(());
        }
        if self.branches.contains_key(new) {
            println!("⚠️ '{}' already exists.", new);
            return Ok(());
        }

        let old_path = Path::new(LOG_DIR).join(format!("{}_{}.json", self.session.id, old));
        let new_path = Path::new(LOG_DIR).join(format!("{}_{}.json", self.session.id, new));
        if old_path.exists() {
            fs::rename(old_path, new_path)?;
        }

        let mut s = self.branches.remove(old).unwrap();
        s.branch = new.into();
        self.branches.insert(new.into(), s.clone());
        if self.session.branch == old {
            self.session = s;
        }
        println!("✏️ Renamed '{}' → '{}'", old, new);
        Ok(())
    }

    /// Handles /session commands.
    pub fn handle_session_command(&mut self, input: &str) -> Result<(), Box<dyn Error>> {
        let parts: Vec<&str> = input.split_whitespace().collect();

        match parts.get(1).map(|s| *s) {
            Some("list") => self.session_list(),
            Some("current") => { println!("📌 Session ID: {}", self.session.id); Ok(()) }
            Some("delete") => self.session_delete(parts.get(2).unwrap_or(&"")),
            Some("clear") => { self.clear_all_sessions(); Ok(()) }
            _ => { println!("Usage: /session [list|current|delete <id>|clear]"); Ok(()) }
        }
    }

    /// Print stored sessions summary.
    fn session_list(&self) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(LOG_DIR)?;

        let mut groups: HashMap<String, usize> = HashMap::new();
        for entry in fs::read_dir(LOG_DIR)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if let Some((id, rest)) = name.split_once('_') {
                    if rest.ends_with(".json") {
                        *groups.entry(id.into()).or_insert(0) += 1;
                    }
                }
            }
        }

        if groups.is_empty() {
            println!("(no sessions)");
            return Ok(());
        }

        println!("📚 Sessions:");
        for (id, count) in groups {
            println!("- {} ({} branches)", id, count);
        }
        Ok(())
    }

    fn session_delete(&mut self, id: &str) -> Result<(), Box<dyn Error>> {
        if id.is_empty() {
            println!("⚠️ Missing session ID.");
            return Ok(());
        }

        let mut files = vec![];
        for entry in fs::read_dir(LOG_DIR)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if name.starts_with(&(id.to_string() + "_")) && name.ends_with(".json") {
                    files.push(path);
                }
            }
        }

        if files.is_empty() {
            println!("❌ Session '{}' not found.", id);
            return Ok(());
        }

        if !ask_confirm(&format!("Delete session '{}'?", id)) {
            println!("❎ Cancelled.");
            return Ok(());
        }

        for p in files {
            let _ = std::fs::remove_file(p);
        }

        if self.session.id == id {
            *self = SessionManager::new();
            println!("🚮 Deleted current session. Started new '{}'.", self.session.id);
        }
        Ok(())
    }
}

/// Ask question via CLI (y/n).
fn ask_confirm(prompt: &str) -> bool {
    use std::io::{stdin, stdout, Write};
    print!("{} (y/n): ", prompt);
    stdout().flush().ok();
    let mut buf = String::new();
    stdin().read_line(&mut buf).ok();
    matches!(buf.trim().to_lowercase().as_str(), "y" | "yes")
}
