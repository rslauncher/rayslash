# Desktop Shortcuts

`rayslash` does not capture global shortcuts itself. On Wayland, applications should not try to listen for arbitrary global key presses; the desktop environment owns global shortcut binding.

Install `rayslash` locally before binding the shortcut. See [INSTALL.md](INSTALL.md) for the local install command and PATH verification steps.

Then bind your desktop shortcut to:

```sh
rayslash toggle
```

During development, you can test the same command from the project checkout with:

```sh
cargo run -p rayslash -- toggle
```

Desktop shortcuts should use the installed binary command, not `cargo run`:

```sh
rayslash toggle
```

## GNOME

1. Open Settings.
2. Go to Keyboard.
3. Open Keyboard Shortcuts or Custom Shortcuts.
4. Add a custom shortcut.
5. Set the name to `rayslash`.
6. Set the command to `rayslash toggle`.
7. Set the shortcut to `Super+\`.

## KDE Plasma

1. Open System Settings.
2. Go to Keyboard, Shortcuts, or Custom Shortcuts, depending on your Plasma version.
3. Add a custom command shortcut.
4. Set the command to `rayslash toggle`.
5. Set the shortcut to `Super+\`.

## Resident Socket

The resident instance uses this local IPC socket:

```sh
$XDG_RUNTIME_DIR/rayslash.sock
```

Users generally do not need to manage this socket manually. `rayslash` creates it for the resident process and removes stale socket paths during startup when no live process responds.

If `XDG_RUNTIME_DIR` is unavailable, `rayslash` uses a user-specific subdirectory under the system temp directory instead of a shared `/tmp/rayslash.sock` path.
