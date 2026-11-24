use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

const URL_PREFIX: &str = "dsh web: ";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

pub fn find_dsh() -> Option<PathBuf> {
    if let Ok(path) = which::which("dsh") {
        return Some(path);
    }
    if let Some(home) = std::env::var_os("HOME") {
        let candidate = PathBuf::from(home).join(".local/bin/dsh");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    // npm-global installs (fnm/nvm) put dsh next to node.
    let node = find_node()?;
    let sibling = node.parent()?.join("dsh");
    sibling.is_file().then_some(sibling)
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
    spawn_with_timeout(STARTUP_TIMEOUT)
}

fn spawn_with_timeout(timeout: Duration) -> std::io::Result<(Child, String)> {
    let executable = find_dsh().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "dsh was not found on PATH, at ~/.local/bin/dsh, or next to node",
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
    if let Some(dir) = executable.parent() {
        let dir_buf = dir.to_path_buf();
        if !path_entries.contains(&dir_buf) {
            path_entries.push(dir_buf);
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

    let mut child = Command::new(&executable)
        .args(["web", "--no-open", "--port", "0"])
        .env("PATH", &new_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("dsh stdout was not piped"))?;

    // Read stdout on a helper thread so a silent or slow dsh cannot block us;
    // the thread keeps draining stdout after the URL is found.
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut sent = false;
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if !sent {
                if let Some(url) = line.trim().strip_prefix(URL_PREFIX) {
                    let _ = tx.send(Ok(url.trim().to_string()));
                    sent = true;
                }
            }
        }
        if !sent {
            let _ = tx.send(Err(std::io::Error::other(
                "dsh web exited before printing its URL",
            )));
        }
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(url)) => Ok((child, url)),
        Ok(Err(error)) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(error)
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "dsh web did not print its URL within {}s",
                    timeout.as_secs()
                ),
            ))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(std::io::Error::other("dsh web stdout reader stopped"))
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Mutex;

    // Both tests mutate PATH, so serialize them.
    static PATH_LOCK: Mutex<()> = Mutex::new(());

    fn install_fake_dsh(script: &str) -> (PathBuf, Option<std::ffi::OsString>) {
        let dir = std::env::temp_dir().join(format!(
            "dsh-desktop-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("dsh");
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let old = std::env::var_os("PATH");
        let prefix = dir.display().to_string();
        let path = match &old {
            Some(existing) => format!("{prefix}:{}", existing.to_string_lossy()),
            None => prefix,
        };
        std::env::set_var("PATH", path);
        (dir, old)
    }

    fn restore_path(old: Option<std::ffi::OsString>) {
        match old {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }
    }

    #[test]
    fn spawn_reads_printed_url() {
        let _lock = PATH_LOCK.lock().unwrap();
        let (dir, old_path) = install_fake_dsh(
            "#!/bin/sh\necho 'dsh web: http://127.0.0.1:4242/?token=abc'\nexec sleep 30\n",
        );
        let result = spawn_with_timeout(Duration::from_secs(5));
        restore_path(old_path);

        let (mut child, url) = result.expect("dsh should start and print a URL");
        assert_eq!(url, "http://127.0.0.1:4242/?token=abc");
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn spawn_times_out_when_no_url_is_printed() {
        let _lock = PATH_LOCK.lock().unwrap();
        let (dir, old_path) = install_fake_dsh("#!/bin/sh\nexec sleep 30\n");
        let result = spawn_with_timeout(Duration::from_millis(200));
        restore_path(old_path);

        let error = result.expect_err("a silent dsh must time out");
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        let _ = std::fs::remove_dir_all(dir);
    }
}
