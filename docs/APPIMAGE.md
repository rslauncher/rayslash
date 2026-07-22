# AppImage

Release tags build architecture-specific AppImages with the pinned module host, desktop entry, scalable icon, AppStream metadata, and required non-system shared libraries. The build uses a digest-pinned `linuxdeploy` release and runs on Ubuntu 22.04 to retain a conservative glibc baseline.

The bundled `AppRun` adds the internal binary directory to `PATH`, points `RAYSLASH_MODULE_HOST` at the bundled host, and forwards all arguments. After downloading and verifying the release:

```sh
chmod +x rayslash-0.2.0-x86_64.AppImage
./rayslash-0.2.0-x86_64.AppImage toggle
```

Bind the absolute AppImage path followed by `toggle` to the desktop shortcut. Updating is an explicit download-and-replace operation; the project does not silently install updates or publish a zsync stream.
