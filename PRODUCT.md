# Spark Code

<!-- impeccable:product-schema 1 -->

## Platform

Native Windows and Linux desktop, with a separate terminal interface.

## Product Purpose

A coding and chat workspace using the user's installed official Codex and Claude Code CLIs, project folders, and local conversation history. The desktop should feel familiar alongside T3 Code and ChatGPT.

## Capabilities and Constraints

Rust and Iced; SQLite local storage. Preserve existing imports, exports, provider permissions, tool approvals, and terminal compatibility. Codex manages its own ChatGPT authentication. Credentials are not copied into Spark Code. ChatGPT-only services such as Dots remain in ChatGPT unless a supported integration is established. GPU rendering, responsive input, bounded memory, and organized, persisted preferences are explicit requirements.

## Brand Commitments

Keep the Spark Code name and existing logo. The user requested T3 Code and ChatGPT as the UI references: neutral surfaces, readable chat, a compact workspace sidebar, and recognizable settings navigation.

## Open Decisions

The desired depth of Dots integration is awaiting clarification; provide access to official ChatGPT services without claiming an undocumented native Dots integration.
