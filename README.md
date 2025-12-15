# ECE1724 Project Proposal: Simple LLM-Powered CLI

| Name       | Student Number | Preferred Email              |
|------------|----------------|------------------------------|
| Housen Zhu | 1008117477     |                              |
| Chufan Ju  | 1011668063     | ethan.ju@mail.utoronto.ca    |
| Tianqi Ju  | 1012870467     | tianqi.ju@mail.utoronto.ca   |

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

## System Overview  

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

---

## Features  

1. **Session Management**  
    - Maintain conversational history across multiple turns within a session.
        - Store user inputs and assistant responses in an ordered message list.
    - Organize conversations into multiple independent sessions.
        - Each session maintains its own isolated context.
        - Users can switch between sessions without losing progress.
    - Support branching conversation histories within a session.
        - Each session can contain multiple branches representing different interaction paths.
        - Branches are isolated and do not interfere with each other.
    - Allow users to fork a new branch by editing a previous user message.
        - Editing a message creates a new branch starting from that point.
    - Persist conversation logs locally for inspection and reproducibility.
        - Serialize session branches into JSON files using `serde`.    

    Session Management allows users to maintain structured and long-running conversations rather than isolated prompts. By organizing interactions into sessions and branches, users can revisit earlier inputs, explore alternative reasoning paths, and work on multiple tasks in parallel. Compared to simple Rust-based LLM CLIs, this design provides stronger context awareness and greater flexibility for complex workflows.

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
   - Visualize workflow steps in the TUI or console.

    Agentic Workflow Execution can divide a task into smaller steps, call tools when needed, and repeat the process until the task is finished. In comparison, many Rust CLI tools can only work with single prompts and cannot manage complex workflows. Our system, based on ReAct, can automate tasks such as code refactoring.

3. **Tool Integration (MCP/ACP)**  
   - Implement support for the Model Context Protocol (MCP) 
       - MCP: Dynamically discover available tools exposed by MCP servers at runtime.

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

4. **Online Model Inference**
   - Enable model inference through online LLM APIs.
       - Send user prompts and conversation context to remote model endpoints.
       - Receive generated responses from the API.
   - Support real-time streaming of model responses.
       - Improve responsiveness and user experience during long generations.
   - Allow flexible model selection through API configuration.
       - Specify different models depending on task requirements.
   - Integrate API-based inference smoothly with session management.
       - Include session context and message history in each API request.
       - Ensure responses remain consistent with the active session.
   
   Online model inference allows the system to leverage powerful language models without requiring local model deployment. By using API-based inference, the CLI remains lightweight and easy to set up, while still supporting context-aware conversations and agentic workflows. Compared to local-only inference, this approach simplifies deployment and improves model availability at the cost of relying on external services.


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

## User's Guide

This section explains how to use the main features of the deliverable through the terminal user interface (TUI). The system supports both keyboard interaction and mouse-based clicking。

### Launch and Basic Navigation

After launch, the screen is divided into:
- Message area: shows conversation history (user + assistant).
- Input area: where prompts are typed (in insert mode).
- Sidebar (if enabled): session list and metadata.
- interactive UI elements (e.g., buttons): can be activated via keyboard or mouse click.

![alt text](screen.png)

### Sidebar, Help, and Exit
- Toggle sidebar:
	- Keyboard: s
	- Mouse: click the sidebar toggle
- Show help:
	- Keyboard: h
- Exit application:
	- Keyboard: q
	- Mouse: close the terminal window (sessions remain saved)

### Interaction Modes

The application operates in two modes:

**Normal Mode**  
Normal mode is used for navigation and control. In this mode, users can:
- switch sessions and branches,
- activate UI controls (via keyboard or mouse),
- toggle sidebar and help views,
- initiate message editing.

The application starts in Normal mode.

**Insert Mode**  
Insert mode is used for typing or editing prompt text in the input box.

