//! Clipboard access by shelling out to the platform tools.
//!
//! Deliberately not an in-process clipboard library: on X11 the clipboard is owned
//! by a live process, so a short-lived binary that sets it and exits loses the
//! contents immediately. `xclip`/`wl-copy` fork a helper that keeps serving it.

use std::io::Write;
use std::process::{Command, Stdio};

struct Tool {
    bin: &'static str,
    args: &'static [&'static str],
}

fn have(bin: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {bin} >/dev/null 2>&1"))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").map(|v| !v.is_empty()).unwrap_or(false)
}

fn paste_tool() -> Result<Tool, String> {
    if cfg!(target_os = "macos") {
        return Ok(Tool { bin: "pbpaste", args: &[] });
    }
    if cfg!(target_os = "windows") {
        return Ok(Tool {
            bin: "powershell",
            args: &["-NoProfile", "-Command", "Get-Clipboard -Raw"],
        });
    }
    if wayland() && have("wl-paste") {
        return Ok(Tool { bin: "wl-paste", args: &["--no-newline"] });
    }
    if have("xclip") {
        return Ok(Tool { bin: "xclip", args: &["-selection", "clipboard", "-o"] });
    }
    if have("xsel") {
        return Ok(Tool { bin: "xsel", args: &["--clipboard", "--output"] });
    }
    if have("wl-paste") {
        return Ok(Tool { bin: "wl-paste", args: &["--no-newline"] });
    }
    Err(missing_tools())
}

fn copy_tool() -> Result<Tool, String> {
    if cfg!(target_os = "macos") {
        return Ok(Tool { bin: "pbcopy", args: &[] });
    }
    if cfg!(target_os = "windows") {
        return Ok(Tool { bin: "clip", args: &[] });
    }
    if wayland() && have("wl-copy") {
        return Ok(Tool { bin: "wl-copy", args: &[] });
    }
    if have("xclip") {
        return Ok(Tool { bin: "xclip", args: &["-selection", "clipboard", "-i"] });
    }
    if have("xsel") {
        return Ok(Tool { bin: "xsel", args: &["--clipboard", "--input"] });
    }
    if have("wl-copy") {
        return Ok(Tool { bin: "wl-copy", args: &[] });
    }
    Err(missing_tools())
}

fn missing_tools() -> String {
    "no clipboard tool found. Install one:\n  \
     Wayland: wl-clipboard  (apt install wl-clipboard / dnf install wl-clipboard)\n  \
     X11:     xclip         (apt install xclip / dnf install xclip)"
        .to_string()
}

pub fn read() -> Result<String, String> {
    let t = paste_tool()?;
    let out = Command::new(t.bin)
        .args(t.args)
        .output()
        .map_err(|e| format!("could not run {}: {e}", t.bin))?;
    if !out.status.success() {
        // wl-paste exits non-zero when the clipboard is empty or holds no text.
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if err.is_empty() {
            return Ok(String::new());
        }
        return Err(format!("{} failed: {err}", t.bin));
    }
    String::from_utf8(out.stdout).map_err(|_| "clipboard does not hold UTF-8 text".to_string())
}

pub fn write(text: &str) -> Result<(), String> {
    let t = copy_tool()?;
    let mut child = Command::new(t.bin)
        .args(t.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("could not run {}: {e}", t.bin))?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| format!("{} took no input", t.bin))?
        .write_all(text.as_bytes())
        .map_err(|e| format!("writing to {} failed: {e}", t.bin))?;
    let status = child.wait().map_err(|e| format!("{} failed: {e}", t.bin))?;
    if !status.success() {
        return Err(format!("{} exited with {status}", t.bin));
    }
    Ok(())
}
