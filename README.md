# ECE1724 Project Proposal: Simple LLM-Powered CLI

Housen Zhu 1008117477

Chufan Ju 1011668063

Tianqi Ju 1012870467

---

## Motivation
Large Language Models (LLMs) are increasingly being integrated into everyday developer workflows including code generation and debugging. Also, LLMs can operate as agents on managing multi-step workflows and interacting with external tools. As one of the most widely used environments, Command-line interfaces (CLIs) are especially suitable for such integration for developers and other technical users valuing its speed and flexibility. A CLI powered by LLMs enables developers to interact with local model inference in their existing workflows with low latency.

While Python dominates the LLM tooling, it comes with some trade-offs (runtime overhead and limited guarantees of safety and concurrency). In contrast, Rust provides strong memory safety and predictable performance. However, despite these strengths, the Rust ecosystem for LLM-powered applications like a CLI remains underdeveloped in some areas.

Protocols such as the Model Context Protocol (MCP) enables LLMs to invoke external tools safely and effectively), where robust Rust integrations remain scarce. Also, CLI-based projects in Rust only offer prompt-based functionality and rarely go beyond simple REPLs (Read–Eval–Print Loop). 

By addressing these gaps, we aim to create a LLM-powered CLI that supports agentic workflows, protocol integration and context-aware sessions in Rust, enabling developers to automate tasks by using LLMs directly in their terminal without leaving their workflow. By serving with local model inference, it increases privacy and control, reduces dependence on cloud APIs or external services.

---

## Objective

The objective of this project is to design and implement a simple, Rust-based command-line interface (CLI) powered by large language models (LLMs). The system allows users to interact with language models through a text-based user interface, providing real-time streaming responses and maintaining context across multiple interactions. By organizing conversations into sessions and branches, the CLI enables structured, context-aware dialogue and flexible exploration of different interaction paths.

In addition, the project aims to support agentic workflows and external tool integration through a lightweight protocol, enabling the model to perform tasks such as file operations and shell command execution when appropriate. The overall goal is to create a lightweight, efficient, and extensible terminal-based assistant that enhances developer productivity while remaining easy to use and adaptable to future extensions.

---

## Features  

1. **Session Management**  
    - Maintain conversational history across multiple turns using a local database.
        - Save user prompts, LLM answers, and also metadata such as timestamps.
    - Support saving and loading sessions to continue work later.
        - Serialize sessions to JSON with `serde` for cross-platform compatibility.
        - Commands: `/save <session_id>` and `/load <session_id>` using Clap for parsing.
    - Allow branching task histories to switch between project contexts.
        - Implement a tree-like structure with `HashMap<String`, `Vec<Message>>`.
        - Command: `/branch <name>` to fork a session (e.g., debugging vs. testing).
    - Prevent token overflow by summarizing long histories automatically.
        - Use LLM to generate summaries (prompt: "Summarize this session").
    - Track session metadata (e.g., tags, creation time) for easy querying.
    
    Session Management allows users to keep the conversation even after they close the terminal, which makes it easier for them to continue from the point they stopped. In contrast, simple Rust CLI tools like `llm-rs` do not provide persistent context. This  is especially useful for developers who often change between different tasks.

2. **Agentic Workflow Execution**  
   - Decompose complex user instructions into subtasks.
       - Parse LLM output as JSON plans with `serde_json`.
       - Limit to 5 steps to ensure completion.
   - Run a ReAct loop (Reason-Act-Observe) for iterative task execution.
       - Reason: LLM decides next step based on prior output.
       - Act: Execute actions like shell commands or tool calls.
       - Observe: Return results to LLM for further reasoning.
   - Safely execute shell commands and capture outputs.
       - Use `std::process::Command` in a sandbox.
   - Support automation loops for multi-step tasks.
       - Example: "Fix bugs in `main.rs`" runs analysis, edits, and tests.
       - Use `tokio::spawn` for async execution.
   - Visualize workflow steps in the TUI or console.
   - Offer verbose mode (`--verbose`) to show raw LLM reasoning.

    Agentic Workflow Execution can divide a task into smaller steps, call tools when needed, and repeat the process until the task is finished. In comparison, many Rust CLI tools can only work with single prompts and cannot manage complex workflows. Our system, based on ReAct, can automate tasks such as code refactoring. It runs locally, safely, and efficiently, and it uses Rust’s `tokio` for asynchronous execution.

3. **Tool Integration (MCP/ACP)**  
   - Implement support for the Model Context Protocol (MCP) and Agent Client Protocol (ACP).  
       - MCP: Dynamically discover available tools exposed by MCP servers at runtime.
       - ACP: Allow the CLI to interact with external editors (e.g., Cursor, Zed) for code editing, formatting, or build tasks.
   - Invoke external editors or services safely (e.g., code formatting, file editing, or data lookup).  
       - Execute tool calls as structured JSON requests.
       - Capture tool outputs and feed them back into the conversation loop.
   - Display tool results directly in the CLI session.  
       - Show both the invoked command/tool and its results in the TUI.
       - Maintain transparency for users to review what the agent executed.
    - Provide a unified tool abstraction layer.
        - Define a common Tool trait for all tools.
        - Allow newly discovered MCP/ACP tools to plug in without modifying the core agent loop.

    Tool Integration ensures the CLI is not just a simple chat interface, but a flexible automation hub. Unlike typical Rust LLM CLIs that only handle prompts, our system bridges LLM reasoning with external developer tools and editors, enabling richer agentic workflows.

