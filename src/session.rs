use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Single message
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// A branch = messages + summary
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Session {
    pub id: String,
    pub branch: String,
    pub created_at: u64,
    pub messages: Vec<Message>,
    pub summary: Option<String>,
}

/// Core manager containing session, branches, model
#[derive(Clone)]
pub struct SessionManager {
    pub session: Session,
    pub branches: HashMap<String, Session>,
    pub model: String,
}

const LOG_DIR: &str = "logs";
const DEFAULT_MODEL: &str = "qwen-plus";

impl SessionManager {
    /// Create new session (with main branch)
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
            model: DEFAULT_MODEL.into(),
        }
    }

    /// Save current branch as JSON
    pub fn save_to_logs(&self) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(LOG_DIR)?;
        let path = Path::new(LOG_DIR).join(format!("{}_{}.json", self.session.id, self.session.branch));
        let file = File::create(&path)?;
        serde_json::to_writer_pretty(file, &self.session)?;
        println!("💾 Saved: {}", path.display());
        Ok(())
    }

    /// Remove ALL sessions
    pub fn clear_all_sessions(&mut self) {
        if !ask_confirm("⚠️ Delete ALL sessions?") {
            println!("❎ Cancelled.");
            return;
        }
        fs::remove_dir_all(LOG_DIR).ok();
        fs::create_dir_all(LOG_DIR).ok();
        *self = SessionManager::new();
        println!("🔥 All sessions removed. New one started.");
    }

    /// Remove all branches except main
    pub fn clear_other_branches(&mut self) {
        if !ask_confirm("⚠️ Remove all branches except 'main'?") {
            println!("❎ Cancelled.");
            return;
        }

        let id = self.session.id.clone();
        for (name, _) in self.branches.clone() {
            if name != "main" {
                let _ = fs::remove_file(Path::new(LOG_DIR).join(format!("{}_{}.json", id, name)));
                self.branches.remove(&name);
            }
        }

        self.session.branch = "main".into();
        println!("🌿 Only main branch kept.");
    }

    /// Load session's main branch
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
        println!("📌 Session loaded: {}", id);
        Ok(())
    }

    /// -------- Branch commands --------
    pub fn handle_branch_command(&mut self, input: &str) -> Result<(), Box<dyn Error>> {
        let parts: Vec<&str> = input.split_whitespace().collect();

        match parts.get(1).copied() {
            Some("new") => self.branch_new(parts.get(2).unwrap_or(&"")),
            Some("switch") => self.branch_switch(parts.get(2).unwrap_or(&"")),
            Some("list") => {
                self.branch_list();
                Ok(())
            }
            Some("current") => {
                println!("📌 Current branch: {}", self.session.branch);
                Ok(())
            }
            Some("delete") => self.branch_delete(parts.get(2).unwrap_or(&"")),
            Some("rename") => self.branch_rename(parts.get(2).unwrap_or(&""), parts.get(3).unwrap_or(&"")),
            _ => {
                println!("Usage: /branch [new|switch|list|current|delete|rename]");
                Ok(())
            }
        }
    }

    fn branch_new(&mut self, name: &str) -> Result<(), Box<dyn Error>> {
        if name.is_empty() {
            println!("⚠️ Missing name.");
            return Ok(());
        }
        if self.branches.contains_key(name) {
            println!("⚠️ Branch exists.");
            return Ok(());
        }

        let mut new_branch = self.session.clone();
        new_branch.branch = name.into();
        self.session = new_branch.clone();
        self.branches.insert(name.into(), new_branch);

        self.save_to_logs()?;
        println!("🌱 Branch created: {}", name);
        Ok(())
    }

    fn branch_switch(&mut self, name: &str) -> Result<(), Box<dyn Error>> {
        if name.is_empty() {
            println!("⚠️ Missing name.");
            return Ok(());
        }

        self.save_to_logs().ok();

        if let Some(b) = self.branches.get(name).cloned() {
            self.session = b;
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
            println!("❌ Branch not found.");
        }

        Ok(())
    }

    fn branch_list(&self) {
        println!("🌿 Branches:");
        for k in self.branches.keys() {
            let mark = if *k == self.session.branch { "*" } else { " " };
            println!("{} {}", mark, k);
        }
    }

    fn branch_delete(&mut self, name: &str) -> Result<(), Box<dyn Error>> {
        if name == "main" {
            println!("⚠️ Cannot delete main.");
            return Ok(());
        }
        if !ask_confirm(&format!("Delete branch '{}'? ", name)) {
            println!("❎ Cancelled.");
            return Ok(());
        }

        self.branches.remove(name);
        let path = Path::new(LOG_DIR).join(format!("{}_{}.json", self.session.id, name));
        let _ = fs::remove_file(path);

        if self.session.branch == name {
            self.session = self.branches["main"].clone();
            println!("↩️ Switched back to main.");
        }

        Ok(())
    }

    fn branch_rename(&mut self, old: &str, new: &str) -> Result<(), Box<dyn Error>> {
        if old.is_empty() || new.is_empty() {
            println!("⚠️ Missing name.");
            return Ok(());
        }
        if old == "main" {
            println!("⚠️ Cannot rename main.");
            return Ok(());
        }
        if !self.branches.contains_key(old) {
            println!("❌ Unknown branch.");
            return Ok(());
        }

        let old_path = Path::new(LOG_DIR).join(format!("{}_{}.json", self.session.id, old));
        let new_path = Path::new(LOG_DIR).join(format!("{}_{}.json", self.session.id, new));
        if old_path.exists() {
            fs::rename(old_path, new_path)?;
        }

        let mut s = self.branches.remove(old).unwrap();
        s.branch = new.into();
        self.branches.insert(new.to_string(), s.clone());

        if self.session.branch == old {
            self.session = s;
        }

        println!("✏️ Renamed {} → {}", old, new);
        Ok(())
    }

    /// -------- Session commands --------
    pub fn handle_session_command(&mut self, input: &str) -> Result<(), Box<dyn Error>> {
        let parts: Vec<&str> = input.split_whitespace().collect();

        match parts.get(1).copied() {
            Some("list") => self.session_list(),
            Some("current") => {
                println!("📌 Session ID: {}", self.session.id);
                Ok(())
            }
            Some("delete") => self.session_delete(parts.get(2).unwrap_or(&"")),
            Some("clear") => {
                self.clear_all_sessions();
                Ok(())
            }
            _ => {
                println!("Usage: /session [list|current|delete|clear]");
                Ok(())
            }
        }
    }

    fn session_list(&self) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(LOG_DIR)?;

        let mut groups: HashMap<String, usize> = HashMap::new();
        for entry in fs::read_dir(LOG_DIR)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();

            if let Some((id, rest)) = name.split_once('_') {
                if rest.ends_with(".json") {
                    *groups.entry(id.into()).or_insert(0) += 1;
                }
            }
        }

        if groups.is_empty() {
            println!("(no sessions)");
            return Ok(());
        }

        println!("📚 Sessions:");
        for (id, branches) in groups {
            println!("- {} ({} branches)", id, branches);
        }
        Ok(())
    }

    fn session_delete(&mut self, id: &str) -> Result<(), Box<dyn Error>> {
        if id.is_empty() {
            println!("⚠️ Missing ID.");
            return Ok(());
        }

        let mut files = vec![];
        for entry in fs::read_dir(LOG_DIR)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&format!("{}_", id)) && name.ends_with(".json") {
                files.push(entry.path());
            }
        }

        if files.is_empty() {
            println!("❌ Session not found.");
            return Ok(());
        }

        if !ask_confirm(&format!("Delete session '{}'? ", id)) {
            println!("❎ Cancelled.");
            return Ok(());
        }

        for file in files {
            let _ = fs::remove_file(file);
        }

        if self.session.id == id {
            *self = SessionManager::new();
            println!("🚮 Deleted current session. New session created.");
        }

        Ok(())
    }
}

/// Yes/No prompt
fn ask_confirm(prompt: &str) -> bool {
    use std::io::{stdin, stdout, Write};

    print!("{prompt} (y/n): ");
    stdout().flush().ok();

    let mut buf = String::new();
    stdin().read_line(&mut buf).ok();

    matches!(buf.trim().to_lowercase().as_str(), "y" | "yes")
}
