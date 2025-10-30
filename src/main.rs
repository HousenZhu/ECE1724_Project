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

    if let Some(prompt) = args.ai_help {
        println!("🧠 Asking Gemma 3: {}\n", prompt);
        stream_ollama_response(&prompt)?;
    } else {
        println!("Run with --ai-help \"your question\"");
    }

    Ok(())
}

fn stream_ollama_response(prompt: &str) -> Result<(), Box<dyn Error>> {
    let client = Client::new();

    // Send POST request to local Ollama API
    let response = client
        .post("http://localhost:11434/api/generate")
        .json(&serde_json::json!({
            "model": "gemma3",
            "prompt": prompt,
            "stream": true
        }))
        .send()?;

    stream_response_lines(response)
}

fn stream_response_lines(response: Response) -> Result<(), Box<dyn Error>> {
    let reader = BufReader::new(response);
    for line_result in reader.lines() {
        let line = line_result?;
        if let Ok(parsed) = serde_json::from_str::<OllamaResponse>(&line) {
            if let Some(text) = parsed.response {
                print!("{text}");
                std::io::Write::flush(&mut std::io::stdout())?;
            }
            if parsed.done.unwrap_or(false) {
                break;
            }
        }
    }
    println!("\n\n✅ Done.");
    Ok(())
}
