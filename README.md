# spark-code

A lightweight native Windows 10 coding workspace with desktop and terminal interfaces. Built in Rust with no Electron or bundled browser.

## Design commitments

- Native Rust desktop and terminal interfaces, with no Electron or bundled browser
- Official Codex app-server and Claude Code CLI adapters using their own subscription sign-in
- Local projects and history with bounded in-memory transcript windows
- Separate concurrent tasks, explicit approvals and cancellation
- Import only when requested in Settings, with a preview before writing
- No credential extraction, hidden API billing, quota-reset claims or automatic history scanning

## Start

Install the Windows setup, or unzip the portable package and open `spark-code-desktop.exe`.
In a new terminal, run `spark-code` for the TUI or `spark-code gui` for the desktop.

Use Settings to point to official native Codex / Claude Code executables, launch their own sign-in, and Refresh. No provider install or sign-in happens automatically.

- [Install and provider setup](docs/INSTALL.md)
- [Working features and explicit limitations](docs/FEATURES.md)
- [Build and package from source](docs/build-windows.md)
- [CI builds and verification](https://github.com/itarqos5/spark-code/actions)

This is an early independent release. Review the feature boundaries before trusting it with important work. The installer is unsigned; real subscription inference and a physical Windows 10 install cycle have not been exercised in the development environment.

Licensed under MIT.
