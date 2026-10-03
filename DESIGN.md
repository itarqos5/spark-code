---
name: "Spark Code"
description: "A focused native coding and chat workspace."
colors:
  bg-dark: "#18181b"
  bg-light: "#ffffff"
  side-dark: "#111113"
  side-light: "#f7f7f8"
  surface-dark: "#202023"
  surface-light: "#ffffff"
  raised-dark: "#2b2b30"
  raised-light: "#ededee"
  border-dark: "#333338"
  border-light: "#e4e4e7"
  text-dark: "#f4f4f5"
  text-light: "#171719"
  muted-dark: "#a1a1aa"
  muted-light: "#66666f"
  faint-dark: "#94949e"
  faint-light: "#707078"
  inverse-dark: "#121214"
  inverse-light: "#ffffff"
typography:
  headline:
    fontFamily: "DM Sans"
    fontSize: "27px"
    fontWeight: 600
  section:
    fontFamily: "DM Sans"
    fontSize: "15px"
    fontWeight: 600
  body:
    fontFamily: "DM Sans"
    fontSize: "15px"
    fontWeight: 400
  control-label:
    fontFamily: "DM Sans"
    fontSize: "14px"
    fontWeight: 400
  navigation:
    fontFamily: "DM Sans"
    fontSize: "13px"
    fontWeight: 400
  label:
    fontFamily: "DM Sans"
    fontSize: "12px"
    fontWeight: 400
  caption:
    fontFamily: "DM Sans"
    fontSize: "11px"
    fontWeight: 400
  mono:
    fontFamily: "monospace"
rounded:
  picker: "6px"
  control: "7px"
  action: "8px"
  panel: "9px"
  message: "14px"
  composer: "22px"
spacing:
  tight: "3px"
  small: "6px"
  group: "8px"
  inset: "12px"
  panel: "16px"
  row: "18px"
  section: "24px"
  open: "32px"
components:
  button-primary-dark:
    backgroundColor: "{colors.text-dark}"
    textColor: "{colors.inverse-dark}"
    typography: "{typography.label}"
    rounded: "{rounded.action}"
    padding: "7px 11px"
  button-primary-hover-dark:
    backgroundColor: "{colors.muted-dark}"
    textColor: "{colors.inverse-dark}"
  input-dark:
    backgroundColor: "{colors.bg-dark}"
    textColor: "{colors.text-dark}"
    typography: "{typography.navigation}"
    rounded: "{rounded.control}"
    padding: "9px 11px"
  navigation-selected-dark:
    backgroundColor: "{colors.raised-dark}"
    textColor: "{colors.text-dark}"
    typography: "{typography.navigation}"
    rounded: "{rounded.control}"
    padding: "11px 10px"
  shortcut-dark:
    backgroundColor: "{colors.surface-dark}"
    textColor: "{colors.muted-dark}"
    typography: "{typography.label}"
    rounded: "{rounded.picker}"
    padding: "6px 10px"
  composer-dark:
    backgroundColor: "{colors.surface-dark}"
    textColor: "{colors.text-dark}"
    typography: "{typography.body}"
    rounded: "{rounded.composer}"
    padding: "14px 16px"
  button-primary-light:
    backgroundColor: "{colors.text-light}"
    textColor: "{colors.inverse-light}"
    typography: "{typography.label}"
    rounded: "{rounded.action}"
    padding: "7px 11px"
  button-primary-hover-light:
    backgroundColor: "{colors.muted-light}"
    textColor: "{colors.inverse-light}"
  input-light:
    backgroundColor: "{colors.bg-light}"
    textColor: "{colors.text-light}"
    typography: "{typography.navigation}"
    rounded: "{rounded.control}"
    padding: "9px 11px"
  navigation-selected-light:
    backgroundColor: "{colors.raised-light}"
    textColor: "{colors.text-light}"
    typography: "{typography.navigation}"
    rounded: "{rounded.control}"
    padding: "11px 10px"
  shortcut-light:
    backgroundColor: "{colors.surface-light}"
    textColor: "{colors.muted-light}"
    typography: "{typography.label}"
    rounded: "{rounded.picker}"
    padding: "6px 10px"
  composer-light:
    backgroundColor: "{colors.surface-light}"
    textColor: "{colors.text-light}"
    typography: "{typography.body}"
    rounded: "{rounded.composer}"
    padding: "14px 16px"
---

# Design System: Spark Code

## Overview

**Creative North Star: "A focused conversation workspace"**

