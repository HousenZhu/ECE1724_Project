use crate::session::{Message, SessionManager};
use reqwest::blocking::{Client, Response};
use serde::Deserialize;
use std::error::Error;
use std::io::{BufRead, BufReader, Write};

/// Streaming response chunk from the LLM.
#[derive(Deserialize, Debug)]
struct OllamaResponse {
    response: Option<String>,
    done: Option<bool>,
}

/// Implementation block for LLM-related functions.
impl SessionManager {
    /// Send user prompt, stream LLM output, append to history, maybe summarize, then save.
    pub fn send_and_stream_llm(&mut self, client: &Client, prompt: &str) -> Result<(), Box<dyn Error>> {
        let history = self.history_string();
        let full_prompt = if history.is_empty() {
            format!("user: {prompt}\nassistant:")
        } else {
            format!("{history}\nuser: {prompt}\nassistant:")
        };

        let response = client
            .post("http://localhost:11434/api/generate")
            .json(&serde_json::json!({
                "model": self.model,
                "prompt": full_prompt,
                "stream": true
            }))
            .send()?;

        let answer = self.stream_collect(response)?;
        self.session.messages.push(Message {
            role: "assistant".into(),
            content: answer,
        });

        self.maybe_summarize(client)?;
        self.save_to_logs().ok();
        Ok(())
    }

    /// Build conversation history as a prompt string.
    fn history_string(&self) -> String {
        self.session
            .messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Collect streaming chunks line by line.
    fn stream_collect(&self, response: Response) -> Result<String, Box<dyn Error>> {
        let reader = BufReader::new(response);
        let mut full = String::new();

        for line in reader.lines() {
            let text = line?;
            if let Ok(parsed) = serde_json::from_str::<OllamaResponse>(&text) {
                if let Some(chunk) = parsed.response {
                    print!("{chunk}");
                    std::io::stdout().flush()?;
                    full.push_str(&chunk);
                }
                if parsed.done.unwrap_or(false) {
                    break;
                }
            }
        }
        println!("\n✅ Done.");
        Ok(full)
    }

    /// If enough messages exist, ask LLM to summarize them.
    fn maybe_summarize(&mut self, client: &Client) -> Result<(), Box<dyn Error>> {
        const SUMMARY_TRIGGER_PAIRS: usize = 20;

        let pairs = self.session.messages.len() / 2;
        if pairs < SUMMARY_TRIGGER_PAIRS {
            return Ok(());
        }

        println!("🧩 {pairs} messages reached. Summarizing...");

        let history = self.history_string();
        let instruction = format!(
            "You are a helpful assistant. Write a concise summary (2-6 sentences):\n\n{history}\n\nSummary:"
        );

        let response = client
            .post("http://localhost:11434/api/generate")
            .json(&serde_json::json!({
                "model": self.model,
                "prompt": instruction,
                "stream": true
            }))
            .send()?;

        let summary = self.stream_collect(response)?;
        if !summary.trim().is_empty() {
            self.session.summary = match &self.session.summary {
                Some(old) => Some(format!("{old}\n\n---\n{summary}")),
                None => Some(summary),
            };
        }

        self.save_to_logs().ok();
        Ok(())
    }
}
