use std::{
    fmt, fs, io,
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::APP_NAME;

const LATEST_RELEASE_API: &str = "https://api.github.com/repos/rslauncher/rayslash/releases/latest";
const DOWNLOAD_LIMIT: u64 = 512 * 1024 * 1024;
const CHECKSUM_LIMIT: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub name: String,
    pub download_url: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppRelease {
    pub version: Version,
    pub page_url: String,
    pub assets: Vec<ReleaseAsset>,
}

impl AppRelease {
    pub fn is_newer_than(&self, current: &Version) -> bool {
        self.version > *current
    }

    pub fn asset(&self, name: &str) -> Option<&ReleaseAsset> {
        self.assets.iter().find(|asset| asset.name == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationKind {
    Flatpak,
    AppImage,
    Rpm,
    Debian,
    Portable,
}

impl InstallationKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Flatpak => "Flatpak",
            Self::AppImage => "AppImage",
            Self::Rpm => "RPM",
            Self::Debian => "Debian package",
            Self::Portable => "portable",
        }
    }

    pub fn asset_name(self, version: &Version) -> Result<String, UpdateError> {
        let architecture = release_architecture()?;
        Ok(match self {
            Self::Flatpak => format!("rayslash-{version}-{architecture}.flatpak"),
            Self::AppImage | Self::Portable => {
                format!("rayslash-{version}-{architecture}.AppImage")
            }
            Self::Rpm => format!("rayslash-{version}-{architecture}.rpm"),
            Self::Debian => {
                let architecture = match architecture {
                    "x86_64" => "amd64",
                    "aarch64" => "arm64",
                    _ => return Err(UpdateError::UnsupportedArchitecture),
                };
                format!("rayslash_{version}_{architecture}.deb")
            }
        })
    }
}

#[derive(Debug)]
pub enum UpdateError {
    Network(String),
    InvalidRelease(String),
    MissingAsset(String),
    ChecksumMismatch,
    Io { path: PathBuf, source: io::Error },
    CommandUnavailable(&'static str),
    InstallFailed(String),
    UnsupportedArchitecture,
    UpdateDirectoryUnavailable,
}

impl fmt::Display for UpdateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(message) => {
                write!(formatter, "could not contact the update service: {message}")
            }
            Self::InvalidRelease(message) => {
                write!(formatter, "invalid release metadata: {message}")
            }
            Self::MissingAsset(name) => write!(formatter, "release asset is missing: {name}"),
            Self::ChecksumMismatch => {
                formatter.write_str("downloaded update checksum did not match")
            }
            Self::Io { path, source } => write!(
                formatter,
                "update I/O failed at {}: {source}",
                path.display()
            ),
            Self::CommandUnavailable(command) => write!(
                formatter,
                "required update command is unavailable: {command}"
            ),
            Self::InstallFailed(message) => write!(formatter, "update installer failed: {message}"),
            Self::UnsupportedArchitecture => {
                formatter.write_str("this CPU architecture is not supported by automatic updates")
            }
            Self::UpdateDirectoryUnavailable => {
                formatter.write_str("the update cache directory is unavailable")
            }
        }
    }
}

impl std::error::Error for UpdateError {}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

pub fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("package version must be valid semver")
}

pub fn fetch_latest_release() -> Result<AppRelease, UpdateError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into();
    let release: GithubRelease = agent
        .get(LATEST_RELEASE_API)
        .header("Accept", "application/vnd.github+json")
        .header(
            "User-Agent",
            concat!("rayslash/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|error| UpdateError::Network(error.to_string()))?
        .into_body()
        .read_json()
        .map_err(|error| UpdateError::InvalidRelease(error.to_string()))?;
    release_from_github(release)
}

fn release_from_github(release: GithubRelease) -> Result<AppRelease, UpdateError> {
    if release.draft || release.prerelease {
        return Err(UpdateError::InvalidRelease(
            "latest endpoint returned a draft or prerelease".into(),
        ));
    }
    let version = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name);
    let version =
        Version::parse(version).map_err(|error| UpdateError::InvalidRelease(error.to_string()))?;
    if release.html_url.trim().is_empty() {
        return Err(UpdateError::InvalidRelease(
            "release page URL is empty".into(),
        ));
    }
    let assets = release
        .assets
        .into_iter()
        .filter(|asset| {
            asset.size > 0
                && asset.size <= DOWNLOAD_LIMIT
                && asset
                    .browser_download_url
                    .starts_with("https://github.com/")
        })
        .map(|asset| ReleaseAsset {
            name: asset.name,
            download_url: asset.browser_download_url,
            size: asset.size,
        })
        .collect::<Vec<_>>();
    if !assets.iter().any(|asset| asset.name == "SHA256SUMS") {
        return Err(UpdateError::MissingAsset("SHA256SUMS".into()));
    }
    Ok(AppRelease {
        version,
        page_url: release.html_url,
        assets,
    })
}