4. **Local Model Inference**  
   - Enable offline usage and lightweight CPU usage by implementing inference backends in Rust.
        - support GPU acceleration if achievable.
    - Support streaming (token-by-token) response display, while ensuring low latency.
    - allow users to select different models at runtime (e.g., model llama2) depending on task requirements.
    - Implement error handling if a model is unavailable or fails to interact.
    - Ensure inference API works well for the agentic workflow and session manager to interact with models consistently.
    - Record the inference performance statistics (e.g., tokens per second) for debugging and optimization.
    
     Local Model Inference allows users to run the LLM-powered CLI offline and keep the data in privacy. Because of Rust’s high performance and strong safety guarantees, the implementation could prioritize running speed while ensuring stability. Users would be flexible to choose appropriate models that fit their personal tasks.


5. **Text User Interface (TUI)**  
 
   - Support scrolling through conversation history, so users can navigate long sessions.
    - Support command navigation to browse history (e.g., keyboard shortcuts:↑/↓).
    - Support copy/paste the outputs and commands.
    - Display important information in a sidebar.
        - Current session ID.
        - Active tools via MCP/ACP.
        - Error or status messages.
    - Support some keyboard shortcuts for session management
        - /new to start a new session.
        - /switch <id> to move between sessions.
    - Display the visual indicators for agentic workflow stages (Reasoning, Acting, Observing) in the loop.
    
    The TUI makes the CLI more than a simple REPL with structured displays and useful shortcuts for managing multi-step workflows. Users can interact with models seamlessly from visibility into the agent’s reasoning process.


---

## Tentative Plan  

### System Overview  

The system pipeline:  

```text
+-----------------+        +-----------------+        +---------------------+        +------------------+
| User Input (CLI)| -----> | Session Manager | -----> | Agent Workflow Layer| -----> | Inference Engine |
+-----------------+        +-----------------+        +---------------------+        +------------------+
        |                           |                          |                               |
        |                           |                          |                               v
        |                           |                          |                   +---------------------+
        |                           |                          |                   |  External Tools     |
        |                           |                          |                   | (MCP/ACP Servers)  |
        |                           |                          |                   +---------------------+
        |                           |                          |                               |
        v                           v                          v                               v
+------------------------------------------------------------------------------------------------------+
|                                       Text User Interface (TUI)                                      |
|          Displays conversation history, tool outputs, agent reasoning steps, and final responses.    |
+------------------------------------------------------------------------------------------------------+
```


### Team Responsibilities  

| Task / Feature                          | Housen Zhu (Inference) | Chufan Ju (Session & Workflow) | Tianqi Ju (UI & Integration) |
|-----------------------------------------|----------------------|-------------------------------|-----------------------------|
| Set up local inference with Ollama      | ✅                    |                               |                             |
| implement Rust inference backends       | ✅                 |                               |                               |
| Implement inference API for CLI         | ✅                    |                               |                             |
| Support streaming token-by-token output | ✅                    |                               |                             |
| Session context management              |                      | ✅                             |                             |
| Save/restore sessions                   |                      | ✅                             |                             |
| Branching task histories                |                      | ✅                             |                             |
| Implement agentic workflow decomposition |                      | ✅                             |                             |
| MCP protocol integration                |                      | ✅                             |                             |
| ACP protocol integration                |                      | ✅                             |                             |
| Tool discovery via MCP servers          |                      | ✅                             |                             |
| Tool invocation & result handling       |                      | ✅                             |                             |
| Build CLI with Ratatui                  |                      |                               | ✅                           |
| Input/output panes (prompts & responses)|                      |                               | ✅                           |
| Scrolling history & navigation          |                      |                               | ✅                           |
| Display session state & tool results    |                      |                               | ✅                           |
| Error handling & status messages        |                      |                               | ✅                           |
| Keyboard shortcuts for context/session  |                      |                               | ✅                           |
| System integration (all modules)        |                      |                               | ✅                           |
| Testing & debugging                     | ✅                    | ✅                             | ✅                           |
| Documentation                           | ✅                    | ✅                             | ✅                           |

---
    
### Member Descriptions

#### Housen Zhu (Inference):
Housen Zhu will manage local model inference setup with Ollama and explore Rust-based inference backends in first 3 weeks. He will implement inference API for CLI and facilitate streaming output in following 3 weeks. In the final 2 weeks, He will help with system testing and documentation.
    
#### Chufan Ju (Session & Workflow):
Chufan Ju will work on building the session management and agent workflow components. These parts will allow the system to keep conversation history and also perform tasks in automatic way. The expected workload is about 550 lines of code during 3 weeks. With these functions, our CLI becomes more advanced and unique in the Rust ecosystem, which can improve the productivity of developers.
    
#### Tianqi Ju (UI & Integration):
Tianqi Ju will build the text user interface using Ratatui, including input/output panes, scrolling history, navigation, and displaying session state and tool results. She’ll also connect the tool system through MCP/ACP, handle errors, and add handy keyboard shortcuts for managing sessions. When everything’s ready, she’ll help bring all the modules together, test the whole system, and make sure the documentation is clear and complete.