A focused conversation workspace follows the user’s pinned T3 Code and ChatGPT conventions: neutral graphite and white surfaces, a compact rail, readable conversation, and an anchored rounded composer. Spark Code’s existing name and bolt remain the identity. This records the shipped native Rust/Iced application; it does not establish a new brand metaphor.

The interface is quiet and practical. Thin seams define the workspace, selected rows use a tonal fill, and assistant responses sit directly on the transcript background. Settings use the same rail and a consistent hierarchy of page title, section title, explanatory row, and control. The user chooses light or dark appearance and conversation density.

**Key Characteristics:**

- Neutral light and dark endpoints with foreground-colored primary actions.
- Bundled DM Sans; semibold headings and compact regular controls.
- Flat surfaces, thin seams, and restrained rounded corners.
- Open assistant transcript with right-aligned user bubbles.
- Persistent seven-tab settings navigation and a bottom-anchored composer.

Scan-mode evidence: [appearance.rs](src/appearance.rs), [ui.rs](src/ui.rs), [settings_ui.rs](src/settings_ui.rs), [icons.rs](src/icons.rs), [gui.rs](src/gui.rs), and final native captures in [dist/ui-review](dist/ui-review). [PRODUCT.md](PRODUCT.md) confirms the references. No approved comp or quality board supplied additional authority. The separate terminal UI is outside this record.

## Colors

The palette uses graphite in dark appearance and white with a lightly tinted rail in light appearance. Frontmatter names preserve Rust roles with a theme suffix; their hex values are exact endpoints of native RGB interpolation.

### Primary

Theme `text` supplies primary fill and `inverse` supplies content. Hover/press uses `muted` fill. There is no separate global chromatic accent.

### Neutral

| Role | Application |
| --- | --- |
| `bg` | Workspace and bordered fields. |
| `side` | Rail and title bar. |
| `surface` | Outline actions, panels, composer. |
| `raised` | Selected/hovered rows, user bubbles, selection. |
| `border` | Seams and outlines. |
| `text` | Primary content and action fill. |
| `muted` | Explanations, secondary controls, focused field outline. |
| `faint` | Placeholders and supporting metadata. |
| `inverse` | Content on primary fill. |

The OpenAI mark follows theme text; Claude retains official orange. Provider readiness dots and blue enable toggles remain local treatments. General preference toggles use Iced’s generated neutral theme.

**The The Foreground Action Rule.** Primary actions use the theme’s text color as their fill and its inverse color for their content. Provider brand and connection colors remain local to provider identity and state.

## Typography

**Display and Body Font:** bundled DM Sans, loaded from `assets/fonts/DMSans.ttf` as Iced’s default. **Mono Font:** platform-resolved `Font::MONOSPACE` for explicit binary versions and storage paths.

The recurring ramp uses semibold settings headlines and sections; regular conversation/body, row labels, navigation, compact labels, and captions. The default message/composer size is adjustable (13, 15, 17, or 19 logical pixels). No global tracking or line-height scale is authored.

The welcome’s semibold heading (32 logical pixels) is local composition rather than a reusable display token. Small metadata values and Iced Markdown heading/code styles are not expanded into an invented type system.

## Layout

Measurements are Iced logical pixels; frontmatter `px` strings are portable notation.

- **Window:** minimum (920 × 640), custom title bar (37 high), toolbar (51 high).
- **Rail:** fixed (248 wide), inset (12), one-pixel seam. Chat can hide it; settings retain navigation. Projects scroll within a bounded height (64); conversations fill remaining space; bottom controls stay available.
- **Transcript:** centered inner maximum (820), padding (28 vertically, 26 horizontally), message spacing (32 normally, 18 compact); user bubbles have maximum (670).
- **Composer/footer:** centered maximum (860), padding (12 vertically, 25 horizontally); option controls wrap.
- **Settings:** centered maximum (760), padding (32 vertically, 36 horizontally); row padding (18 vertically), copy/control gap (24), label/detail gap (6).
- **Activity:** optional right column (278 wide).

Names are shortened to preserve rail geometry. Welcome suggestions wrap inside a local maximum (720). No mobile substitution or browser breakpoint is implemented. Spacing tokens record reused steps, not a mandatory modular grid.

## Elevation & Depth

The authored workspace uses tonal layers and one-pixel outlines, with no surface shadows. User bubbles use raised fill without outlines. Protected full-access confirmation uses a translucent scrim and bordered panel.

**The The Tonal Seam Rule.** Use the existing surface tones and thin borders to distinguish regions. The shipped application does not author surface shadows.

Theme interpolation lasts (200 ms) with native smoothstep progress. Navigation uses a small linear top-padding movement (7 logical pixels over 180 ms). Reduced motion bypasses both. No additional hover animation or CSS easing is prescribed.

