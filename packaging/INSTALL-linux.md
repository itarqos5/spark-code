# Install spark-code 1.0.0 on Linux

This x86-64 release is built and no-account launch-tested on Ubuntu 24.04. It is a native dynamically linked build, not an AppImage or a universal static binary. The binary requires glibc 2.39 or newer; older distributions may require a source build. ARM64 is not included.

## Desktop dependencies

Use an X11 desktop, or XWayland within a Wayland desktop. A pure Wayland session without XWayland is not supported by this build. On Ubuntu 24.04 the desktop libraries can be installed with:

```sh
sudo apt install libx11-6 libx11-xcb1 libxcb1 libxcb-render0 libxcb-shape0 libxcb-xfixes0 libxcb-randr0 libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libwayland-cursor0 libwayland-egl1
```

File/folder dialogs use `xdg-desktop-portal` and the backend appropriate to your desktop, for example `xdg-desktop-portal-gtk`. Most full desktop installations already supply these. A working D-Bus desktop session is needed for portal dialogs. GUI startup in CI does not test every desktop or portal backend.

## Extract and run

Download the tarball and Linux checksum file from the same GitHub release, then:

```sh
sha256sum -c spark-code-1.0.0-linux-x64-SHA256SUMS.txt
tar -xzf spark-code-1.0.0-linux-x64.tar.gz
cd spark-code-1.0.0-linux-x64
./spark-code-desktop  # graphical workspace
./spark-code          # terminal workspace
./spark-code gui      # also opens the graphical workspace
./spark-code --help
```

Keep both executables and bundled documentation/licenses together. No root installation, PATH modification, service, or desktop shortcut is performed. You can add the extracted directory to your shell's PATH yourself if desired.

Provider tools are optional and separately installed: [Codex CLI](https://developers.openai.com/codex/cli/) and [Claude Code](https://code.claude.com/docs/en/setup). Use their supported subscription sign-in, then configure the executable paths in spark-code Settings and select Refresh. Provider installation and sign-in never run automatically.

History is stored under `$XDG_DATA_HOME/spark-code` or `~/.local/share/spark-code` by default, separately from the application. `SPARK_CODE_DATA_DIR` can select a different data directory. The database and exports are unencrypted; keep backups private.

To remove this unpacked application, close it, delete the extracted folder, and remove any PATH entry you added. This does not delete history, project files, or provider configuration. See `FEATURES.md` and `RELEASE_NOTES.md` for the implementation boundaries.
