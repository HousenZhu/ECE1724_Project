use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ========================= Data Structures =========================

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub role: String,     // "user" | "assistant"
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Session {
    pub id: String,           // session id
    pub branch: String,       // branch name
    pub created_at: u64,      // unix ts
    pub messages: Vec<Message>,
    pub summary: Option<String>,
}

#[derive(Deserialize, Debug)]
struct OllamaResponse {
    response: Option<String>,
    done: Option<bool>,
}

pub struct SessionManager {
    pub current: Session,
    pub branches: HashMap<String, Session>, // key: branch name
}

const SESS_DIR: &str = "sessions";
const MODEL_NAME: &str = "gemma3";
const SUMMARY_TRIGGER_PAIRS: usize = 20; // user+assistant = 1 pair

impl SessionManager {
    pub fn new(id_opt: Option<String>) -> Self {
        fs::create_dir_all(SESS_DIR).ok();
        let id = id_opt.unwrap_or_else(new_id);
        let now = now_ts();
        let main = Session {
            id: id.clone(),
            branch: "main".to_string(),
            created_at: now,
            messages: vec![],
            summary: None,
        };
        let mut branches = HashMap::new();
        branches.insert("main".to_string(), main.clone());
        Self { current: main, branches }
    }

    // -------------------------- Paths & Files --------------------------
    fn file_path_for(&self, id: &str, branch: &str) -> PathBuf {
        Path::new(SESS_DIR).join(format!("{}_{}.json", id, branch))
    }

    fn current_file_path(&self) -> PathBuf {
        self.file_path_for(&self.current.id, &self.current.branch)
    }

