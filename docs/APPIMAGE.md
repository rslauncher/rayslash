# AppImage Status

AppImage remains deferred for the first public packaging pass.

The normal Linux install layout is now defined by `packaging/linux/inventory.toml`,
the desktop entry, the hicolor SVG icon, and the AppStream metainfo file. Fedora,
Arch/AUR, and Flatpak packaging can install those files directly into standard
locations and validate them with the same metadata commands.

Before adding an AppImage build, decide:

- How `rayslash toggle` should be exposed from the AppImage path users bind to a desktop shortcut.
- Whether AppImage desktop integration should install or update the desktop entry, icon, and metainfo.
- How update metadata and signature expectations will be handled.
- Whether host desktop-entry scanning and icon lookup behave the same when launched from an AppImage mount.
