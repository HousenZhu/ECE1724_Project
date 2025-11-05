mod session;
use session::{MemorySession, Role};

use reqwest::blocking::{Client, Response};
use serde::Deserialize;
use std::error::Error;
use std::io::{BufRead, BufReader};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "mycli", about = "Rust CLI using local Ollama model")]
struct Cli {
    /// Ask Ollama AI for help (e.g. --ai-help "Explain Rust ownership")
    #[structopt(long, help = "Ask the local Ollama model for help")]
    ai_help: Option<String>,
}

#[derive(Deserialize, Debug)]
struct OllamaResponse {
    response: Option<String>,
    done: Option<bool>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::from_args();
    let mut session = MemorySession::new();
    let client = Client::new();

    if let Some(prompt) = args.ai_help {
        println!("Asking Gemma 3: {}\n", prompt);
        session.add(Role::User, prompt);
        let _ = stream_ollama_response(&client, &mut session)?;
        println!("\n---");
    }

    loop {
        print!("> ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            continue;
        }
        if input == "quit" || input == "exit" {
            println!("Goodbye!");
            break;
        }

        session.add(Role::User, input.to_string());

        println!("Thinking...");
        let _ = stream_ollama_response(&client, &mut session)?;
        println!();
    }

    Ok(())
}

fn stream_ollama_response(client: &Client, session: &mut MemorySession) -> Result<String, Box<dyn Error>> {
    let prompt = session.build_prompt();

    let response = client
        .post("http://localhost:11434/api/generate")
        .json(&serde_json::json!({
            "model": "gemma3",
            "prompt": prompt,
            "stream": true
        }))
        .send()?;

    let mut full_response = String::new();
    stream_response_lines(response, |text| {
        print!("{}", text);
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
        full_response.push_str(text);
    })?;

    if !full_response.trim().is_empty() {
        session.add(Role::Assistant, full_response.clone());
    }

    Ok(full_response)
}

fn stream_response_lines<F>(response: Response, mut on_chunk: F) -> Result<(), Box<dyn Error>>
where
    F: FnMut(&str),
{
    let reader = BufReader::new(response);
    for line_result in reader.lines() {
        let line = line_result?;
        if let Ok(parsed) = serde_json::from_str::<OllamaResponse>(&line) {
            if let Some(text) = parsed.response {
                on_chunk(&text);
            }
            if parsed.done.unwrap_or(false) {
                break;
            }
        }
    }
    Ok(())
}