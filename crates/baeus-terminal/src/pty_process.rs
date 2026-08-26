// PtyProcess — wraps portable-pty to spawn a real shell process.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

/// A running PTY process backed by `portable-pty`.
pub struct PtyProcess {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    reader: Arc<Mutex<Box<dyn Read + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl PtyProcess {
    /// Spawn a new shell process with the given terminal size.
    ///
    /// Detects the user's shell from `$SHELL` (falls back to `/bin/sh`),
    /// sets `TERM=xterm-256color`, and spawns the child.
    pub fn spawn_shell(rows: u16, cols: u16) -> anyhow::Result<Self> {
        Self::spawn_shell_with_env(rows, cols, &[])
    }

    /// Spawn a shell with additional environment variables.
    ///
    /// Each entry is a `(key, value)` pair set in the child process environment.
    pub fn spawn_shell_with_env(
        rows: u16,
        cols: u16,
        env: &[(&str, &str)],
    ) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();

        let pair = pty_system.openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;

        #[cfg(not(target_os = "windows"))]
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

        #[cfg(target_os = "windows")]
        let shell =
            std::env::var("COMSPEC").unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".to_string());

        // Validate the shell path: must be an absolute path pointing to a known shell.
        let shell = validate_shell_path(&shell);

        let mut cmd = CommandBuilder::new(&shell);
        cmd.env("TERM", "xterm-256color");
        for (k, v) in env {
            cmd.env(k, v);
        }

        let child = pair.slave.spawn_command(cmd)?;

        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        Ok(Self {
            master: pair.master,
            child,
            reader: Arc::new(Mutex::new(reader)),
            writer: Arc::new(Mutex::new(writer)),
        })
    }

    /// Write input bytes to the PTY master (stdin of the shell).
    pub fn write_input(&self, data: &[u8]) -> anyhow::Result<()> {
        let mut writer = self.writer.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    /// Read available output from the PTY master (stdout of the shell).
    /// Returns the number of bytes read. Non-blocking if no data is available
    /// may block briefly depending on the OS PTY implementation.
    pub fn read_output(&self, buf: &mut [u8]) -> anyhow::Result<usize> {
        let mut reader = self.reader.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        let n = reader.read(buf)?;
        Ok(n)
    }

    /// Resize the PTY to new dimensions.
    pub fn resize(&self, rows: u16, cols: u16) -> anyhow::Result<()> {
        self.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
        Ok(())
    }

    /// Get a clone of the reader handle for use in a background thread.
    pub fn reader_handle(&self) -> Arc<Mutex<Box<dyn Read + Send>>> {
        Arc::clone(&self.reader)
    }

    /// Get a clone of the writer handle for use in a background thread.
    pub fn writer_handle(&self) -> Arc<Mutex<Box<dyn Write + Send>>> {
        Arc::clone(&self.writer)
    }

    /// Terminate the child process.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }
}

impl Drop for PtyProcess {
    fn drop(&mut self) {
        // Ensure child process is cleaned up to prevent zombies and fd leaks.
        self.kill();
        // Reap the child so it doesn't remain as a zombie.
        let _ = self.child.wait();
    }
}

/// Allowlist of known safe shell paths (Unix).
#[cfg(not(target_os = "windows"))]
const ALLOWED_SHELLS: &[&str] = &[
    "/bin/bash",
    "/bin/sh",
    "/bin/zsh",
    "/bin/fish",
    "/bin/dash",
    "/bin/ksh",
    "/bin/csh",
    "/bin/tcsh",
    "/usr/bin/bash",
    "/usr/bin/zsh",
    "/usr/bin/fish",
    "/usr/local/bin/bash",
    "/usr/local/bin/zsh",
    "/usr/local/bin/fish",
    "/opt/homebrew/bin/bash",
    "/opt/homebrew/bin/zsh",
    "/opt/homebrew/bin/fish",
    "/run/current-system/sw/bin/bash",
    "/run/current-system/sw/bin/zsh",
    "/run/current-system/sw/bin/fish",
];

/// Allowlist of known safe shell paths (Windows).
#[cfg(target_os = "windows")]
const ALLOWED_SHELLS: &[&str] = &[
    r"C:\Windows\System32\cmd.exe",
    r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
    r"C:\Program Files\PowerShell\7\pwsh.exe",
];

/// Validate a shell path against the allowlist. Returns a safe fallback if invalid.
#[cfg(not(target_os = "windows"))]
fn validate_shell_path(shell: &str) -> String {
    let path = std::path::Path::new(shell);

    // Must be absolute
    if !path.is_absolute() {
        tracing::warn!("Shell path '{}' is not absolute, falling back to /bin/sh", shell);
        return "/bin/sh".to_string();
    }

    // Must not contain path traversal
    if shell.contains("..") {
        tracing::warn!("Shell path '{}' contains '..', falling back to /bin/sh", shell);
        return "/bin/sh".to_string();
    }

    // Check against allowlist
    if ALLOWED_SHELLS.contains(&shell) {
        return shell.to_string();
    }

    // Also accept any path listed in /etc/shells (if readable)
    if let Ok(contents) = std::fs::read_to_string("/etc/shells") {
        for line in contents.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') && line == shell {
                return shell.to_string();
            }
        }
    }

    tracing::warn!(
        "Shell '{}' is not in the allowlist or /etc/shells, falling back to /bin/sh",
        shell
    );
    "/bin/sh".to_string()
}

/// Validate a shell path against the allowlist. Returns a safe fallback if invalid.
#[cfg(target_os = "windows")]
fn validate_shell_path(shell: &str) -> String {
    let path = std::path::Path::new(shell);

    // Must be absolute
    if !path.is_absolute() {
        tracing::warn!("Shell path '{}' is not absolute, falling back to cmd.exe", shell);
        return r"C:\Windows\System32\cmd.exe".to_string();
    }

    // Must not contain path traversal
    if shell.contains("..") {
        tracing::warn!("Shell path '{}' contains '..', falling back to cmd.exe", shell);
        return r"C:\Windows\System32\cmd.exe".to_string();
    }

    // Must end with .exe
    if !shell.to_lowercase().ends_with(".exe") {
        tracing::warn!("Shell path '{}' does not end with .exe, falling back to cmd.exe", shell);
        return r"C:\Windows\System32\cmd.exe".to_string();
    }

    // Case-insensitive comparison against allowlist
    let shell_lower = shell.to_lowercase();
    for allowed in ALLOWED_SHELLS {
        if allowed.to_lowercase() == shell_lower {
            return shell.to_string();
        }
    }

    // Fall back to COMSPEC if set, otherwise cmd.exe
    let fallback =
        std::env::var("COMSPEC").unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".to_string());
    tracing::warn!("Shell '{}' is not in the allowlist, falling back to '{}'", shell, fallback);
    fallback
}
