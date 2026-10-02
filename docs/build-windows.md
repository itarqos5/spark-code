# Windows build and packaging

## Native Windows build (preferred release route)

Build on Windows x64 with Rust stable from [rustup](https://rustup.rs/), Microsoft's C++ build tools/Windows SDK, Python 3.9 or newer, and [NSIS 3.11](https://nsis.sourceforge.io/Download). CI uses the Windows 2022 runner. Rust stable is recorded in the job log; dependencies are pinned in `Cargo.lock`. A later reproducibility pass should pin the exact successful Rust toolchain, rather than guessing a version unavailable to contributors.

```powershell
rustup target add x86_64-pc-windows-msvc
cargo fmt --all -- --check
$env:RUSTFLAGS = '-C target-feature=+crt-static'
cargo test --locked --target x86_64-pc-windows-msvc --all-targets
cargo build --locked --release --target x86_64-pc-windows-msvc --bins
.\packaging\package-windows.ps1
.\packaging\smoke-windows.ps1
```

The static CRT flag avoids a separate MSVC runtime installation. The packager independently audits PE imports and fails if a non-system DLL is unresolved. If changing link flags introduces a runtime dependency, provide the corresponding redistributable DLL and license with `-DllDir` and `-RuntimeNotice`, or fix the build; never copy arbitrary Windows system DLLs.

Outputs under `dist/`:

- `spark-code-<version>-windows-x64-setup.exe`
- `spark-code-<version>-windows-x64-portable.zip`
- `spark-code-<version>-windows-x64-SHA256SUMS.txt`
- `windows-smoke.json` after the Windows smoke script runs

The ZIP contains both executable files, the installation guide, project license, available crate license files and dependency notices, a per-file SHA-256 manifest, and an import audit. The installer uses the exact same payload. Neither package includes a provider CLI, token, sign-in session, account settings, project, or user history. This package is unsigned.

## Linux cross-build

Cross-building needs Rust's `x86_64-pc-windows-gnu` target, the corresponding MinGW-w64 GCC/binutils, native Linux dependencies required by build scripts, Python, and NSIS. `.cargo/config.toml` selects `x86_64-w64-mingw32-gcc` for this target.

```bash
rustup target add x86_64-pc-windows-gnu
cargo build --locked --release --target x86_64-pc-windows-gnu --bins
bash packaging/package-windows.sh
```

Set `MAKENSIS` to an alternate NSIS executable and `NSISDIR` when using an extracted NSIS installation. `SPARK_BINARY_DIR`, `SPARK_TARGET`, and `SPARK_OUTPUT` can override the defaults. For a toolchain requiring runtime DLLs, add explicit arguments such as:

```bash
bash packaging/package-windows.sh \
  --dll-dir /path/to/mingw/runtime \
  --runtime-notice /path/to/runtime/copyright
```

The script recursively inspects imports in executables and bundled DLLs, verifies AMD64 PE32+, and copies only non-system imports from explicitly provided directories. Any DLL bundle requires its matching redistribution notice. The presently inspected GNU release imports only Windows system libraries, so no MinGW runtime DLL is needed. Recheck this for every changed build. Cross-compilation establishes packaging/build status only; run the native checks on Windows.

## Installer and PATH safety

- Per-user installation, `RequestExecutionLevel user`, default `%LOCALAPPDATA%\Programs\spark-code`
- Start-menu GUI shortcut, optional desktop shortcut, and optional selected-by-default user PATH component
- PATH helper uses .NET registry operations to preserve unexpanded values and avoid NSIS's string-length limit
- Existing matching PATH entries are not claimed; uninstall removes only the entry recorded as added by this installation
- Uninstall has an exact generated file list and uses only non-recursive directory removals
- `%LOCALAPPDATA%\spark-code` and provider/project data are outside the removal list
- No hidden app launch, provider install, provider sign-in, credentials, service, telemetry agent, or scheduled task is added by Setup

The PowerShell helper is run for one process with execution-policy bypass so a downloaded helper can run under standard user settings. It does not change the machine/user policy, and organization-enforced policy can still block it. A blocked uninstall stops before removing application files.

## CI and checks still requiring a user machine

`.github/workflows/build.yml` runs format/tests on Linux and Windows, builds the actual release binaries, packages them, checks `spark-code --help`, and opens/closes the GUI with an isolated temporary history directory. It records 15 idle samples of working set, private bytes, cumulative CPU seconds, and direct child processes. It does not start a provider, sign into an account, submit prompts, or perform exhaustive visual QA. TUI idle RAM is explicitly unmeasured because that requires an interactive terminal host.

Artifact upload uses a commit-pinned official GitHub Action with read-only repository permissions. The workflow does not publish a GitHub Release or request a paid external service. CI runner OS/Rust package updates mean this is a traceable build, not a guarantee of byte-for-byte reproducibility. SHA-256 checksums verify file integrity, not identity.

Before describing a release as Windows 10 tested, run an install/launch/PATH/uninstall cycle on an actual Windows 10 x64 machine. Verify history survives uninstall, pre-existing PATH entries survive, a long PATH survives, and a fresh terminal resolves exactly `spark-code`. Provider-connected workflows and visual checks remain separate from no-account CI smoke checks. Memory results are measurements for the recorded runner/scenario, not a maximum RAM guarantee.
