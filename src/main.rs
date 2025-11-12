use reqwest::blocking::{Client, Response};
use serde::Deserialize;
use serde_json::Value;
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

/// Main agentic workflow: loop until final Done
fn run_agentic_session(initial_prompt: &str) -> Result<(), Box<dyn Error>> {
    let client = Client::new();

    // Conversation memory
    let mut conversation_log = String::new();

    // Initial system instruction
    let system_intro = format!(
        "You are an AI assistant with access to MCP tools.\n\
        Available tools:\n\
        - filesystem.read  — read file content. Example: <use_tool name=\"filesystem.read\" params={{\"path\": \"src/main.rs\"}} />\n\
        - filesystem.write — write text into a file. Example: <use_tool name=\"filesystem.write\" params={{\"path\": \"output.txt\", \"content\": \"Hello\"}} />\n\
        - shell.run — run shell commands. Example: <use_tool name=\"shell.run\" params={{\"content\": \"mkdir Playground\"}} />\n\
        Notice that those commands working on windows system. Try add /q if necessary. \n\
        When using a tool, use EXACTLY this XML-style syntax. \n\
        you can add some explaining information after a tool call, but take care of format for readability. \n\
        You can use **only one <use_tool> command per message.** If you need to use multiple tools, call them one by one — wait for the tool's result before issuing the next <use_tool>. \n\
        You can use **only one <use_tool> command per message.** \n\
        You can use **only one <use_tool> command per message.** \n\
        When you are done, end your final output with 'Done.'\n\n"
    );

    loop {
        // Rebuild full prompt each iteration
        let full_prompt = format!(
            "{system_intro}\
            User initial prompt:\n{initial_prompt}\n\n\
            Conversation so far:\n{conversation_log}\n\n\
            Continue reasoning or issue next tool call if needed.",
            system_intro = system_intro,
            initial_prompt = initial_prompt,
            conversation_log = conversation_log
        );

        // Send request to model
        let response = client
            .post("http://localhost:11434/api/generate")
            .json(&serde_json::json!({
                "model": "qwen3:8b",
                "prompt": full_prompt,
                "stream": true
            }))
            .send()?;

        // Stream and collect model output
        let output = stream_response_lines(response)?;

        // Add to conversation log
        conversation_log.push_str("\n[Model]: ");
        conversation_log.push_str(&output);

        // Check if model used a tool
        if let Some(tool_call) = parse_tool_use(&output) {
            println!("\n⚙️  Detected MCP command: {:?}\n", tool_call);
            let result = execute_mcp(&tool_call)?;

            // Append tool output to log for context
            conversation_log.push_str(&format!(
                "\n[Tool: {} result]\n{}\n",
                tool_call.name, result
            ));
        } else {
            // No further tools, likely done
            println!("✅ No further tool use detected — session complete.");
            // Also check for explicit "Done." in model output
            if output.to_lowercase().contains("done.") {
                println!("🏁 Model signaled completion.\n");
            }
            break;
        }

        // Check for “Done” inside the model output; if present, break loop
        if output.to_lowercase().contains("done.") {
            println!("🏁 Model signaled completion.\n");
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

/// Struct for parsed tool info
#[derive(Debug)]
struct ToolCall {
    name: String,
    path: Option<String>,
    content: Option<String>,
}

/// Parse MCP-style tool command from model output
/// This function first tries to capture the full params object and parse it as JSON (robust).
/// If JSON parsing fails, it falls back to older simpler regex patterns.
fn parse_tool_use(output: &str) -> Option<ToolCall> {
    // First try to capture the whole params JSON object (dot matches newlines with (?s))
    let re_full = Regex::new(r#"(?s)<use_tool\s+name="([^"]+)"\s+params=(\{.*?\})\s*/?>"#).ok()?;
    if let Some(caps) = re_full.captures(output) {
        let name = caps.get(1)?.as_str().to_string();
        let params_str = caps.get(2)?.as_str();

        // Try parsing params as JSON
        if let Ok(json_val) = serde_json::from_str::<Value>(params_str) {
            let path = json_val.get("path").and_then(|v| v.as_str()).map(|s| s.to_string());
            let content = json_val.get("content").and_then(|v| v.as_str()).map(|s| s.to_string());
            return Some(ToolCall { name, path, content });
        }
        // If parsing failed, continue to fallback regexes below
    }

    // Fallback: simple read command
    let re_read = Regex::new(
        r#"<use_tool\s+name="filesystem\.read"\s+params=\{\s*"path":\s*"([^"]+)"\s*\}\s*/?>"#
    ).ok()?;

    if let Some(caps) = re_read.captures(output) {
        return Some(ToolCall {
            name: "filesystem.read".into(),
            path: Some(caps[1].to_string()),
            content: None,
        });
    }

    // Fallback: simple write command (content without escaped quotes might be captured)
    let re_write = Regex::new(
        r#"<use_tool\s+name="filesystem\.write"\s+params=\{\s*"path":\s*"([^"]+)"\s*,\s*"content":\s*"([^"]*)"\s*\}\s*/?>"#
    ).ok()?;

    if let Some(caps) = re_write.captures(output) {
        return Some(ToolCall {
            name: "filesystem.write".into(),
            path: Some(caps[1].to_string()),
            content: Some(caps[2].to_string()),
        });
    }

    // Fallback: shell.run (content = command)
    let re_shell = Regex::new(
        r#"<use_tool\s+name="shell\.run"\s+params=\{\s*"content":\s*"([^"]+)"\s*\}\s*/?>"#
    ).ok()?;

    if let Some(caps) = re_shell.captures(output) {
        return Some(ToolCall {
            name: "shell.run".into(),
            path: None,
            content: Some(caps[1].to_string()),
        });
    }

    None
}

/// Simulate MCP tools (filesystem.read, filesystem.write, shell.run)
fn execute_mcp(tool: &ToolCall) -> Result<String, Box<dyn Error>> {
    match tool.name.as_str() {
        "filesystem.read" => {
            let path = tool.path.as_ref().ok_or("Missing path for filesystem.read")?;
            let content = fs::read_to_string(path)?;
            println!("📂 Read file '{}': {} bytes", path, content.len());
            Ok(format!("Read file '{}' ({} bytes). Content:\n{}", path, content.len(), content))
        }

        "filesystem.write" => {
            let path = tool.path.as_ref().ok_or("Missing path for filesystem.write")?;
            let data_raw = tool.content.as_ref().ok_or("Missing content for filesystem.write")?;
            let data = normalize_escaped_content(data_raw);

            fs::write(path, &data)?;
            println!("💾 Wrote {} bytes to '{}'", data.len(), path);
            Ok(format!("Wrote {} bytes to '{}'.", data.len(), path))
        }

        "shell.run" => {
            let command_raw = tool
                .content
                .as_ref()
                .ok_or("Missing 'content' for shell.run (expected shell command)")?;
            println!("🖥️ Running shell command: `{}`", command_raw);

            #[cfg(target_os = "windows")]
            let output = std::process::Command::new("cmd")
                .args(&["/C", command_raw])
                .output()?;

            #[cfg(not(target_os = "windows"))]
            let output = std::process::Command::new("sh")
                .args(&["-c", command_raw])
                .output()?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            println!("📤 Command output:\n{}", stdout);
            if !stderr.is_empty() {
                println!("⚠️ Command error output:\n{}", stderr);
            }

            Ok(format!(
                "Command `{}` executed.\nSTDOUT:\n{}\nSTDERR:\n{}",
                command_raw, stdout, stderr
            ))
        }

        _ => Err(format!("Unknown MCP tool: {}", tool.name).into()),
    }
}


/// Loosely decode escaped sequences and handle real newlines safely
fn normalize_escaped_content(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('n') => { out.push('\n'); chars.next(); }
                Some('t') => { out.push('\t'); chars.next(); }
                Some('r') => { out.push('\r'); chars.next(); }
                Some('\\') => { out.push('\\'); chars.next(); }
                Some('"') => { out.push('"'); chars.next(); }
                Some('\'') => { out.push('\''); chars.next(); }
                _ => out.push(c),
            }
        } else {
            out.push(c);
        }
    }

    out
}