pub fn detect_installation(current_executable: &Path) -> InstallationKind {
    if std::env::var_os("FLATPAK_ID").is_some() {
        return InstallationKind::Flatpak;
    }
    if std::env::var_os("APPIMAGE").is_some() {
        return InstallationKind::AppImage;
    }
    if dirs::home_dir().is_some_and(|home| current_executable.starts_with(home)) {
        return InstallationKind::Portable;
    }
    if command_succeeds("rpm", &["-q", APP_NAME]) {
        InstallationKind::Rpm
    } else if command_succeeds("dpkg-query", &["-W", APP_NAME]) {
        InstallationKind::Debian
    } else {
        InstallationKind::Portable
    }
}

pub fn download_verified_asset(
    release: &AppRelease,
    kind: InstallationKind,
) -> Result<PathBuf, UpdateError> {
    let asset_name = kind.asset_name(&release.version)?;
    let asset = release
        .asset(&asset_name)
        .ok_or_else(|| UpdateError::MissingAsset(asset_name.clone()))?;
    let checksums = release
        .asset("SHA256SUMS")
        .ok_or_else(|| UpdateError::MissingAsset("SHA256SUMS".into()))?;
    let checksum_bytes = download_bytes(checksums, CHECKSUM_LIMIT)?;
    let checksum_text = std::str::from_utf8(&checksum_bytes)
        .map_err(|error| UpdateError::InvalidRelease(error.to_string()))?;
    let expected = checksum_for(checksum_text, &asset_name)
        .ok_or_else(|| UpdateError::MissingAsset(format!("checksum for {asset_name}")))?;

    let directory = dirs::cache_dir()
        .map(|path| {
            path.join(APP_NAME)
                .join("updates")
                .join(release.version.to_string())
        })
        .ok_or(UpdateError::UpdateDirectoryUnavailable)?;
    fs::create_dir_all(&directory).map_err(|source| io_error(&directory, source))?;
    let destination = directory.join(&asset_name);
    if destination.is_file() && sha256_file(&destination)? == expected {
        return Ok(destination);
    }
    let temporary = directory.join(format!(".{asset_name}.download"));
    let actual = download_file(asset, &temporary)?;
    if actual != expected {
        let _ = fs::remove_file(&temporary);
        return Err(UpdateError::ChecksumMismatch);
    }
    fs::rename(&temporary, &destination).map_err(|source| io_error(&destination, source))?;
    Ok(destination)
}

pub fn install_downloaded_update(
    kind: InstallationKind,
    downloaded: &Path,
    current_executable: &Path,
) -> Result<(), UpdateError> {
    match kind {
        InstallationKind::AppImage => {
            let target = std::env::var_os("APPIMAGE")
                .map(PathBuf::from)
                .unwrap_or_else(|| current_executable.to_path_buf());
            install_portable(downloaded, &target)
        }
        InstallationKind::Portable => {
            let target =
                if dirs::home_dir().is_some_and(|home| current_executable.starts_with(home)) {
                    current_executable.to_path_buf()
                } else {
                    dirs::home_dir()
                        .map(|home| home.join(".local/bin/rayslash"))
                        .ok_or(UpdateError::UpdateDirectoryUnavailable)?
                };
            install_portable(downloaded, &target)
        }
        InstallationKind::Rpm => run_installer("pkexec", &["dnf", "install", "-y"], downloaded),
        InstallationKind::Debian => {
            run_installer("pkexec", &["apt-get", "install", "-y"], downloaded)
        }
        InstallationKind::Flatpak => run_installer(
            "flatpak-spawn",
            &[
                "--host",
                "flatpak",
                "install",
                "--user",
                "--noninteractive",
                "--reinstall",
            ],
            downloaded,
        ),
    }
}

fn install_portable(downloaded: &Path, target: &Path) -> Result<(), UpdateError> {
    let parent = target
        .parent()
        .ok_or(UpdateError::UpdateDirectoryUnavailable)?;
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    let staged = parent.join(".rayslash.update");
    fs::copy(downloaded, &staged).map_err(|source| io_error(&staged, source))?;
    fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))
        .map_err(|source| io_error(&staged, source))?;
    let backup = parent.join(".rayslash.previous");
    let had_target = target.is_file();
    if had_target {
        let _ = fs::remove_file(&backup);
        fs::rename(target, &backup).map_err(|source| io_error(target, source))?;
    }
    if let Err(source) = fs::rename(&staged, target) {
        if had_target {
            let _ = fs::rename(&backup, target);
        }
        return Err(io_error(target, source));
    }
    Ok(())
}

fn run_installer(
    program: &'static str,
    arguments: &[&str],
    downloaded: &Path,
) -> Result<(), UpdateError> {
    let mut command = Command::new(program);
    command
        .args(arguments)
        .arg(downloaded)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let status = command.status().map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            UpdateError::CommandUnavailable(program)
        } else {
            io_error(Path::new(program), error)
        }
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(UpdateError::InstallFailed(status.to_string()))
    }
}

