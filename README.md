# Spark Code 1.1.0

A lightweight native coding workspace for Windows and Linux, with desktop and terminal interfaces. Built in Rust with no Electron or bundled browser. The app is **Spark Code**; the repository and terminal command remain `spark-code`.

## What is new

- Monochrome native desktop with embedded Inter, saved dark/light themes, reduced-motion support, and a custom draggable title bar with native window controls
- Readable Markdown and code, a focused “What are we building next?” welcome screen, and one Projects add control
- Model-first selection backed by each official provider’s actual readiness and model catalog
- Read-only general chat without selecting a project, plus project chat with Workspace approval mode by default
- Provider-reported Codex reasoning efforts, explicit confirmation for Codex Full access, and retained tool approvals
- Separate Codex and Claude account/usage displays and a bounded activity timeline; unavailable quota data stays unavailable

The terminal interface shares the local data store and core provider engine. The new desktop layout and controls are specific to the graphical interface.

## Download and start

Get the **Windows x64 setup**, **Windows x64 portable ZIP**, or **Linux x64 archive** from [the 1.1.0 release](https://github.com/itarqos5/spark-code/releases/tag/v1.1.0). Verify the matching SHA-256 checksums before running a download. Windows displays and shortcuts use **Spark Code**.

- **Windows:** install setup, or fully extract the portable ZIP, then open `spark-code-desktop.exe`
- **Linux:** extract the archive and run `./spark-code-desktop`; compatible system libraries and X11 or XWayland are required
- **Terminal:** run `spark-code` for the TUI or `spark-code gui` for the desktop; setup offers optional Windows user-PATH integration

Install the official Codex or Claude Code CLI separately. In Settings, select its executable, launch its own subscription sign-in, then Refresh. Sending stays locked until the selected provider is ready and an available model is selected. Installation, sign-in, history scanning, and imports do not run automatically.

## Boundaries worth knowing

This is an independent early release. It does not claim feature parity with Codex Desktop, T3 Code, or ChatGPT. General chat uses an app-owned neutral working directory with read-only provider permissions; this is **not a promise of OS isolation**, and installed provider policies/hooks still apply. Claude Full access is not exposed because the official bypass mode removes ordinary permission prompts.

Credentials remain with the official CLIs. API-billed modes fail closed, and there is no hidden API-billing fallback. Imports are explicit and previewed. Local chat history and exports are unencrypted.

The Windows installer is unsigned. Do not disable security protections to run it. Real authenticated inference and a physical Windows 10 install/uninstall cycle have not been verified. Windows CI uses Server 2022; Windows 10 1809+ is the intended target. Linux CI uses Ubuntu 24.04; older distributions may need a source build. No macOS or ARM64 binaries are included.

- [1.1.0 release notes and validation scope](docs/RELEASE_NOTES_v1.1.0.md)
- [Install and provider setup](docs/INSTALL.md)
- [Features and limitations](docs/FEATURES.md)
- [Build and package from source](docs/build-windows.md)
- [CI builds and native verification](https://github.com/itarqos5/spark-code/actions)

Licensed under MIT. The bundled Inter font includes its SIL Open Font License.
