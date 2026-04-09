// terminal.rs — PTY-based terminal backend for Tauri
// Uses portable-pty to allocate real pseudo-terminals, supporting full ANSI
// colour, interactive programs (vim, htop), Tab completion, etc.

use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize, Child};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
// ── State ─────────────────────────────────────────────────────────────────────

struct PtyHandle {
    pair: PtyPair,
    writer: Box<dyn Write + Send>,
    // Keep child alive so the process is not orphaned and can be killed on demand.
    child: Box<dyn Child + Send + Sync>,
}

pub struct TerminalState {
    ptys: Mutex<HashMap<String, PtyHandle>>,
}

impl TerminalState {
    pub fn new() -> Self {
        TerminalState {
            ptys: Mutex::new(HashMap::new()),
        }
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Spawn a new PTY shell and start streaming its output as Tauri events.
/// Returns the terminal id so the frontend can route I/O correctly.
#[tauri::command]
pub async fn create_terminal(
    app: AppHandle,
    id: String,
    rows: u16,
    cols: u16,
    cwd: Option<String>,
) -> Result<String, String> {
    let state: State<Arc<TerminalState>> = app.state();

    // Detect default shell from environment; fall back to /bin/zsh → /bin/sh
    let shell = std::env::var("SHELL").unwrap_or_else(|_| {
        if std::path::Path::new("/bin/zsh").exists() {
            "/bin/zsh".to_string()
        } else {
            "/bin/sh".to_string()
        }
    });

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("openpty failed: {e}"))?;

    let mut cmd = CommandBuilder::new(&shell);
    cmd.arg("-l"); // login shell so ~/.zshrc / ~/.bash_profile are sourced
    if let Some(dir) = cwd {
        cmd.cwd(dir);
    }

    // Spawn the shell inside the PTY slave
    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("spawn failed: {e}"))?;

    // Clone master for reading (output from shell)
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("clone_reader failed: {e}"))?;

    // Writer (input to shell)
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("take_writer failed: {e}"))?;

    let terminal_id = id.clone();
    let app_handle = app.clone();

    // Background thread: read output bytes and emit as Tauri events
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    // Shell exited; notify frontend
                    let _ = app_handle.emit(&format!("terminal-exit:{terminal_id}"), ());
                    break;
                }
                Ok(n) => {
                    // Send raw bytes as base64 string so Tauri JSON can carry them
                    let data = base64_encode(&buf[..n]);
                    let _ = app_handle.emit(&format!("terminal-data:{terminal_id}"), data);
                }
                Err(_) => {
                    let _ = app_handle.emit(&format!("terminal-exit:{terminal_id}"), ());
                    break;
                }
            }
        }
    });

    // Store handle (child kept alive here)
    let handle = PtyHandle { pair, writer, child };
    state
        .ptys
        .lock()
        .map_err(|e| e.to_string())?
        .insert(id.clone(), handle);

    Ok(id)
}

/// Write user keystrokes (base64-encoded bytes) into the PTY master.
#[tauri::command]
pub fn write_terminal(
    app: AppHandle,
    id: String,
    data: String, // base64-encoded bytes
) -> Result<(), String> {
    let state: State<Arc<TerminalState>> = app.state();
    let mut ptys = state.ptys.lock().map_err(|e| e.to_string())?;
    let handle = ptys.get_mut(&id).ok_or_else(|| format!("no PTY for id {id}"))?;
    let bytes = base64_decode(&data).map_err(|e| format!("base64 decode: {e}"))?;
    handle.writer.write_all(&bytes).map_err(|e| e.to_string())?;
    handle.writer.flush().map_err(|e| e.to_string())?;
    Ok(())
}

/// Resize a PTY (called when the panel width/height changes).
#[tauri::command]
pub fn resize_terminal(
    app: AppHandle,
    id: String,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let state: State<Arc<TerminalState>> = app.state();
    let ptys = state.ptys.lock().map_err(|e| e.to_string())?;
    let handle = ptys.get(&id).ok_or_else(|| format!("no PTY for id {id}"))?;
    handle
        .pair
        .master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Kill a PTY and remove it from the map.
#[tauri::command]
pub fn kill_terminal(app: AppHandle, id: String) -> Result<(), String> {
    let state: State<Arc<TerminalState>> = app.state();
    let mut ptys = state.ptys.lock().map_err(|e| e.to_string())?;
    if let Some(mut handle) = ptys.remove(&id) {
        // Best-effort kill; ignore errors (process may have already exited)
        let _ = handle.child.kill();
    }
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b = match chunk.len() {
            1 => [chunk[0], 0, 0],
            2 => [chunk[0], chunk[1], 0],
            _ => [chunk[0], chunk[1], chunk[2]],
        };
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Result<u32, String> {
        match c {
            b'A'..=b'Z' => Ok((c - b'A') as u32),
            b'a'..=b'z' => Ok((c - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((c - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0),
            _ => Err(format!("invalid base64 char: {c}")),
        }
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        if chunk.len() < 4 {
            break;
        }
        let n = (val(chunk[0])? << 18)
            | (val(chunk[1])? << 12)
            | (val(chunk[2])? << 6)
            | val(chunk[3])?;
        out.push(((n >> 16) & 0xff) as u8);
        if chunk[2] != b'=' {
            out.push(((n >> 8) & 0xff) as u8);
        }
        if chunk[3] != b'=' {
            out.push((n & 0xff) as u8);
        }
    }
    Ok(out)
}
