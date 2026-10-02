# Install spark-code on Windows

## Requirements and current validation

- Windows 10 or Windows 11, x64; Windows 10 22H2 is the intended minimum testing baseline
- A terminal for the terminal UI; no browser engine or Electron runtime is bundled
- Codex and Claude Code are optional, separately installed official command-line tools

This is an early, unsigned build. Cross-compilation, installer generation, and a successful CI run do **not** establish full Windows 10 runtime compatibility. Consult the exact build's test results. A Windows 11/Server CI smoke check does not replace a Windows 10 check. No paid code-signing service is used. If Windows displays an unknown-publisher warning, verify the source and hashes before deciding whether to proceed; do not disable Windows security features.

## Setup installer

1. Download `spark-code-<version>-windows-x64-setup.exe` and its matching `SHA256SUMS.txt` from the same build.
2. Verify the file hash, for example `Get-FileHash .\spark-code-<version>-windows-x64-setup.exe -Algorithm SHA256`, and compare the whole hash with the published checksum. A checksum detects corruption; it is not publisher authentication.
3. Close any running spark-code windows and terminal sessions, then run Setup as your normal user. Administrator rights are not requested.
4. Keep **Add spark-code to my PATH** selected to enable the terminal command. The desktop shortcut is optional; Setup always adds a Start-menu shortcut for the graphical app.
5. The default installation folder is `%LOCALAPPDATA%\Programs\spark-code`. Open a fresh terminal after installation.

Commands:

```powershell
spark-code          # terminal UI
spark-code gui      # graphical UI
spark-code --help   # command help
```

Desktop and Start-menu shortcuts open `spark-code-desktop.exe` directly, without a terminal window. If PATH has not refreshed, close all terminal application windows and reopen one, or sign out and back in. You can always open the desktop app from the Start menu. Run `Get-Command spark-code` to check which executable will run.

Setup modifies only the current user's PATH, never the machine-wide PATH. It preserves long PATH values and does not claim an entry that already existed. Do not install inside a project folder or the history folder. Uninstall the previous copy before moving an installed app to a different directory.

## Portable ZIP

Extract `spark-code-<version>-windows-x64-portable.zip` completely before running either executable. Keep both executables, any bundled DLLs, and the license files together.

- Double-click `spark-code-desktop.exe` for the graphical app
- Open a terminal in the extracted folder and run `.\spark-code.exe` for the terminal UI
- `.\spark-code.exe gui` also opens the graphical app
- Portable mode does not change PATH or create shortcuts automatically

To use the bare `spark-code` command from any directory with a portable copy, add that extracted folder to your **user** PATH using Windows' Environment Variables settings. Remove that entry yourself if you later move/delete the portable folder. Local history still lives in `%LOCALAPPDATA%\spark-code`; “portable” refers to the application files, not a self-contained history database.

## Provider setup is your choice

Install and sign in to the official providers separately, following their own instructions:

- [OpenAI Codex CLI](https://developers.openai.com/codex/cli/)
- [Anthropic Claude Code setup](https://code.claude.com/docs/en/setup)

spark-code never installs a provider, signs in, extracts credentials, or imports old conversations automatically. Use the provider's supported subscription sign-in flow. Provider access, usage limits, features, and charges are governed by your account and the provider's terms; spark-code does not reset quotas or promise free usage. Import, where offered, is initiated by you in Settings with a preview.

## Uninstall and data retention

Use **Settings → Apps → spark-code → Uninstall**, or `Uninstall.exe` in the install folder. Close spark-code first.

The uninstaller removes the exact application files and app shortcuts, and removes the user PATH entry only if this installation added it. It does not recursively erase the install folder. Unrecognized files you created there are left behind.

Your history in `%LOCALAPPDATA%\spark-code`, project folders, and provider sign-in/configuration files are preserved. Delete local history yourself only if you intend to discard it. For a portable copy, remove the extracted application folder and any PATH entry you added; local history remains.

If PATH cleanup is blocked by your organization's PowerShell policy, uninstall stops before removing files. Remove only this application's exact folder from your user PATH in Windows' Environment Variables settings, then retry. If PowerShell is still blocked, keep the helper/installer and ask your administrator for the supported removal method; do not weaken policy.

## First-run checks

A useful manual Windows 10 check is: install without elevation, open both interfaces, open/close a task, check `spark-code` from a newly opened terminal, and uninstall while verifying existing history and unrelated PATH entries remain. Provider-connected checks require your separate sign-in and approval; CI does not use real accounts.