fn download_bytes(asset: &ReleaseAsset, limit: u64) -> Result<Vec<u8>, UpdateError> {
    if asset.size > limit {
        return Err(UpdateError::InvalidRelease(format!(
            "{} exceeds the download limit",
            asset.name
        )));
    }
    http_agent()
        .get(&asset.download_url)
        .header(
            "User-Agent",
            concat!("rayslash/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|error| UpdateError::Network(error.to_string()))?
        .into_body()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(|error| UpdateError::Network(error.to_string()))
}

fn download_file(asset: &ReleaseAsset, destination: &Path) -> Result<String, UpdateError> {
    if asset.size > DOWNLOAD_LIMIT {
        return Err(UpdateError::InvalidRelease(format!(
            "{} exceeds the download limit",
            asset.name
        )));
    }
    let response = http_agent()
        .get(&asset.download_url)
        .header(
            "User-Agent",
            concat!("rayslash/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|error| UpdateError::Network(error.to_string()))?;
    let mut reader = response.into_body().into_reader();
    let mut file = fs::File::create(destination).map_err(|source| io_error(destination, source))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| UpdateError::Network(error.to_string()))?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        if total > DOWNLOAD_LIMIT || total > asset.size {
            return Err(UpdateError::InvalidRelease(format!(
                "{} has an unexpected size",
                asset.name
            )));
        }
        file.write_all(&buffer[..read])
            .map_err(|source| io_error(destination, source))?;
        hasher.update(&buffer[..read]);
    }
    if total != asset.size {
        return Err(UpdateError::InvalidRelease(format!(
            "{} has an unexpected size",
            asset.name
        )));
    }
    file.sync_all()
        .map_err(|source| io_error(destination, source))?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn checksum_for(contents: &str, asset_name: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let (checksum, name) = line.split_once(char::is_whitespace)?;
        let name = name.trim_start().trim_start_matches('*');
        (name == asset_name
            && checksum.len() == 64
            && checksum.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| checksum.to_ascii_lowercase())
    })
}

fn sha256_file(path: &Path) -> Result<String, UpdateError> {
    let mut file = fs::File::open(path).map_err(|source| io_error(path, source))?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher).map_err(|source| io_error(path, source))?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn http_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(120)))
        .build()
        .into()
}

fn release_architecture() -> Result<&'static str, UpdateError> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("x86_64"),
        "aarch64" => Ok("aarch64"),
        _ => Err(UpdateError::UnsupportedArchitecture),
    }
}

fn command_succeeds(program: &str, arguments: &[&str]) -> bool {
    Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn io_error(path: &Path, source: io::Error) -> UpdateError {
    UpdateError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn checksum_parser_requires_an_exact_asset_name() {
        let contents = format!("{}  rayslash-1.2.3-x86_64.AppImage\n", "a".repeat(64));
        assert_eq!(
            checksum_for(&contents, "rayslash-1.2.3-x86_64.AppImage"),
            Some("a".repeat(64))
        );
        assert_eq!(checksum_for(&contents, "rayslash-1.2.3-x86_64.rpm"), None);
    }

    #[test]
    fn release_assets_follow_packaging_names() {
        let version = Version::new(1, 2, 3);
        let architecture = release_architecture().unwrap();
        assert_eq!(
            InstallationKind::Rpm.asset_name(&version).unwrap(),
            format!("rayslash-1.2.3-{architecture}.rpm")
        );
        assert_eq!(
            InstallationKind::AppImage.asset_name(&version).unwrap(),
            format!("rayslash-1.2.3-{architecture}.AppImage")
        );
    }

    #[test]
    fn latest_version_comparison_uses_semver() {
        let release = AppRelease {
            version: Version::new(0, 3, 0),
            page_url: "https://github.com/rslauncher/rayslash/releases/tag/v0.3.0".into(),
            assets: Vec::new(),
        };
        assert!(release.is_newer_than(&Version::new(0, 2, 9)));
        assert!(!release.is_newer_than(&Version::new(0, 3, 0)));
    }

    #[test]
    fn portable_install_replaces_the_executable_and_keeps_a_backup() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "rayslash-update-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create update test directory");
        let downloaded = directory.join("downloaded.AppImage");
        let target = directory.join("rayslash");
        fs::write(&downloaded, b"new build").expect("write downloaded update");
        fs::write(&target, b"old build").expect("write current executable");

        install_portable(&downloaded, &target).expect("install portable update");

        assert_eq!(fs::read(&target).unwrap(), b"new build");
        assert_eq!(
            fs::read(directory.join(".rayslash.previous")).unwrap(),
            b"old build"
        );
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o755
        );
        fs::remove_dir_all(directory).expect("remove update test directory");
    }
}
