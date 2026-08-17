use std::path::Path;
use std::process::{Command, Stdio};

/// Spawn a shell command for a Wayland client in a detached, non-interactive process context.
pub fn spawn_wayland_command(command: &str, wayland_display: &str) -> std::io::Result<()> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .env("WAYLAND_DISPLAY", wayland_display)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    if let Some(shell) = login_shell_from_passwd() {
        cmd.env("SHELL", shell);
    } else if !shell_env_is_usable() {
        // If SHELL points to an invalid binary, terminals can open without a usable prompt.
        // Let clients fall back to the account's login shell from /etc/passwd instead.
        cmd.env_remove("SHELL");
    }

    cmd.spawn().map(|_| ())
}

fn shell_env_is_usable() -> bool {
    let Some(shell) = std::env::var_os("SHELL") else {
        return true;
    };
    let path = Path::new(&shell);
    shell_path_is_executable(path) && !looks_like_noninteractive_shell(path)
}

fn login_shell_from_passwd() -> Option<String> {
    let user = std::env::var("USER").ok()?;
    let content = std::fs::read_to_string("/etc/passwd").ok()?;
    let line = content
        .lines()
        .find(|line| line.starts_with(&format!("{user}:")))?;
    let shell = line.split(':').nth(6)?.trim();
    let shell_path = Path::new(shell);
    if shell_path_is_executable(shell_path) && !looks_like_noninteractive_shell(shell_path) {
        Some(shell.to_string())
    } else {
        None
    }
}

fn looks_like_noninteractive_shell(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("nologin") | Some("false")
    )
}

#[cfg(unix)]
fn shell_path_is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    metadata.is_file() && (metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn shell_path_is_executable(path: &Path) -> bool {
    path.is_file()
}
