# Spark Code 1.1.0 features and boundaries

## Native desktop and terminal

- Rust desktop built with Iced and the CPU tiny-skia renderer, plus a Ratatui terminal frontend sharing one SQLite store
- No Electron, embedded web preview, or bundled browser engine
- Native desktop typography using embedded Inter; monochrome dark/light themes saved between launches
- Short, bounded theme/layout transitions and a saved reduced-motion setting
- Spark Code bolt branding and Windows display/search metadata; executable names and terminal command remain lowercase `spark-code`
- Custom draggable desktop title bar with minimize, maximize/restore, and close; resize edges/corners and a native Windows system menu
- A focused “What are we building next?” welcome screen with one canonical Projects add control
- Native Markdown/code rendering, multiline composer, response copying, Enter to send, Shift+Enter for a newline, and an IME-composition submission guard
- Local projects, searchable conversations, JSON export, and explicit imported-context-to-draft actions

The refreshed layout, theme controls, account panels, and access/effort selectors are desktop features. The terminal interface shares the provider engine and data store; identical interface capabilities are not claimed.

## Provider readiness and safe scopes

- Codex through the official app-server protocol and Claude Code through the unmodified official CLI
- Explicit official login launch and account/model refresh; no provider installation or sign-in happens automatically
- Readiness and model catalogs tracked separately per provider, avoiding cross-provider model-cache reuse
- Model-first desktop selection: sending requires a ready provider and a model from its reported catalog
- Read-only general chat without selecting a project, using an app-owned neutral working directory
- Project chat defaults to Workspace approval mode; supported access modes are provider-specific
- Codex reasoning-effort options come from the official provider’s model information
- Codex Full access requires an explicit desktop confirmation and retains provider tool approvals
- Claude Full access is not exposed: the official bypass mode would remove ordinary permission prompts
- Existing-subscription authentication checks reject API-billed modes rather than silently falling back to API-key billing

Read-only general chat is a provider permission scope, **not OS isolation or a guarantee that no tools execute**. Installed managed policies, hooks, and other official CLI behavior still apply. Native Windows Claude Code has no OS-level sandbox. Review project paths, access settings, and tool approval requests before proceeding.

## Actual usage and bounded activity

- Separate Codex and Claude usage/account information in the sidebar and Settings, sourced from the official tools
- Codex quota percentages and reset timestamps only when reported by the provider
- Claude quota percentages/reset times shown as unavailable when the official interface does not expose them; actual reported token usage remains distinct from account quotas
- Streaming replies and structured activity with bounded timestamps, tool arguments/results, status, and brief provider-exposed summaries
- Raw hidden reasoning, private thinking, and signatures are not displayed or reconstructed
- Explicit tool approval/denial, independent cancellation, and separate concurrent task state
- Up to four bounded concurrent agents, two by default, including different project directories

Spark Code cannot reset provider quotas, guarantee free usage, or infer a remaining quota from token counts. Provider access, limits, features, and charges depend on the account and provider terms.

## Explicit, previewed imports

- Import only from Settings after an explicit request and conversation-selection preview
- Compatible native Codex history through official APIs, supported T3 Code projection SQLite backups, and Spark Code JSON/ZIP backups
- Compatible native Codex threads can resume through the official provider
- Imported T3 text is not silently sent: an explicit desktop action places a small recent excerpt into the draft for review
- Project paths are references, not copies of source files or worktrees; missing paths must be re-added on the current machine
- T3 imports preserve supported text and paths, not all attachments, tool state, provider state, or branch metadata; unknown schema shapes are rejected

Use a consistent T3 SQLite backup rather than a live database with WAL/SHM sidecars. Credentials stay with the official CLIs. Local history and exported backups are unencrypted.

## Packaging and verification

- Windows x64 per-user setup with optional user-PATH integration, Start-menu entries, and an optional desktop shortcut; uninstall preserves history and project files
- Windows x64 portable ZIP and Linux x64 native archive; both include desktop and terminal executables, documentation, license, and third-party notices
- Versioned 1.1.0 filenames, exact CLI version checks, and platform SHA-256 manifests
- Native Windows icon/version-resource validation, window-style/system-menu checks, repeated minimize/restore/maximize cycles, clean close, and idle memory measurement in CI
- Packaged Linux CLI/visible GUI smoke testing under Xvfb/X11, with a native app-rendered screenshot
- Release publication gated on passing Linux and Windows jobs for the exact release commit, matching checksums, and successful platform reports

The existing 1.0.0 release is retained unchanged. Release scripts do not overwrite an existing published version.

## Important limitations

- Independent early software, not feature parity with Codex Desktop, T3 Code, or ChatGPT
- No access to ChatGPT dot identities, hosted dot workflows, unsupported cloud-history APIs, or hidden provider endpoints
- Only Codex and Claude Code adapters are implemented; arbitrary provider/API-key plugins are not
- Real authenticated subscription inference and account quota behavior have not been end-to-end verified in the release environment
- Agent tool output is a coding-agent stream, not a full interactive ConPTY terminal emulator
- Windows executables/installer are unsigned; verify download origin and checksums without disabling security protections
- Windows CI runs on Server 2022. Windows 10 1809+ is intended, but an actual Windows 10 installation/uninstallation cycle is unverified
- Native command/style tests do not establish every title-bar mouse interaction, resize edge, DPI configuration, multi-monitor layout, or Windows 11 Snap Layout hover behavior
- Linux x64 builds target Ubuntu 24.04 and compatible libraries, including glibc 2.39, X11 or XWayland, and the desktop/file-dialog dependencies in bundled `INSTALL.md`; older distributions may require a source build
- Pure Wayland without XWayland, all portal backends, and every Linux distribution are not validated
- No verified macOS or ARM64 binaries are supplied

## Memory claims

Provider processes start on demand, queues and transcript windows are bounded, and the desktop bundles no browser runtime. These choices aim to keep idle overhead small, but **no fixed RAM claim is made**. CI reports idle private bytes/working set; these are not authenticated agent-workload measurements. Active memory includes provider processes, tools, transcript size, and simultaneous tasks.
