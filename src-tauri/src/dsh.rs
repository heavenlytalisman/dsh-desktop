use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
};

const URL_PREFIX: &str = "dsh web: ";

pub fn find_dsh() -> Option<PathBuf> {
    if let Ok(path) = which::which("dsh") {
        return Some(path);
    }
    let home = std::env::var_os("HOME")?;
    let candidate = PathBuf::from(home).join(".local/bin/dsh");
    candidate.is_file().then_some(candidate)
}

/// Spawn `dsh web` and return the child together with the authenticated URL it
/// prints. The URL carries the per-process launch token that satisfies the
/// browser-session fence, so it is loaded directly in the content webview.
pub fn spawn() -> std::io::Result<(Child, String)> {
    let executable = find_dsh().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "dsh was not found on PATH or at ~/.local/bin/dsh",
        )
    })?;
    let mut child = Command::new(executable)
        .args(["web", "--no-open", "--port", "0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("dsh stdout was not piped"))?;
    let mut lines = BufReader::new(stdout).lines();
    let url = loop {
        match lines.next() {
            Some(Ok(line)) => {
                if let Some(url) = line.trim().strip_prefix(URL_PREFIX) {
                    break url.trim().to_string();
                }
            }
            Some(Err(error)) => return Err(error),
            None => {
                let status = child.wait()?;
                return Err(std::io::Error::other(format!(
                    "dsh web exited before printing its URL ({status})"
                )));
            }
        }
    };
    // Drain the rest of stdout so dsh never blocks on a full pipe.
    std::thread::spawn(move || for _ in lines {});
    Ok((child, url))
}