Mode switching:
- Press i → enter Insert mode
- Press Esc → return to Normal mode

### Creating and Navigating Sessions

**Creating a New Session**  

A new session can be created by:
- pressing n in Normal mode,
- selecting the New Session button using Tab + Enter,
- clicking the New Session button with the mouse.

**Navigating Sessions**
- Keyboard: j / k or arrow keys('↑↓').
- Mouse: click on a session entry in the session list.

The active session is highlighted in the UI.

### Entering Prompts and Receiving Responses
1.	Focus the input box (via i or mouse click).
2.	Type a prompt.
3.	Submit the prompt by:
	- pressing Enter, or
	- clicking "send" button.

Responses from the LLM are streamed and rendered in the message area.

### Editing Previous Messages

To edit the most recent user message:
- Keyboard: press e in Normal mode.
- Mouse: click the Edit control displayed under the message.

Editing behavior:
- the message content is loaded into the input box,
- the application switches to Insert mode,
- submitting the edited message forks a new branch to preserve history.

### Branching Conversations

Each session may contain multiple branches. Use '[' and ']' key to switch between branches.

The message view updates immediately when the active branch changes.

-----

## Reproducibility Guide 

In order to use our CLI, first clone the project repository from GitHub:
```bash
git clone https://github.com/HousenZhu/ECE1724_Project.git
cd ECE1724_Project
```

This project uses the Qwen model through the DashScope API for online inference.
For simplicity, the API key is currently defined directly in `api_key.rs`:

```bash
pub const DASHSCOPE_API_KEY: &str = "your_api_key_here";
```
In a production setting, the API key should be managed using environment variables or a secure configuration method.

Next, choose the appropriate environment directory based on your operating system:

* Windows:

```bash
cd Windows
```
On Windows, the project supports interaction with the language model through the terminal interface. The text-based user interface (TUI) is not available.

* macOS:

```bash
cd macOS
```
On macOS, the project provides a fully functional text-based user interface (TUI) along with some key GUI functionalities for enhanced interaction.

After selecting the environment, build and run the project using Cargo:

```bash
cargo build
cargo run
```
Once the program starts, users can interact with the language model directly from the terminal or TUI.
Enjoy exploring the system!



---

## Contributions 

| Task / Feature                          | Housen Zhu (MCP & Workflow) | Chufan Ju (Context & backends) | Tianqi Ju (UI) |
|-----------------------------------------|-----------------------------|-----------------------------------|----------------|
| Set up local inference with Ollama      | ✅                   |                               |                             |
| Set up local inference with Qwen3      |                        | ✅                          |                             |
| implement Rust inference backends       |                       | ✅                             |                               |
| Implement inference API for CLI         | ✅                    |                               |                             |
| Support streaming token-by-token output |                     | ✅                              |                             |
| Session context management              |                      | ✅                             |                             |
| Save/restore sessions                   |                      | ✅                             |                             |
| Branching task histories                |                      | ✅                             |                             |
| Implement agentic workflow decomposition | ✅                    |                              |                             |
| MCP protocol integration                | ✅                     |                              |                             |
| Tool discovery via MCP servers          | ✅                    |                              |                             |
| Tool invocation & result handling       | ✅                     |                              |                             |
| Build CLI with Ratatui                  |                      |                               | ✅                           |
| Input/output panes (prompts & responses)|                      |                               | ✅                           |
| Scrolling history & navigation          |                      |                               | ✅                           |
| Display session state & tool results    |                      |                               | ✅                           |
| Error handling & status messages        |                      |                               | ✅                           |
| Editing messages to fork a branch        |                      |                               | ✅                           |
| Keyboard shortcuts for context/session  |                      |                               | ✅                           |
| System integration (all modules)        | ✅                    |                               |                            |
| Testing & debugging                     | ✅                    | ✅                             | ✅                           |
| Documentation                           | ✅                    | ✅                             | ✅                           ||

---

## Lessons learned and concluding remarks
