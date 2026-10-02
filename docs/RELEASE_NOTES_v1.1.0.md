# Spark Code 1.1.0

This release refreshes the native desktop and makes provider readiness, access scope, and usage information more explicit. Spark Code remains an independent Rust application with graphical and terminal interfaces, without Electron or a bundled browser engine. The repository and terminal command remain `spark-code`.

## Downloads

- **Windows x64 setup:** `spark-code-1.1.0-windows-x64-setup.exe`
- **Windows x64 portable ZIP:** `spark-code-1.1.0-windows-x64-portable.zip`
- **Linux x64 archive:** `spark-code-1.1.0-linux-x64.tar.gz`
- Platform-specific `SHA256SUMS.txt`, native smoke reports, Windows chrome/resource reports, and exact-commit build reports

Both platform bundles include the desktop and terminal executables, install instructions, feature boundaries, release notes, MIT license, and dependency/font licenses. Setup runs per user, offers optional user-PATH integration and a desktop shortcut, and adds Start-menu entries. Uninstall preserves local history and project files. Portable bundles do not configure PATH automatically.

The previously published v1.0.0 tag and assets remain unchanged. This is a separate 1.1.0 release, with no automatic 1.0.1 publication.

## Native desktop refresh

- Monochrome desktop using embedded Inter, with saved dark/light themes
- Bounded theme and layout transitions plus a saved reduced-motion setting
- New bolt icons and **Spark Code** Windows display/search branding
- Draggable custom title bar with native minimize, maximize/restore, close, resize edges/corners, and Windows system-menu integration
- Readable native Markdown and code rendering
- Simplified welcome screen reading exactly “What are we building next?”, with one canonical Projects add control and no canned suggestion cards
- Enter sends; Shift+Enter inserts a newline. IME composition/confirmation is guarded against accidental submission

These layout and control changes apply to the desktop. The terminal frontend continues to share the local data store and provider engine; desktop/TUI feature parity is not claimed.

## Provider selection, access, and usage

- Model-first selection keeps sending locked until the chosen provider is ready and a reported model is available
- Provider readiness/model catalogs are kept separate, including a fix for cross-provider model-cache reuse
- Read-only general chat can start without choosing a project, using an app-owned neutral working directory
- Project chat defaults to Workspace approval mode
- Codex reasoning-effort choices follow the official model catalog
- Codex Full access requires explicit desktop confirmation; tool approvals remain enabled
- Claude Full access is not exposed because the official bypass mode removes ordinary permission prompts
- Codex and Claude have separate sidebar/Settings usage displays. Claude quota percentages and reset times remain honestly unavailable when the official CLI does not expose them; actual reported token counts are shown separately
- Official CLI status and structured activity expose bounded timestamps, tool arguments/results, and brief provider-provided summaries. Raw hidden reasoning is never displayed or reconstructed

General chat is a read-only provider scope, **not a promise of OS isolation or tool-free execution**. Installed managed policies, hooks, and official CLI behavior still apply. Native Windows Claude Code has no OS-level sandbox. Review access and tool requests before authorizing work.

Both integrations use the official tools and existing-subscription authentication checks. API-billed modes fail closed; there is no hidden API-billing fallback. Provider credentials remain with their official CLIs. Spark Code cannot reset account quotas or guarantee free usage.

## Existing workspace and import capabilities

Local projects, searchable conversations, JSON export, streaming replies, explicit tool approval/denial, cancellation, and bounded concurrent tasks remain available. Import stays explicit and previewed in Settings: compatible native Codex history, supported T3 Code projection backups, and Spark Code JSON/ZIP backups.

Imported project paths do not copy source files or worktrees. T3 imports preserve supported text and paths, not all provider/tool state, attachments, or branch metadata. Unknown schemas are rejected. Use a consistent SQLite backup rather than a live database with WAL/SHM sidecars. Imported text is not automatically sent to a provider; the explicit context action puts a small recent excerpt in the draft for review.

## Validation and release gates

Publication is gated on both platform jobs passing for the exact release commit. The release job alone receives an ephemeral Actions token with `contents: write`. It runs only on the explicitly marked 1.1.0 release commit on `main`, validates artifact checksums and native reports, and refuses to overwrite an existing published release or move a conflicting tag.

- **Windows Server 2022 CI:** native MSVC build/tests; exact CLI version; embedded PE icons, Spark Code branding, and product/file versions; resizable native window styles and system-menu presence; two minimize/restore/maximize/restore cycles; restored bounds and clean close; no-account launch and idle-memory samples
- **Ubuntu 24.04 CI:** native build/tests; packaged CLI version; visible packaged GUI under Xvfb/X11; a native app-rendered PNG capture; idle process-memory status
- **Packaging:** Windows PE/import validation, explicit runtime dependency handling, exact-file uninstall manifest, user-PATH integration tests, Linux ELF architecture validation and executable permissions, package manifests, license inventories, and SHA-256 checksums

Attached reports describe the actual release run. No real provider account or authenticated inference is exercised by CI. A screenshot is a capture from the running native app, not a browser-based recreation. Idle samples exclude authenticated provider workloads and are not a fixed RAM guarantee.

## Compatibility and known limits

- **Windows:** x64, intended for Windows 10 1809+ and Windows 11. A physical Windows 10 install/uninstall cycle has not been verified. Server CI does not establish compatibility with every Windows build, DPI setup, or monitor configuration
- **Native chrome:** automated checks cover styles and native window commands; manual title-bar mouse interaction, drag/double-click behavior, every resize edge/corner, and Windows 11 Snap Layout hover are not established by those tests alone
- **Linux:** x64, built on Ubuntu 24.04 with glibc 2.39; compatible system libraries and X11 or XWayland are required. It is not a universal static binary. Older distributions may require a source build. Pure Wayland without XWayland and every desktop portal backend are not verified
- **Other platforms:** no verified macOS or ARM64 binaries
- **Signing:** Windows executables and setup are unsigned. SmartScreen/unknown-publisher warnings are possible. Verify origin and checksums; do not disable security protections
- **Provider validation:** real authenticated subscription inference and live account quota behavior remain unverified in this release environment
- **Scope:** no full parity claim with Codex Desktop, T3 Code, or ChatGPT; no dot identity/hosted workflow access, unsupported cloud-history endpoints, arbitrary API-key plugins, or full interactive terminal emulator
- **Privacy:** local history and exports are unencrypted; protect backups and exports

## Quick start

**Windows:** install setup, or extract the portable ZIP completely. Open `spark-code-desktop.exe`, or run `spark-code` in a new terminal after selecting PATH integration.

**Linux:** run `sha256sum -c spark-code-1.1.0-linux-x64-SHA256SUMS.txt`, extract the archive, and start `./spark-code-desktop` or `./spark-code`. Keep both binaries together. See bundled `INSTALL.md` for desktop and portal dependencies.

Install the official provider CLI separately, configure its executable in Settings, use its own supported subscription sign-in, then Refresh and select an available model. No installation, sign-in, history scan, or import happens automatically.

Source: [itarqos5/spark-code](https://github.com/itarqos5/spark-code) · [Build verification](https://github.com/itarqos5/spark-code/actions) · [Feature boundaries](https://github.com/itarqos5/spark-code/blob/v1.1.0/docs/FEATURES.md)
