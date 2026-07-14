Name:           rayslash
Version:        0.1.0
Release:        2%{?dist}
Summary:        Fast native Linux desktop launcher

License:        MIT
URL:            https://github.com/rslauncher/rayslash
Requires:       rayslash-module-host >= 0.1.2
Source0:        %{name}-%{version}.tar.gz
Source1:        %{name}-%{version}-vendor.tar.xz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc
BuildRequires:  fontconfig-devel
BuildRequires:  desktop-file-utils
BuildRequires:  appstream

%description
rayslash is a lightweight keyboard-first launcher for Linux desktops. It
searches installed desktop applications and configured folders from a compact
native Slint window. Optional capabilities are installed as modules.

%prep
%autosetup -a 1
install -Dm0644 packaging/fedora/cargo-config.toml .cargo/config.toml

%build
cargo build --release --frozen --jobs 2 -p rayslash

%install
install -Dm0755 target/release/rayslash %{buildroot}%{_bindir}/rayslash
install -Dm0644 packaging/linux/dev.rayan6ms.rayslash.desktop %{buildroot}%{_datadir}/applications/dev.rayan6ms.rayslash.desktop
install -Dm0644 icons/rayslash-icon.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/dev.rayan6ms.rayslash.svg
install -Dm0644 packaging/linux/dev.rayan6ms.rayslash.metainfo.xml %{buildroot}%{_metainfodir}/dev.rayan6ms.rayslash.metainfo.xml

%check
cargo test --release --frozen --jobs 2 --workspace
desktop-file-validate packaging/linux/dev.rayan6ms.rayslash.desktop
appstreamcli validate --no-net packaging/linux/dev.rayan6ms.rayslash.metainfo.xml

%files
%license LICENSE*
%doc docs/INSTALL.md docs/PACKAGING.md docs/SHORTCUTS.md
%{_bindir}/rayslash
%{_datadir}/applications/dev.rayan6ms.rayslash.desktop
%{_datadir}/icons/hicolor/scalable/apps/dev.rayan6ms.rayslash.svg
%{_metainfodir}/dev.rayan6ms.rayslash.metainfo.xml

%changelog
* Mon Jul 13 2026 RaySlash contributors - 0.1.0-2
- Require the module host and build from vendored dependencies without network access.

* Fri Jul 03 2026 rayan6ms <rayan6ms@example.invalid> - 0.1.0-1
- Initial Fedora packaging.
