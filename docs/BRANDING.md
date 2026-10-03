# Spark Code identity

The application's display name is **Spark Code**. Keep that spelling, space, and
capitalization in the native window title, Windows Start/Search shortcuts,
installer, file properties, and installed-apps entry.

The executable/terminal command remains `spark-code`. The desktop executable is
`spark-code-desktop.exe`. Existing lowercase installation, history, and registry
paths are deliberately unchanged. This is a visual identity update, not a data
migration.

## Original icon

`assets/branding/spark-code.svg` is the editable source: an original white
lightning bolt on a near-black (`#111113`) rounded square. It uses no proprietary
logo, font, external image, filter, or online asset. It is covered by this
repository's MIT license.

Use the same bolt geometry for in-app vector rendering. The viewBox is 256×256;
the background rectangle is `(4, 4, 248, 248)` with radius 52. The white path is:

```svg
M144 37L67 141H116L103 219L191 108H139L156 37Z
```

Checked-in derivatives:

- `spark-code.png`: 256×256 RGBA, also suitable for the native window icon
- `spark-code-{16,24,32,48,64,128,256}.png`: individual pixel sizes
- `spark-code.ico`: all seven sizes in one Windows icon

Regenerate with Inkscape and Pillow installed:

```sh
python assets/branding/generate_icons.py
```

The script renders the SVG at 1024px before high-quality downsampling. Builds do
not need Inkscape or Pillow: Cargo and NSIS consume the checked-in derivatives.
`build.rs` embeds the icon and version properties using the registry-published
`winresource` crate for Windows targets, including Linux-to-Windows cross-builds.
Package version fields come from Cargo and are not changed by this branding work.

## Typography

`assets/fonts/DMSans.ttf` is unmodified DM Sans (variable, `opsz` + `wght` axes) from
the [google/fonts](https://github.com/google/fonts/tree/main/ofl/dmsans) OFL release;
the license is `assets/fonts/DMSans-OFL.txt`. Its font-family name is **DM Sans**.
Register the bundled bytes before the native UI renders and use that exact family
name; do not rely on a locally installed copy.

Provider marks in `src/icons.rs` use embedded SVG assets from `assets/icons`:
the OpenAI mark for Codex and the Claude-orange spark for Claude.

The font uses the SIL Open Font License 1.1. The full upstream license is preserved
in `assets/fonts/DMSans-OFL.txt` and shipped by both packagers under
`licenses/dm-sans/OFL.txt`. Other distribution formats that embed the font must also
include this license.

SHA-256 of the upstream font:
`4989b125924991b90d05b2d16e0e388c48f7d5bb8b30539bbf9c755278d0ccaf`

## Windows shell identity

Use stable AppUserModelID `SparkCode.Desktop` before creating a desktop window,
via Windows `SetCurrentProcessExplicitAppUserModelID`. Keep it synchronized with
`SPARK_APP_ID` in `packaging/installer.nsi`. It has no relationship to file or data
paths and does not need a version suffix.

The installer creates `Spark Code/Spark Code.lnk` in the current user's Start
menu, optionally creates `Spark Code.lnk` on the desktop, and uses the executable's
embedded icon. It writes the matching AppUserModelID through Windows
`IPropertyStore`, using official stock NSIS COM/property helper headers and the
built-in System plugin. No third-party NSIS plugin is downloaded. Metadata failure
is logged but leaves the ordinary working shortcut intact.

Upgrades remove only the exact legacy lowercase shortcut filenames. Uninstall
removes only exact known files and shortcuts, retains non-recursive directory
cleanup, and preserves existing PATH ownership checks and user data.

Resource inspection and installer compilation are static checks. A real Windows
10/11 install, Start/Search, taskbar pin/grouping, optional-shortcut, and uninstall
cycle is still needed before claiming those shell behaviors are runtime-tested.

References: [Microsoft AppUserModelID guidance](https://learn.microsoft.com/en-us/windows/win32/shell/appids),
[winresource API](https://docs.rs/winresource/0.1.31/winresource/struct.WindowsResource.html)
