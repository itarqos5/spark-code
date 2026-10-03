# Spark Code 1.1.3

This release includes all changes since 1.1.0: a refreshed native conversation workspace, organized settings, GPU rendering, responsiveness improvements, and better official CLI discovery on Windows. Spark Code remains a Rust desktop and terminal application using your installed Codex and Claude Code CLIs and local SQLite history.

## Downloads

- **Windows x64 setup:** `spark-code-1.1.3-windows-x64-setup.exe`
- **Windows x64 portable ZIP:** `spark-code-1.1.3-windows-x64-portable.zip`
- **Linux x64 archive:** `spark-code-1.1.3-linux-x64.tar.gz`
- Platform SHA-256 checksum files, native launch reports, Windows chrome/resource reports, and build reports identifying the release commit

Both platform bundles include the desktop and terminal executables, installation instructions, feature boundaries, release notes, and application, dependency, and DM Sans licenses. The Windows installer supports per-user installation, optional user-PATH integration, and shortcuts. Uninstall preserves local history and project files.

## Conversation workspace

- Bundled DM Sans replaces Inter, with updated neutral light/dark surfaces and clearer secondary text.
- A compact workspace sidebar groups conversations by date and keeps project and conversation lists usable at the minimum window size.
- Assistant responses use an open transcript; user messages use right-aligned bubbles. Copy controls and optional timestamps accompany messages.
- A rounded, centered composer keeps model, reasoning-effort, access, and send/cancel controls together. Contextual starter prompts populate a draft for review.
- Follow-latest scrolling respects manual scrolling and offers a return-to-latest action.
- Selecting the current provider preserves composer choices; switching providers clears the previous transcript. Searching keeps the selected conversation available to the engine.
- Embedded OpenAI and Claude SVG marks identify provider connections. Native title-bar controls, Markdown/code rendering, Enter/Shift+Enter behavior, and IME guards remain available.

## Organized, saved settings

Seven settings tabs cover **General**, **Appearance**, **Providers**, **ChatGPT & Dots**, **Performance**, **Data & history**, and **Keyboard shortcuts**. Settings navigation remains visible when the chat sidebar is hidden.

Preferences now include message text size, compact spacing, timestamps, auto-follow scrolling, sidebar/activity visibility, provider enablement, CLI discovery, connection checks, concurrent-agent capacity, transcript length, stream refresh interval, and antialiasing. Persisted numeric preferences are bounded before use.

Keyboard controls include Ctrl+N for a new conversation, Ctrl+, for settings, Ctrl+B for the sidebar, Ctrl+Shift+A for activity, and Ctrl+Shift+T for appearance.

## Rendering and responsiveness

- wgpu GPU rendering is preferred, with tiny-skia available as a renderer initialization fallback.
- Performance settings show the actual graphics adapter/backend and process memory at the last diagnostic check.
- Completed Markdown and vector geometry are cached; streaming updates are batched and search is debounced.
- Typing, scrolling, and transitions use cached conversation history. Transcript reads occur when selecting a conversation, sending, changing its history limit, or completing the selected run.
- Native file/folder pickers are asynchronous. Idle workspaces stop polling once pending activity finishes.
- Optional antialiasing, bounded transcript lengths, adjustable stream intervals, and one to four concurrent agents expose resource tradeoffs. Each project folder remains limited to one running agent.

## Official provider connections

- Optional startup discovery finds installed Codex and Claude Code CLIs on PATH, followed by connection/model checks when enabled.
- Windows supports plain npm `.cmd`/`.bat` and PowerShell CLI launchers, with validation of suspicious script-path spellings. Codex discovery prefers the bundled native executable when available.
- Provider cancellation and transport cleanup terminate the spawned Windows process tree, including children started through npm shims.
- Provider settings show CLI versions and connection state, support enabling/disabling connections, and keep sign-in with the official CLI.
- Codex continues to use its CLI-managed ChatGPT account. ChatGPT and Dots actions open their official websites; Dots is not a native Spark Code API integration.

Readiness/model gating, subscription checks, access confirmation, tool approval/denial, previewed imports, and JSON export remain in place. Provider credentials remain with the official CLIs. Token counts are not quota percentages, and unavailable provider quota data remains unavailable.

## Release and validation

The package, CLI/resource checks, smoke tests, and publication workflow target 1.1.3. Both packagers include the replacement DM Sans license. Publication requires successful Windows and Linux builds/tests for the release commit, native launch/chrome checks, and verified package checksums. The release verifier also covers 1.1.3 explicitly.

`packaging/capture-desktop.ps1` adds repeatable native screenshots and short idle samples against isolated synthetic history, covering both themes, all settings tabs, conversation content, and the minimum window size. Design and product records are included in `DESIGN.md`, `.impeccable/design.json`, and `PRODUCT.md`.

## Compatibility and limits

Windows packages are unsigned x64 builds intended for Windows 10/11. Linux x64 packages are built on Ubuntu 24.04 and require compatible system libraries and X11 or XWayland. No macOS or ARM64 binaries are provided. Local history and exports remain unencrypted.

Desktop settings/layout features do not establish terminal UI parity. General chat is a provider read-only scope, not OS isolation. CI uses no real provider account or authenticated inference; attached reports describe the actual release checks rather than every hardware, desktop, or provider configuration.

**Full changes:** [v1.1.0...v1.1.3](https://github.com/itarqos5/spark-code/compare/v1.1.0...v1.1.3)
