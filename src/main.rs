use reqwest::blocking::Client;
use std::error::Error;
use std::io::{self, Write};
use structopt::StructOpt;

mod session;
use session::SessionManager;


#[derive(StructOpt, Debug)]
#[structopt(name = "mycli", about = "Rust CLI using local Ollama (gemma3) with sessions/branches")]
struct Cli {
    /// Start with a specific session id (optional). If omitted, a new id is created.
    #[structopt(long, help = "Specify a session ID to start with")]
    session_id: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::from_args();
    let client = Client::new();
    let mut manager = SessionManager::new(args.session_id);

    println!("🧠 Rust CLI Chat (Gemma 3)");
    println!("Type your message or use /help for commands.\n");

    loop {
        print!("> ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        // Command message
        if input.starts_with('/') {
            if input == "/quit" {
                println!("👋 Bye!");
                break;
            } else if input == "/help" {
                print_help();
            } else if input.starts_with("/save") {
                if let Err(e) = manager.save_current_branch() {
                    eprintln!("❌ Save error: {e}");
                }
            } else if input.starts_with("/load ") {
                let id = input.split_whitespace().nth(1);
                if let Err(e) = manager.load_session(id) {
                    eprintln!("❌ Load error: {e}");
                }
            } else if input.starts_with("/branch") || input.starts_with("/b ") {
                if let Err(e) = manager.handle_branch_command(input) {
                    eprintln!("❌ Branch command error: {e}");
                }
            } else if input.starts_with("/session") {
                if let Err(e) = manager.handle_session_command(input) {
                    eprintln!("❌ Session command error: {e}");
                }
            } else {
                println!("⚠️ Unknown command. Try /help.");
            }
            continue;
        }

        // Normal chat message
        manager.add_user_message(input.to_string()); // Add msg
        if let Err(e) = manager.send_and_stream_llm(&client, input) { //Send msg
            eprintln!("❌ Request failed: {e}");
        }
    }

    Ok(())
}

fn print_help() {
    println!(
        r#"
Commands
========
Session:
  /session list             List all session ids (and branch counts)
  /session current          Show current session id
  /session delete <id>      Delete a session (all its branches) with confirmation

Branch:
  /branch new <name>        Create a new branch (inherits current history)
  /branch switch <name>     Switch to an existing branch (load from file if needed)
  /branch list              List all known branches (loaded in memory)
  /branch current           Show current branch name
  /branch delete <name>     Delete a branch (with confirmation). Falls back to 'main' if current is deleted
  /branch rename <old> <new> Rename a branch (also rename the file if exists)

General:
  /save                     Save current branch to disk
  /load <session_id>        Load session (loads 'main' branch by default if exists)
  /help                     Show this help
  /quit                     Exit the program

Notes:
- Files are stored as: sessions/<session_id>_<branch>.json
- Conversation context is sent on every request.
- Auto-summarize triggers after 20 dialog pairs (user+assistant counted as one pair).
"#
    );
}



// INSTRUCTIONS!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
// 1. Cargo run
// 2. Type in msg.
// 3. Maybe some command (see in help)