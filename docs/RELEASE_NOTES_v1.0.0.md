# spark-code 1.0.0

The first downloadable release of spark-code: an independent native Rust coding workspace with graphical and terminal interfaces. No Electron or bundled browser engine is used.

## Downloads

- **Windows x64 installer:** `spark-code-1.0.0-windows-x64-setup.exe`
- **Windows x64 portable ZIP:** `spark-code-1.0.0-windows-x64-portable.zip`
- **Linux x64 native archive:** `spark-code-1.0.0-linux-x64.tar.gz`
- Matching platform-specific `SHA256SUMS.txt` files and no-account launch reports (`windows-smoke.json`, `linux-smoke.json`)

Both platform bundles contain the desktop app, terminal app, installation instructions, feature boundaries, release notes, MIT license, and third-party dependency notices/licenses. The Windows installer runs per user without requesting administrator rights, offers user-PATH integration and a desktop shortcut, and adds Start-menu entries. Uninstall preserves chat history and project files. The portable packages do not configure PATH automatically.

## Features implemented in 1.0.0

### Native workspace

- Iced desktop frontend with the CPU tiny-skia renderer and a Ratatui terminal frontend sharing one local SQLite store
- Local projects and conversations, conversation search, JSON export, provider selection, and model selection
- Multiline desktop composer, response copying, and keyboard shortcuts; `spark-code` opens the terminal UI and `spark-code gui` launches the desktop app
- Bounded transcript windows and event queues, on-demand provider processes, and event-driven desktop idle behavior

### Official provider integrations

- Codex through its official app-server protocol, and Claude Code through the unmodified official CLI
- Explicit official login launch and account/model refresh; provider credentials remain with those tools
- Existing-subscription authentication checks, with API-billed modes rejected rather than silently falling back to API-key billing
- Streaming responses, tool requests, explicit approval/denial, and task cancellation
- Separate task state with up to four concurrent agents, two by default, including work in different project directories

### Explicit, previewed imports

- Settings-driven imports with a conversation-selection preview before writing local history
- Compatible native Codex thread history through official provider APIs
- Supported T3 Code projection SQLite backups and spark-code JSON/ZIP backups
- Imported project paths and text; compatible native Codex threads can resume through the official provider
- Imported text is not automatically sent to a provider. The desktop's explicit context action places a small recent excerpt into the draft for review

### Packaging and reliability work

- Versioned Windows setup and portable ZIP, plus native Linux x64 tarball
- Windows PE architecture/import validation, explicit runtime dependency handling, exact-file uninstall manifest, and dependency-license inventory
- Windows user-PATH add/remove tests covering long raw values, idempotence, empty segments, and pre-existing entries
- Native Linux ELF architecture validation, preserved executable permissions, archive manifest, and SHA-256 integrity files
- Unit tests for provider protocol handling, import validation, persistence, bounded concurrency, approvals, and cancellation, plus platform packaging checks

## Validation and platform scope

Publication is gated on both platform CI jobs passing for the release commit. Windows CI uses Windows Server 2022 and exercises native MSVC build/tests, CLI help, visible GUI launch, idle measurement, and normal GUI close. Linux CI uses Ubuntu 24.04 and exercises native build/tests, CLI help, and a visible packaged GUI launch under Xvfb/X11. No real provider account or authenticated inference is used in CI.

- **Windows:** x64, intended for Windows 10/11. A physical Windows 10 installation/uninstallation cycle has not been verified. Server CI does not establish full compatibility with every Windows 10 build
- **Linux:** x64, built on Ubuntu 24.04 with glibc 2.39. Requires compatible system libraries and X11 or XWayland; this is not a universal static binary. Older distributions may require a source build. The bundled Linux `INSTALL.md` lists desktop and file-dialog requirements
- **macOS and ARM64:** no verified binaries are included in this release

The attached smoke reports describe the actual release run. Idle memory samples exclude authenticated provider workloads and are not a fixed RAM guarantee. Active memory also includes provider processes, tools, transcript size, and simultaneous tasks.

## Known limitations and safety boundaries

- This is an early independent release despite its requested 1.0.0 version number, not feature parity with Codex Desktop, T3 Code, or ChatGPT
- No ChatGPT dot identity access, hosted dot workflows, unsupported cloud-history APIs, or hidden provider endpoints
- Only Codex and Claude Code adapters are implemented; arbitrary provider/API-key plugins are not
- Real authenticated subscription inference and account quota behavior have not been end-to-end tested in this release environment
- Windows executables and installer are unsigned. SmartScreen/unknown-publisher warnings are possible; verify origin and checksums, and do not disable security protections
- Provider tools must be installed and signed in separately. Provider access, usage limits, features, and charges depend on your account and provider terms. spark-code cannot reset quotas or guarantee free usage
- Native Windows Claude Code has no OS-level sandbox. Review tool permissions and approval requests carefully
- Agent tool output is a stream, not a full interactive terminal emulator
- T3 imports preserve supported text/path data, not all attachments, worktrees, branch metadata, tool state, or provider state. Unknown schemas are rejected. Use a consistent SQLite backup rather than a live database with WAL/SHM sidecars
- Importing project paths does not copy source files. Re-add paths that do not exist on the current machine
- Local history and exports are unencrypted. Keep exports and backups private
- Linux native Wayland without XWayland, every portal backend, and every Linux distribution have not been validated

## Quick start

**Windows:** run Setup, or extract the portable ZIP fully. Open `spark-code-desktop.exe`, or run `spark-code` from a fresh terminal after choosing PATH integration.

**Linux:** verify `sha256sum -c spark-code-1.0.0-linux-x64-SHA256SUMS.txt`, extract the tarball, and run `./spark-code-desktop` or `./spark-code` in the extracted directory. Keep both executables together. Install the desktop libraries and portal backend described in bundled `INSTALL.md` if missing.

Configure the official provider executable in Settings, use its supported subscription sign-in, then select Refresh. No provider installation, sign-in, history scan, or import runs automatically.

Source: [itarqos5/spark-code](https://github.com/itarqos5/spark-code) · [Build verification](https://github.com/itarqos5/spark-code/actions)
