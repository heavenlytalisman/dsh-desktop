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

/// Find the node binary. dsh is a Node.js script (#!/usr/bin/env node),
/// so node must be on PATH when spawning it. When launched from a desktop
/// environment (KDE Kickoff, systemd, etc.) the PATH is minimal and node
/// is typically missing.
pub fn find_node() -> Option<PathBuf> {
    if let Ok(path) = which::which("node") {
        return Some(path);
    }
    let home = std::env::var_os("HOME")?;
    // fnm (Fast Node Manager)
    let fnm_dir = PathBuf::from(&home).join(".local/share/fnm");
    if let Ok(entries) = std::fs::read_dir(fnm_dir.join("node-versions")) {
        for entry in entries.flatten() {
            let node = entry.path().join("installation/bin/node");
            if node.is_file() {
                return Some(node);
            }
        }
    }
    // nvm
    let nvm_dir = PathBuf::from(&home).join(".nvm/versions/node");
    if let Ok(entries) = std::fs::read_dir(&nvm_dir) {
        for entry in entries.flatten() {
            let node = entry.path().join("bin/node");
            if node.is_file() {
                return Some(node);
            }
        }
    }
    // Direct install locations
    for candidate in [
        PathBuf::from(&home).join(".local/bin/node"),
        PathBuf::from("/usr/local/bin/node"),
        PathBuf::from("/usr/bin/node"),
    ] {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    // Hermes bundled node
    let hermes = PathBuf::from(&home).join(".hermes/tools");
    if let Ok(entries) = std::fs::read_dir(&hermes) {
        for entry in entries.flatten() {
            let node = entry.path().join("bin/node");
            if node.is_file() {
                return Some(node);
            }
        }
    }
    None
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
    // Build a PATH that includes node and dsh locations, since desktop
    // environments (KDE Kickoff, systemd) have a minimal PATH.
    let mut path_entries: Vec<PathBuf> = Vec::new();
    if let Some(node) = find_node() {
        if let Some(dir) = node.parent() {
            let dir_buf = dir.to_path_buf();
            if !path_entries.contains(&dir_buf) {
                path_entries.push(dir_buf);
            }
        }
    }
        if let Some(dsh) = find_dsh() {
            if let Some(dir) = dsh.parent() {
                let dir_buf = dir.to_path_buf();
                if !path_entries.contains(&dir_buf) {
                    path_entries.push(dir_buf);
                }
            }
        }
    // Also include common locations
    for dir in ["/usr/local/bin", "/usr/bin", "/bin"] {
        path_entries.push(PathBuf::from(dir));
    }
    // Prepend to existing PATH
    let existing_path = std::env::var("PATH").unwrap_or_default();
    for p in existing_path.split(':') {
        let pb = PathBuf::from(p);
        if !path_entries.contains(&pb) {
            path_entries.push(pb);
        }
    }
    let new_path = path_entries
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(":");

    let mut child = Command::new(executable)
        .args(["web", "--no-open", "--port", "0"])
        .env("PATH", &new_path)
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