    // -------------------------- Save / Load --------------------------
    pub fn save_current_branch(&self) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(SESS_DIR)?;
        let path = self.current_file_path();
        let file = File::create(&path)?;
        serde_json::to_writer_pretty(file, &self.current)?;
        println!("💾 Saved: {}", path.display());
        Ok(())
    }

    pub fn load_session(&mut self, id: Option<&str>) -> Result<(), Box<dyn Error>> {
        let Some(id) = id else {
            println!("⚠️ Please provide a session id.");
            return Ok(());
        };
        let path = self.file_path_for(id, "main");
        if !path.exists() {
            println!("❌ Not found: {}", path.display());
            return Ok(());
        }
        let file = File::open(&path)?;
        let main: Session = serde_json::from_reader(file)?;
        let mut branches = HashMap::new();
        branches.insert("main".to_string(), main.clone());
        self.current = main;
        self.branches = branches;
        println!("✅ Loaded session '{}', branch 'main'", id);
        Ok(())
    }

    // -------------------------- Branch Handling --------------------------
    pub fn handle_branch_command(&mut self, input: &str) -> Result<(), Box<dyn Error>> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() < 2 {
            self.print_branch_usage();
            return Ok(());
        }
        match parts[1] {
            "new" => {
                if let Some(name) = parts.get(2) {
                    self.branch_new(name)?;
                } else {
                    println!("⚠️ Missing branch name.");
                }
            }
            "switch" => {
                if let Some(name) = parts.get(2) {
                    self.branch_switch(name)?;
                } else {
                    println!("⚠️ Missing branch name.");
                }
            }
            "list" => {
                self.branch_list();
            }
            "current" => {
                println!("📌 Current branch: {}", self.current.branch);
            }
            "delete" => {
                if let Some(name) = parts.get(2) {
                    self.branch_delete(name)?;
                } else {
                    println!("⚠️ Missing branch name.");
                }
            }
            "rename" => {
                if parts.len() >= 4 {
                    self.branch_rename(parts[2], parts[3])?;
                } else {
                    println!("Usage: /branch rename <old> <new>");
                }
            }
            _ => self.print_branch_usage(),
        }
        Ok(())
    }

    fn print_branch_usage(&self) {
        println!("Usage: /branch [new|switch|list|current|delete|rename] <args>");
    }

    fn branch_new(&mut self, name: &str) -> Result<(), Box<dyn Error>> {
        if self.branches.contains_key(name) {
            println!("⚠️ Branch '{}' already exists.", name);
            return Ok(());
        }
        let mut new_branch = self.current.clone(); // inherit history
        new_branch.branch = name.to_string();
        self.branches.insert(name.to_string(), new_branch.clone());
        self.current = new_branch;
        self.save_current_branch()?;
        println!("🌱 Created and switched to '{}'", name);
        Ok(())
    }

    fn branch_switch(&mut self, name: &str) -> Result<(), Box<dyn Error>> {
        // save current first
        self.save_current_branch().ok();

        if let Some(existing) = self.branches.get(name).cloned() {
            self.current = existing;
            println!("🔀 Switched to '{}'", name);
            return Ok(());
        }
        // try load from file
        let path = self.file_path_for(&self.current.id, name);
        if path.exists() {
            let file = File::open(&path)?;
            let loaded: Session = serde_json::from_reader(file)?;
            self.branches.insert(name.to_string(), loaded.clone());
            self.current = loaded;
            println!("🔀 Switched to loaded '{}'", name);
        } else {
            println!("❌ Branch '{}' not found.", name);
        }
        Ok(())
    }

    fn branch_list(&self) {
        println!("🌿 Branches (loaded):");
        for k in self.branches.keys() {
            let star = if *k == self.current.branch { "*" } else { " " };
            println!("{} {}", star, k);
        }
    }

    fn branch_delete(&mut self, name: &str) -> Result<(), Box<dyn Error>> {
        if name == "main" {
            println!("⚠️ Cannot delete 'main'.");
            return Ok(());
        }
        if !self.branches.contains_key(name) {
            println!("⚠️ Branch '{}' not loaded; will still try to remove file if exists.", name);
        }
        if !ask_confirm(&format!("Confirm delete branch '{}'?", name))? {
            println!("❎ Cancelled.");
            return Ok(());
        }
        // remove from memory
        self.branches.remove(name);

        // remove file
        let path = self.file_path_for(&self.current.id, name);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        // if deleting current, switch to main if exists
        if self.current.branch == name {
            if let Some(main) = self.branches.get("main").cloned() {
                self.current = main;
                println!("↩️  Switched back to 'main'.");
            } else {
                // recreate an empty main to keep manager valid
                let main = Session {
                    id: self.current.id.clone(),
                    branch: "main".to_string(),
                    created_at: now_ts(),
                    messages: vec![],
                    summary: None,
                };
                self.branches.insert("main".to_string(), main.clone());
                self.current = main;
                println!("↩️  Recreated and switched to 'main'.");
            }
        }
        println!("🗑️ Deleted branch '{}'", name);
        Ok(())
    }

    fn branch_rename(&mut self, old: &str, new: &str) -> Result<(), Box<dyn Error>> {
        if old == "main" {
            println!("⚠️ Cannot rename 'main'.");
            return Ok(());
        }
        if !self.branches.contains_key(old) {
            println!("❌ Branch '{}' not found.", old);
            return Ok(());
        }
        if self.branches.contains_key(new) {
            println!("⚠️ Branch '{}' already exists.", new);
            return Ok(());
        }
        // rename file on disk if exists
        let old_path = self.file_path_for(&self.current.id, old);
        let new_path = self.file_path_for(&self.current.id, new);
        if old_path.exists() {
            fs::rename(&old_path, &new_path)?;
        }

        // update in memory
        let mut s = self.branches.remove(old).unwrap();
        s.branch = new.to_string();
        self.branches.insert(new.to_string(), s.clone());
        if self.current.branch == old {
            self.current = s;
        }
        println!("✏️  Renamed '{}' → '{}'", old, new);
        Ok(())
    }

    // -------------------------- Session Commands --------------------------
    pub fn handle_session_command(&mut self, input: &str) -> Result<(), Box<dyn Error>> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() < 2 {
            self.print_session_usage();
            return Ok(());
        }
        match parts[1] {
            "list" => self.session_list()?,
            "current" => println!("🆔 Current session id: {}", self.current.id),
            "delete" => {
                if let Some(id) = parts.get(2) {
                    self.session_delete(id)?;
                } else {
                    println!("Usage: /session delete <id>");
                }
            }
            _ => self.print_session_usage(),
        }
        Ok(())
    }

    fn print_session_usage(&self) {
        println!("Usage: /session [list|current|delete <id>]");
    }

    fn session_list(&self) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(SESS_DIR)?;
        let mut groups: HashMap<String, Vec<String>> = HashMap::new();
        for entry in fs::read_dir(SESS_DIR)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() { continue; }
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                // pattern: <id>_<branch>.json
                if let Some((id, rest)) = name.split_once('_') {
                    if rest.ends_with(".json") {
                        groups.entry(id.to_string()).or_default().push(rest.to_string());
                    }
                }
            }
        }
        if groups.is_empty() {
            println!("(no sessions)");
            return Ok(());
        }
        println!("📚 Sessions:");
        for (id, branches) in groups {
            println!("- {} ({} branches)", id, branches.len());
        }
        Ok(())
    }

    fn session_delete(&mut self, id: &str) -> Result<(), Box<dyn Error>> {
        // Collect files to delete
        let mut files = vec![];
        for entry in fs::read_dir(SESS_DIR)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() { continue; }
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

        if !ask_confirm(&format!("Confirm delete session '{}' ({} files)?", id, files.len()))? {
            println!("❎ Cancelled.");
            return Ok(());
        }

        for p in files {
            fs::remove_file(&p)?;
        }

        if self.current.id == id {
            // Re-initialize to a fresh session id
            *self = SessionManager::new(None);
            println!("↩️  Deleted current session. Switched to new session '{}'.", self.current.id);
        } else {
            println!("🗑️ Deleted session '{}'.", id);
        }
        Ok(())
    }

    // -------------------------- LLM Interaction --------------------------
    pub fn add_user_message(&mut self, content: String) {
        self.current.messages.push(Message { role: "user".into(), content });
    }

    pub fn send_and_stream_llm(&mut self, client: &Client, prompt: &str) -> Result<(), Box<dyn Error>> {
        // Build context (role: content) + current user prompt
        let history = self
            .current
            .messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");
        let full_prompt = if history.is_empty() {
            format!("user: {prompt}\nassistant:")
        } else {
            format!("{history}\nuser: {prompt}\nassistant:")
        };

        let response = client
            .post("http://localhost:11434/api/generate")
            .json(&serde_json::json!({
                "model": MODEL_NAME,
                "prompt": full_prompt,
                "stream": true
            }))
            .send()?;

        let answer = self.stream_collect_answer(response)?;
        self.current.messages.push(Message { role: "assistant".into(), content: answer.clone() });
        println!("");

        // Auto summarize after N dialog pairs
        let pairs = self.current.messages.len() / 2;
        if pairs >= SUMMARY_TRIGGER_PAIRS {
            println!("🧩 Reached {pairs} pairs. Generating summary...");
            self.generate_summary(client)?;
        }

        // Auto-save after each exchange
        self.save_current_branch().ok();
        Ok(())
    }

    fn stream_collect_answer(&self, response: Response) -> Result<String, Box<dyn Error>> {
        let reader = BufReader::new(response);
        let mut full = String::new();
        for line_res in reader.lines() {
            let line: String = line_res?;
            if let Ok(parsed) = serde_json::from_str::<OllamaResponse>(&line) {
                if let Some(text) = parsed.response {
                    print!("{text}");
                    io::stdout().flush()?;
                    full.push_str(&text);
                }
                if parsed.done.unwrap_or(false) {
                    break;
                }
            }
        }
        println!("\n✅ Done.");
        Ok(full)
    }

    fn generate_summary(&mut self, client: &Client) -> Result<(), Box<dyn Error>> {
        let history = self
            .current
            .messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let instruction = format!(
            "You are a helpful assistant. Write a concise summary (2-6 sentences) of the conversation below, focusing on key facts, decisions, and user preferences.\n\nConversation:\n{history}\n\nSummary:"
        );

        let response = client
            .post("http://localhost:11434/api/generate")
            .json(&serde_json::json!({
                "model": MODEL_NAME,
                "prompt": instruction,
                "stream": true
            }))
            .send()?;

        let summary = self.stream_collect_answer(response)?;
        let summary = summary.trim().to_string();

        if summary.is_empty() {
            return Ok(());
        }

        // Append or set summary
        self.current.summary = match &self.current.summary {
            Some(old) if !old.trim().is_empty() => Some(format!("{old}\n\n---\n{summary}")),
            _ => Some(summary),
        };

        // Save after summarizing
        self.save_current_branch()?;
        Ok(())
    }
}

// ========================= Utils =========================

fn new_id() -> String {
    now_ts().to_string()
}

fn now_ts() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

fn ask_confirm(prompt: &str) -> Result<bool, Box<dyn Error>> {
    print!("⚠️  {prompt} (y/n): ");
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    let ans = buf.trim().to_lowercase();
    Ok(ans == "y" || ans == "yes")
}
