use reqwest::blocking::{Client, Response};
use serde::Deserialize;
use std::error::Error;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use structopt::StructOpt;
use regex::Regex;

#[derive(StructOpt, Debug)]
#[structopt(name = "mycli", about = "Rust CLI using Ollama + MCP")]
struct Cli {
    /// Ask the local AI with MCP tool support
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
        println!("🧠 Asking qwen3:8b (with MCP tools): {}\n", prompt);
        run_agentic_session(&prompt)?;
    } else {
        println!("Run with --ai-help \"your question\"");
    }

    Ok(())
}

/// Main agentic workflow: prompt -> check MCP -> re-prompt until final Done
fn run_agentic_session(initial_prompt: &str) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let mut current_prompt = format!(
        "You are an AI with access to MCP tools.\n\
        Example: <use_tool name=\"filesystem.read\" params={{\"path\": \"src/main.rs\"}} />\n\
        Always use this format exactly when you need file data.\n\
        If you already have the file content, don't ask again.\n\
        User prompt: {}",
        initial_prompt
    );

    loop {
        // Send request to Ollama
        let response = client
            .post("http://localhost:11434/api/generate")
            .json(&serde_json::json!({
                "model": "qwen3:8b",
                "prompt": current_prompt,
                "stream": true
            }))
            .send()?;

        // Stream and collect output
        let output = stream_response_lines(response)?;

        // Check if “Done” keyword appears → exit
        if output.to_lowercase().contains("done.") || output.to_lowercase().contains("✅ done") {
            println!("🏁 Model signaled completion.\n");
            break;
        }

        // Parse tool request if any
        if let Some((tool_name, path)) = parse_tool_use(&output) {
            println!("\n⚙️  Detected MCP command: {} → {}\n", tool_name, path);
            let file_content = execute_mcp(&tool_name, &path)?;

            // Construct follow-up prompt
            current_prompt = format!(
                "The tool {tool_name} returned this content from {path}:\n\
                ----- FILE START -----\n\
                {file_content}\n\
                ----- FILE END -----\n\
                Continue your previous reasoning and complete your answer.\n\
                If you are done, end with 'Done.'"
            );
        } else {
            // No tools needed, assume model completed
            println!("✅ No further tool use detected — session complete.");
            break;
        }
    }

    Ok(())
}

/// Streams Ollama responses line by line and returns the combined text
fn stream_response_lines(response: Response) -> Result<String, Box<dyn Error>> {
    let reader = BufReader::new(response);
    let mut buffer = String::new();

    for line_result in reader.lines() {
        let line = line_result?;
        if let Ok(parsed) = serde_json::from_str::<OllamaResponse>(&line) {
            if let Some(text) = parsed.response {
                print!("{text}");
                std::io::stdout().flush()?;
                buffer.push_str(&text);
            }
            if parsed.done.unwrap_or(false) {
                break;
            }
        }
    }
    println!("\n");
    Ok(buffer)
}

/// Parse MCP-style tool command from model output
fn parse_tool_use(output: &str) -> Option<(String, String)> {
    let re = Regex::new(
        r#"<use_tool\s+name="([^"]+)"\s+params=\{\s*"path":\s*"([^"]+)"\s*\}\s*/?>"#
    ).ok()?;

    if let Some(caps) = re.captures(output) {
        let name = caps.get(1)?.as_str().to_string();
        let path = caps.get(2)?.as_str().to_string();
        return Some((name, path));
    }
    None
}

/// Simulate MCP tool (filesystem.read)
fn execute_mcp(tool_name: &str, path: &str) -> Result<String, Box<dyn Error>> {
    match tool_name {
        "filesystem.read" => {
            let content = fs::read_to_string(path)?;
            println!("📂 Read file '{}': {} bytes", path, content.len());
            Ok(content)
        }
        _ => Err(format!("Unknown MCP tool: {}", tool_name).into()),
    }
}
