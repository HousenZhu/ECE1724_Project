use crate::session::{Message, SessionManager};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::error::Error;
use std::io::Write;
use crate::api_key::DASHSCOPE_API_KEY;

pub fn call_chat_api(
    client: &Client,
    model: &str,
    messages: &[Value],
) -> Result<String, Box<dyn Error>> {

    // 直接使用来自独立文件的 Key
    let api_key = DASHSCOPE_API_KEY;

    let url = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";

    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": model,
            "messages": messages,
        }))
        .send()?;

    let status = resp.status();
    let body: Value = resp.json()?;

    if !status.is_success() {
        let msg = body
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error from API");
        return Err(format!("DashScope API error ({status}): {msg}").into());
    }

    Ok(body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

/// Implementation block for LLM-related functions.
impl SessionManager {
    pub fn send_and_stream_llm(
        &mut self,
        client: &Client,
        _prompt: &str,
    ) -> Result<(), Box<dyn Error>> {
        let messages: Vec<Value> = self
            .session
            .messages
            .iter()
            .map(|m| {
                json!({
                    "role": m.role,
                    "content": m.content,
                })
            })
            .collect();

        let answer = call_chat_api(client, &self.model, &messages)?;

        for ch in answer.chars() {
            print!("{ch}");
            std::io::stdout().flush().ok();
        }
        println!("\n✅ Done.");

        self.session.messages.push(Message {
            role: "assistant".into(),
            content: answer.clone(),
        });

        self.maybe_summarize(client)?;
        self.save_to_logs().ok();
        Ok(())
    }

    pub(crate) fn history_string(&self) -> String {
        self.session
            .messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn maybe_summarize(&mut self, client: &Client) -> Result<(), Box<dyn Error>> {
        const SUMMARY_TRIGGER_PAIRS: usize = 20;

        let pairs = self.session.messages.len() / 2;
        if pairs < SUMMARY_TRIGGER_PAIRS {
            return Ok(());
        }

        println!("🧩 {pairs} messages reached. Summarizing...");

        let history = self.history_string();

        let messages = vec![
            json!({
                "role": "system",
                "content": "You are a helpful assistant. Write a concise summary (2-6 sentences).",
            }),
            json!({
                "role": "user",
                "content": format!(
                    "Here is the conversation history:\n\n{}\n\nPlease summarize it.",
                    history
                ),
            }),
        ];

        let summary = call_chat_api(client, &self.model, &messages)?;

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
