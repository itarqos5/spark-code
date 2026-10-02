# What this release includes

## Working implementation

- Native Rust desktop (Iced + CPU tiny-skia renderer) and terminal (Ratatui) frontends sharing one SQLite data store
- Projects, searchable local conversations, export, provider and model selection
- Official Codex app-server and unmodified Claude Code CLI adapters, with existing-subscription auth checks; API-billed modes fail closed
- Streaming replies, provider tool requests, approval/deny, and isolated cancellation
- Up to four bounded concurrent agents, two by default; different project directories can work at once
- Explicit Settings imports with selection preview: compatible Codex history through official APIs; supported T3 Code projection SQLite backup; spark-code JSON/ZIP
- Multiline desktop composer, copy response, explicit imported-context-to-draft action
- Official CLI login launch and explicit account/model refresh; metadata is read without importing history or sending a prompt
- Per-user Windows installer with optional user PATH integration and portable ZIP

## Important boundaries

This is an independent early release, not a full replacement for Codex Desktop, T3 Code or ChatGPT.

- No access to ChatGPT dot identities, hosted dot workflows, unsupported cloud history APIs or hidden provider endpoints
- Two supported providers. Arbitrary provider/API-key plugins are not implemented
- A project path import does not copy source files or worktrees. Missing paths must be added again for the current machine
- T3 imports preserve text and paths, not all attachments, provider state, tools or branch metadata. Unknown schema shapes are rejected. Use a consistent backup, never a live DB with WAL/SHM sidecars
- Native Codex history can resume compatible official threads. Imported T3 text is not silently transmitted; the desktop action places a small recent excerpt in the draft for review
- Native Windows Claude Code has no OS-level sandbox. Provider permissions are retained and tool requests require review
- Tool output is a coding-agent stream, not a full interactive ConPTY terminal emulator
- Account quota reset is not possible. Codex reset timestamps are provider-reported Unix times. Claude quotas may be unavailable; token usage is not a quota percentage
- Account credentials stay with the official CLIs. Local chat DB and exports are unencrypted
- Executable signing is not yet configured; unsigned Windows builds may display SmartScreen warnings
- CI smoke testing runs on Windows Server 2022. Windows 10 1809+ is the intended target, but a physical Windows 10 install cycle has not yet been verified

## Memory claims

The desktop bundles no browser/Electron runtime. Provider CLIs are started only on demand and queues/transcripts are bounded. This architecture is intended to keep idle overhead small, but **no fixed RAM claim is made**. Total active memory includes the official provider processes and depends on model tools, transcript size and simultaneous agents. CI emits a measured idle private-bytes/working-set report; it is not a measurement of real authenticated agent workloads.
