Name:           rayslash
Version:        0.2.7
Release:        1%{?dist}
Summary:        Fast native Linux desktop launcher
%global         module_host_version 0.1.4
# Supersede standalone host RPMs through 0.1.4-1 without self-obsoleting.
%global         module_host_package_release 2
%ifarch x86_64
%global         module_host_target x86_64-unknown-linux-gnu
%else
%global         module_host_target aarch64-unknown-linux-gnu
%endif

License:        MIT
URL:            https://github.com/rslauncher/rayslash
ExclusiveArch:  x86_64 aarch64
Provides:       rayslash-module-host = %{module_host_version}-%{module_host_package_release}
Provides:       bundled(rayslash-module-host) = %{module_host_version}
Obsoletes:      rayslash-module-host < %{module_host_version}-%{module_host_package_release}
Source0:        %{name}-%{version}.tar.gz
Source1:        %{name}-%{version}-vendor.tar.xz
Source2:        https://github.com/rslauncher/rayslash-module-host/releases/download/v%{module_host_version}/rayslash-module-host-v%{module_host_version}-x86_64-unknown-linux-gnu.tar.xz
Source3:        https://github.com/rslauncher/rayslash-module-host/releases/download/v%{module_host_version}/rayslash-module-host-v%{module_host_version}-aarch64-unknown-linux-gnu.tar.xz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc
BuildRequires:  fontconfig-devel
BuildRequires:  desktop-file-utils
BuildRequires:  appstream

%description
rayslash is a lightweight keyboard-first launcher for Linux desktops. It
searches installed desktop applications and configured folders from a compact
native Slint window. The required sandbox host is included; optional
capabilities are installed separately as modules.

%prep
%autosetup -a 1
install -Dm0644 packaging/fedora/cargo-config.toml .cargo/config.toml
%ifarch x86_64
tar --extract --xz --file %{SOURCE2}
%else
tar --extract --xz --file %{SOURCE3}
%endif

%build
cargo build --release --frozen --jobs 2 -p rayslash

%install
install -Dm0755 target/release/rayslash %{buildroot}%{_bindir}/rayslash
install -Dm0755 \
  rayslash-module-host-v%{module_host_version}-%{module_host_target}/rayslash-module-host \
  %{buildroot}%{_libexecdir}/rayslash/rayslash-module-host
install -Dm0755 \
  rayslash-module-host-v%{module_host_version}-%{module_host_target}/rayslash-module-compiler \
  %{buildroot}%{_libexecdir}/rayslash/rayslash-module-compiler
install -Dm0644 packaging/linux/dev.rayan6ms.rayslash.desktop %{buildroot}%{_datadir}/applications/dev.rayan6ms.rayslash.desktop
install -Dm0644 icons/rayslash-icon.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/dev.rayan6ms.rayslash.svg
install -Dm0644 packaging/linux/dev.rayan6ms.rayslash.metainfo.xml %{buildroot}%{_metainfodir}/dev.rayan6ms.rayslash.metainfo.xml

%check
# The packaged thin-LTO graph is no longer needed after %%install. Remove it
# before compiling test harnesses so the two graphs cannot exhaust CI storage.
cargo clean --release
CARGO_PROFILE_RELEASE_LTO=false \
CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
CARGO_PROFILE_RELEASE_STRIP=true \
RUSTFLAGS="-Cdebuginfo=0" \
  cargo test --release --frozen --jobs 2 --workspace
desktop-file-validate packaging/linux/dev.rayan6ms.rayslash.desktop
appstreamcli validate --no-net packaging/linux/dev.rayan6ms.rayslash.metainfo.xml
test -x %{buildroot}%{_libexecdir}/rayslash/rayslash-module-host
test -x %{buildroot}%{_libexecdir}/rayslash/rayslash-module-compiler

%files
%license LICENSE*
%{_bindir}/rayslash
%{_libexecdir}/rayslash/rayslash-module-host
%{_libexecdir}/rayslash/rayslash-module-compiler
%{_datadir}/applications/dev.rayan6ms.rayslash.desktop
%{_datadir}/icons/hicolor/scalable/apps/dev.rayan6ms.rayslash.svg
%{_metainfodir}/dev.rayan6ms.rayslash.metainfo.xml

%changelog
* Sat Aug 08 2026 RaySlash contributors - 0.2.7-1
- Tighten and align diagnostics, module status, and About layouts.

* Sat Aug 08 2026 RaySlash contributors - 0.2.6-1
- Refine update indicators, diagnostics readability, and release information layout.

* Sat Aug 08 2026 RaySlash contributors - 0.2.5-1
- Add verified in-app updates, update notifications, version information, and live project discovery.

* Thu Aug 06 2026 RaySlash contributors - 0.2.4-1
- Add privacy-conscious application discovery diagnostics and fix Ctrl+Enter input state.

* Sun Jul 26 2026 RaySlash contributors - 0.2.3-1
- Ship the split module compiler, persistent timer notifications, and optimized runtime paths.

* Sun Jul 26 2026 RaySlash contributors - 0.2.2-1
- Polish result stability, ranking, module queries, country time zones, and settings layout.

* Sat Jul 25 2026 RaySlash contributors - 0.2.1-1
- Ship the optimized launcher and host with consistent module icons.

* Fri Jul 24 2026 RaySlash contributors - 0.2.0-2
- Bundle the verified module host and publish one self-contained app RPM.

* Wed Jul 22 2026 RaySlash contributors - 0.2.0-1
- Refine settings and module management and add multi-format release packaging.

* Tue Jul 14 2026 RaySlash contributors - 0.1.1-1
- Publish complete architecture-matched app and module-host package sets.

* Mon Jul 13 2026 RaySlash contributors - 0.1.0-2
- Require the module host and build from vendored dependencies without network access.

* Fri Jul 03 2026 rayan6ms <rayan6ms@example.invalid> - 0.1.0-1
- Initial Fedora packaging.
