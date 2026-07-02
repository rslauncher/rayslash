use std::{
    env, fs,
    io::{self, BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
};

const SOCKET_FILE_NAME: &str = "rayslash.sock";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcRequest {
    Show,
    Toggle,
}

impl IpcRequest {
    fn as_line(self) -> &'static str {
        match self {
            Self::Show => "show\n",
            Self::Toggle => "toggle\n",
        }
    }
}

#[derive(Debug)]
pub enum BindSocketError {
    AlreadyRunning,
    Io(io::Error),
}

impl From<io::Error> for BindSocketError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn socket_path() -> PathBuf {
    let runtime_dir = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(fallback_runtime_dir);

    socket_path_in(runtime_dir)
}

pub fn socket_path_in(runtime_dir: impl AsRef<Path>) -> PathBuf {
    runtime_dir.as_ref().join(SOCKET_FILE_NAME)
}

pub fn send_request(path: &Path, request: IpcRequest) -> io::Result<()> {
    let mut stream = UnixStream::connect(path)?;
    stream.write_all(request.as_line().as_bytes())
}

pub fn bind_server_socket(path: &Path) -> Result<UnixListener, BindSocketError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    match UnixListener::bind(path) {
        Ok(listener) => Ok(listener),
        Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
            if UnixStream::connect(path).is_ok() {
                return Err(BindSocketError::AlreadyRunning);
            }

            fs::remove_file(path)?;
            UnixListener::bind(path).map_err(BindSocketError::Io)
        }
        Err(error) => Err(BindSocketError::Io(error)),
    }
}

fn fallback_runtime_dir() -> PathBuf {
    env::temp_dir().join(format!("rayslash-{}", effective_user_id()))
}

fn effective_user_id() -> u32 {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }

    unsafe { geteuid() }
}

pub fn start_server(
    listener: UnixListener,
    on_request: impl Fn(IpcRequest) + Send + 'static,
) -> JoinHandle<()> {
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => match read_request(stream) {
                    Ok(Some(request)) => on_request(request),
                    Ok(None) => {}
                    Err(error) => eprintln!("failed to read rayslash IPC request: {error}"),
                },
                Err(error) => eprintln!("failed to accept rayslash IPC connection: {error}"),
            }
        }
    })
}

pub fn read_request(stream: UnixStream) -> io::Result<Option<IpcRequest>> {
    parse_request_line(&read_line(stream)?)
}

fn read_line(stream: UnixStream) -> io::Result<String> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line)
}

fn parse_request_line(line: &str) -> io::Result<Option<IpcRequest>> {
    match line.trim() {
        "show" => Ok(Some(IpcRequest::Show)),
        "toggle" => Ok(Some(IpcRequest::Toggle)),
        "" => Ok(None),
        command => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown command `{command}`"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::File,
        sync::atomic::{AtomicUsize, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn socket_path_uses_runtime_dir() {
        assert_eq!(
            socket_path_in("/tmp/rayslash-runtime"),
            PathBuf::from("/tmp/rayslash-runtime/rayslash.sock")
        );
    }

    #[test]
    fn fallback_socket_path_uses_user_specific_temp_subdirectory() {
        let path = socket_path_in(fallback_runtime_dir());

        assert!(path.ends_with("rayslash.sock"));
        assert!(
            path.parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("rayslash-"))
        );
    }

    #[test]
    fn binding_socket_creates_parent_directory() {
        let dir = test_dir().join("nested");
        let path = socket_path_in(&dir);

        let listener = bind_server_socket(&path).expect("socket should bind");

        assert!(dir.is_dir());

        drop(listener);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
        let _ = fs::remove_dir(dir.parent().expect("test dir parent"));
    }

    #[test]
    fn stale_socket_path_is_removed_before_binding() {
        let dir = test_dir();
        fs::create_dir_all(&dir).expect("test dir should be created");
        let path = socket_path_in(&dir);
        File::create(&path).expect("stale socket placeholder should be created");

        let listener = bind_server_socket(&path).expect("stale path should be replaced");

        drop(listener);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn active_socket_is_reported_as_running() {
        let dir = test_dir();
        fs::create_dir_all(&dir).expect("test dir should be created");
        let path = socket_path_in(&dir);
        let listener = UnixListener::bind(&path).expect("test listener should bind");

        let error = bind_server_socket(&path).expect_err("active socket should be detected");

        assert!(matches!(error, BindSocketError::AlreadyRunning));

        drop(listener);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn request_lines_are_parsed() {
        assert_eq!(
            parse_request_line("show\n").unwrap(),
            Some(IpcRequest::Show)
        );
        assert_eq!(
            parse_request_line("toggle\n").unwrap(),
            Some(IpcRequest::Toggle)
        );
        assert_eq!(parse_request_line("").unwrap(), None);
        assert!(parse_request_line("open\n").is_err());
    }

    fn test_dir() -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);

        env::temp_dir().join(format!("rayslash-ipc-test-{now}-{count}"))
    }
}
