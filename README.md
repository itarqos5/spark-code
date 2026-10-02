# spark-code

A lightweight native Windows 10 coding workspace, currently under active development.

## Design commitments

- Native Rust desktop and terminal interfaces, with no Electron or bundled browser
- Official Codex app-server and Claude Code CLI adapters using their own subscription sign-in
- Local projects and history with bounded in-memory transcript windows
- Separate concurrent tasks, explicit approvals and cancellation
- Import only when requested in Settings, with a preview before writing
- No credential extraction, hidden API billing, quota-reset claims or automatic history scanning

Working features, installer downloads, measurements and known limitations will be added as each tested milestone lands. This initial repository is not yet a ready-to-install release.

Licensed under MIT.