## Shapes

Picker and shortcut corners use `picker`; inputs and ghost/selected rows use `control`; primary/outline actions use `action`. Repeated activity/import panels use `panel`; user bubbles/protected confirmation use `message`; the writing surface uses `composer`.

The send/stop circle is local geometry (31 square, radius 16). Authored icons use a 24-unit vector grid and rounded stroke (1.65), generally at compact sizes (14–18). The bolt is filled; official provider marks are embedded SVGs.

## Components

### Buttons

Primary actions use text fill/inverse content, with muted hover/press. Outline actions use surface/text and a one-pixel border, with raised hover/press. Ghost actions are transparent/muted at rest and raised on hover/press. Selected rows remain raised/text across status.

The recurring compact action padding is (7 vertically, 11 horizontally). Icon actions commonly use hit areas (30 × 28) and descriptive tooltips. Unavailable actions lose their press handler; the shared recipe has no universal disabled-opacity or distinct button-focus treatment.

### Chips / Containers

Shortcut tags are static bordered surface containers with muted text, picker corners, and padding (6 vertically, 10 horizontally). Repeated panels use surface/text, a one-pixel border, panel corners, and padding (13). Settings rows remain open between seams. Welcome suggestions are outline actions rather than a new chip system.

### Inputs / Fields

Binary-path fields use bg/text, faint placeholder, control corners, border outline, and padding (9 vertically, 11 horizontally). Focus changes border to muted; selection uses raised. Rail search is borderless on side. Composer editing is transparent and borderless inside its panel.

Pickers use muted text, faint handles, transparent rest/raised hover, picker corners, and no authored border width. Dropdown menus and general toggles are toolkit-generated.

### Navigation / Settings

Selected rail rows use raised/text; other rows use ghost treatment. Seven tabs persist: General, Appearance, Providers, ChatGPT & Dots, Performance, Data & history, and Keyboard shortcuts. Back stays at the bottom. Titles, sections, label/explanation rows, controls, and seams recur across tabs.

Connection rows combine official marks, readiness explanation, expansion, and enabled state. Expanded content offers binary path and official CLI sign-in/selection. Authentication remains CLI-managed.

### Transcript

Assistant responses remain open with a compact author/action row. User messages align right in raised bubbles with message corners and padding (14 vertically, 18 horizontally). Copy actions and optional timestamps remain secondary.

**The The Open Response Rule.** Assistant responses remain on the transcript background. The rounded raised bubble identifies the user’s message.

### Composer

The anchored composer uses composer corners (22), surface fill, a one-pixel border, and padding (14 vertically, 16 horizontally). Editor height is (67). Supported model, reasoning, and access controls wrap below; send/stop stays right.

Enabled send uses primary/inverse; disabled send uses raised/faint. Running work presents cancellation. The footer holds status and keyboard hints. Missing readiness routes to provider settings; controls reflect official CLI support.

### Portable Previews

The five sidecar HTML/CSS snippets represent native components; they are not the app implementation. Literal dark-theme values are used because the app exposes no CSS variables. DM Sans metrics depend on font availability in the preview host. Native layout negotiation, toolkit menus, application actions, and persistence are omitted. Preview button `:focus-visible` reuses observed hover tone because there is no separate native button-focus recipe; field focus follows the native border change. Synthesized tonal strips are panel metadata, not app colors.

## Do's and Don'ts

### Do:

- **Do** preserve the graphite and white theme roles and foreground-colored primary actions.
- **Do** use bundled DM Sans for interface text and semibold weight for the established heading roles.
- **Do** keep the native rail, transcript, settings, and composer width constraints when extending those surfaces.
- **Do** retain the settings row hierarchy: label and explanation on the left, control on the right, seams between rows.
- **Do** draw functional icons using the existing vector system and preserve the embedded official provider marks.
- **Do** show CLI-managed connection status and supported controls; keep authentication actions with the official CLI.

### Don't:

- **Don't** turn provider brand colors, provider toggle blue, or readiness dots into a global brand accent.
- **Don't** wrap assistant responses in generic cards or add shadows to the established flat workspace.
- **Don't** introduce decorative kickers, glyph icons, or a system display face.
- **Don't** invent mobile breakpoints or browser layout rules for this native desktop application.
- **Don't** expose pasted session-token inputs or imply that ChatGPT-only tools are native Spark Code controls.

Deliberately not canonized: local provider/state colors as global accents, tiny metadata as decorative heading styles, and toolkit-generated values as authored tokens. No craft-floor refusal is promoted into the system.
