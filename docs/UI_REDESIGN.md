# Native workspace redesign

## Layout goals

The 1.1 desktop revision keeps the lightweight native renderer while replacing the composition with a focused workspace and a readable conversation area.

- Display name: **Spark Code**. Repository, executable command, install paths and data remain `spark-code`
- Crisp white lightning bolt, custom draggable title bar, minimize/maximize/close, keyboard access and DPI-scaled controls
- Black/white default appearance, optional light theme, persistent theme/reduced-motion settings
- Compact persistent projects/conversations sidebar; centered readable chat; useful activity rail only when opened
- Well-spaced rounded composer with context, provider and provider-specific model selection
- Embedded licensed Inter Variable typography, native vector icons, native Markdown/code rendering
- Short interaction/theme transitions only; no continuing idle animation timer
- Native screenshots must be reviewed before a new installer/release is delivered

## Architecture boundary

This remains a Rust/Iced application. No Electron, Tauri, Chromium, WebView or JavaScript UI engine is bundled. React and Next.js are not claimed to run inside it. Styling/layout concepts are implemented natively rather than introducing a new browser runtime.

## Reference

Cursor's publicly published UI references were consulted for layout clarity, not copied as assets: https://cursor.com/changelog/1-0

The final layout uses a single project-add control, no canned suggestion cards, real-model readiness gating, and separate honest provider usage panels. Enter submits only the focused composer; Shift+Enter inserts a newline, with IME composition guards.
