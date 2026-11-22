//! Minimal niri IPC client. niri exposes a Unix socket in `$NIRI_SOCKET`;
//! requests and replies are one JSON value per line.

use std::io::{BufRead, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

/// Path from `$NIRI_SOCKET`, or `None` when raidzbar was not started by niri.
pub fn socket_path() -> Option<PathBuf> {
    std::env::var_os("NIRI_SOCKET")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Send one request line and return the single-line reply.
pub fn request(socket: &Path, request: &str) -> Option<String> {
    let mut stream = UnixStream::connect(socket).ok()?;
    stream.write_all(format!("{request}\n").as_bytes()).ok()?;
    stream.flush().ok()?;

    let mut reader = std::io::BufReader::new(stream);
    let mut reply = String::new();
    if reader.read_line(&mut reply).ok()? == 0 {
        return None;
    }
    Some(reply.trim().to_string())
}
