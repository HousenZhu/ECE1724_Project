use reqwest::blocking::Client;
use std::error::Error;
use std::io::{self, Write};

mod session;
mod llm;
mod mcp;
mod api_key;

use session::SessionManager;

fn main() -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let mut manager = SessionManager::new();

    println!("╔══════════════════════════════════════════╗");
    println!("║ 🤖  Rust Cloud AI Console (Chat Client)   ║");
    println!("╚══════════════════════════════════════════╝");
    println!("  Model in use  :  {}", manager.model);
    println!("  Switch model  :  /use <model-name>");
    println!("  Help menu     :  /help");
    println!("  Exit          :  /quit\n");
    println!("💬 Start typing below:\n");

    loop {
        print!(
            "\x1b[1;36m{}\x1b[0m-\x1b[35m{}/{}\x1b[0m> ",
            manager.model,
            manager.session.id,
            manager.session.branch
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        // -------- Command handling --------
        if input.starts_with('/') {
            match input {
                "/quit" => {
                    println!("👋 Bye!");
                    break;
                }
                "/help" => print_help(),
                "/use" => println!("📌 Current model: {}", manager.model),

                x if x.starts_with("/use ") => {
                    let name = x.split_whitespace().nth(1).unwrap();
                    manager.model = name.to_string();
                    println!("🔄 Model switched to '{}'", name);
                }

                x if x.starts_with("/mcp ") => {
                    let prompt = x.strip_prefix("/mcp ").unwrap().trim();
                    if let Err(e) = manager.handle_mcp_command(prompt) {
                        eprintln!("❌ MCP Agent Error: {e}");
                    }
                }

                "/session clear" => manager.clear_all_sessions(),

                "/branch clear" => manager.clear_other_branches(),

                x if x.starts_with("/save") => {
                    if let Err(e) = manager.save_to_logs() {
                        eprintln!("❌ Save error: {e}");
                    }
                }

                x if x.starts_with("/load ") => {
                    let id = x.split_whitespace().nth(1);
                    if let Err(e) = manager.load_session(id) {
                        eprintln!("❌ Load error: {e}");
                    }
                }

                x if x.starts_with("/branch") || x.starts_with("/b ") => {
                    if let Err(e) = manager.handle_branch_command(x) {
                        eprintln!("❌ Branch error: {e}");
                    }
                }

                x if x.starts_with("/session") => {
                    if let Err(e) = manager.handle_session_command(x) {
                        eprintln!("❌ Session error: {e}");
                    }
                }

                _ => println!("⚠️ Unknown command. Use /help."),
            }
            continue;
        }

        // -------- Regular chat message --------
        manager.session.messages.push(session::Message {
            role: "user".into(),
            content: input.to_string(),
        });

        if let Err(e) = manager.send_and_stream_llm(&client, input) {
            eprintln!("❌ Request failed: {e}");
        }
    }

    Ok(())
}

/// Help menu
fn print_help() {
    println!(
        r#"
Commands
========
Model:
  /use                 Show current model
  /use <model>         Switch to another model

Session:
  /session list             Show stored sessions
  /session current          Show current session ID
  /session delete <id>      Delete a session
  /session clear            Remove ALL sessions

Branch:
  /branch new <name>        Create new branch
  /branch switch <name>     Switch branch
  /branch list              Show branches
  /branch current           Show current branch
  /branch delete <name>     Delete a branch
  /branch rename <old> <new> Rename a branch
  /branch clear             Delete all branches except 'main'

General:
  /save                     Save current branch
  /load <session_id>        Load saved session
  /help                     Show help
  /quit                     Exit

Notes:
- History saved in logs/<session>_<branch>.json
- Model context persists unless session/branch is cleared.
"#
    );
}
