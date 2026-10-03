# Native workspace redesign

## Layout goals

The 1.1 desktop revision keeps the lightweight native renderer while replacing the composition with a focused workspace and a readable conversation area.

- Display name: **Spark Code**. Repository, executable command, install paths and data remain `spark-code`
- Crisp white lightning bolt, custom draggable title bar, minimize/maximize/close, keyboard access and DPI-scaled controls
- Black/white default appearance, optional light theme, persistent theme/reduced-motion settings
- Compact persistent projects/conversations sidebar; centered readable chat; useful activity rail only when opened
- Well-spaced rounded composer with context, provider and provider-specific model selection
- Embedded licensed DM Sans typography, vector provider marks, native vector icons, native Markdown/code rendering
- Short interaction/theme transitions only; no continuing idle animation timer
- Native screenshots must be reviewed before a new installer/release is delivered

## Architecture boundary

This remains a Rust/Iced application. No Electron, Tauri, Chromium, WebView or JavaScript UI engine is bundled. React and Next.js are not claimed to run inside it. Styling/layout concepts are implemented natively rather than introducing a new browser runtime.

## Reference

Cursor's publicly published UI references were consulted for layout clarity, not copied as assets: https://cursor.com/changelog/1-0

The current layout follows the user's T3 Code and ChatGPT reference: a compact persistent sidebar, dated conversation groups, an open reading area, contextual starter prompts, and a centered composer. Starter prompts only populate a draft. Real-model readiness gating and separate provider usage remain. Enter submits only the focused composer; Shift+Enter inserts a newline, with IME composition guards.

## Settings and responsiveness

General, Appearance, Providers, ChatGPT & Dots, Performance, Data & history, and Keyboard shortcuts have their own navigation tabs. Preferences persist locally, including concurrent-agent capacity. Settings keeps its navigation visible even when the chat sidebar is hidden.

wgpu is the preferred GPU renderer; tiny-skia remains an initialization fallback. Performance shows the actual adapter/backend, memory at the last diagnostic check, optional multisample antialiasing, streaming refresh interval, and a bounded transcript length. Idle windows have no continuous polling. Completed Markdown and vector geometry are cached, chat search is debounced, and native pickers are asynchronous. SQLite transcript reads happen when selecting a conversation, sending, changing the history limit, or completing a selected run, rather than during typing, scrolling, or transitions.

Use `packaging/capture-desktop.ps1` with a release build for repeatable native screenshots and idle samples against synthetic data. It does not run providers or use account credentials. Captures are written under `dist/ui-review`.

## Validation

The final release build passed 61 tests across all targets, `cargo fmt --all -- --check`, and strict Clippy checks for the library and desktop binary. Both release binaries were rebuilt with locked dependencies.

Eleven native captures cover both themes, every settings tab, conversation content, and a 920 by 640 window with thirteen projects. The renderer reported an AMD Radeon RX 580 2048SP using Vulkan. In isolated synthetic-data captures with provider checks disabled, working-set samples ranged from 128.6 to 153.3 MiB. Nine two-second idle samples recorded zero additional CPU time at the process timer's precision; two recorded 0.062 and 0.141 CPU seconds. These short samples describe behavior on this machine; frame rate during streaming was not benchmarked.

The installed official Codex app-server completed initialization, read the existing ChatGPT account, and returned eight available models. This check did not run inference. Codex owns sign-in, credential storage, and refresh. ChatGPT and Dots actions open their official web destinations; the app does not claim an embedded Dots API or copy account tokens.

The independent finish review returned **ship** with no remaining material findings:

| Finding | Final verdict |
| --- | --- |
| Settings navigation and workspace-sidebar preference | Resolved |
| Locked model control opens Providers | Resolved |
| Current-provider selection preserves model, effort, and access | Resolved |
| Complete conversation row visible at 920 by 640 with many projects | Resolved |
