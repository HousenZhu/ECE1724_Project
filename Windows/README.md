# User Guide

This section describes how users interact with the CLI through built-in commands to manage models, sessions, and branches.

---

## Model Management

- Display the currently active language model.
    - Use `/use` to show the model in use.

- Switch to a different language model at runtime.
    - Use `/use <model>` to change the active model.

Model switching allows users to adapt the system to different tasks without restarting the CLI.

---

## Session Management

- Manage multiple conversation sessions with isolated contexts.
    - Each session represents an independent conversation state.

- List all stored sessions.
    - Use `/session list` to view available sessions.

- Identify the currently active session.
    - Use `/session current` to show the current session ID.

- Delete one or more sessions when they are no longer needed.
    - Use `/session delete <id>` to remove a specific session.
    - Use `/session clear` to remove all stored sessions.

Sessions allow users to resume previous work and organize conversations across different tasks.

---

## Branch Management

- Organize conversations into multiple branches within a session.
    - Each branch represents a separate conversation path.

- Create and switch between branches.
    - Use `/branch new <name>` to create a new branch.
    - Use `/branch switch <name>` to change the active branch.

- Inspect the branch structure of the current session.
    - Use `/branch list` to list all branches.
    - Use `/branch current` to show the active branch.

- Modify or clean up existing branches.
    - Use `/branch delete <name>` to delete a branch.
    - Use `/branch rename <old> <new>` to rename a branch.
    - Use `/branch clear` to remove all branches except `main`.

Branch management enables users to explore alternative ideas or workflows without overwriting existing conversations.

---

## Saving and Loading

- Persist conversation data locally for later use.
    - Use `/save` to save the current branch.

- Restore previously saved sessions.
    - Use `/load <session_id>` to load a saved session.

Conversation history is stored as JSON files under `logs/<session>_<branch>.json`.

---

## General Commands

- Access the built-in help menu.
    - Use `/help` to display available commands.

- Exit the application safely.
    - Use `/quit` to terminate the CLI.

---

## Notes

- Conversation history is saved locally in JSON format.
- Model context persists unless the current session or branch is cleared.
- Switching sessions or branches does not automatically discard existing context.

---

### Summary

This CLI provides structured interaction through sessions and branches, allowing users to manage context, reuse previous conversations, and explore multiple workflows in parallel. Compared to simple prompt-based CLIs, this design offers improved organization, flexibility, and control over long-running interactions.
