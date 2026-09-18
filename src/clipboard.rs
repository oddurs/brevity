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

/// `pbpaste` and `pbcopy` choose their encoding from the locale. A hotkey
/// daemon runs under launchd, which sets no locale at all, so they fall back to
/// Mac OS Roman: reading turns `é` into the invalid byte 0x8e and drops emoji,
/// and writing mangles every non-ASCII character in the summary. Forcing a
/// UTF-8 locale on the child is the whole fix, and it costs nothing elsewhere.
fn utf8_locale() -> &'static str {
    if cfg!(target_os = "macos") {
        "en_US.UTF-8"
    } else {
        "C.UTF-8"
    }
}

fn command(bin: &str) -> Command {
    let mut c = Command::new(bin);
    c.env("LC_ALL", utf8_locale()).env("LC_CTYPE", utf8_locale());
    c
}

/// Clipboards carry a BOM from Windows editors and CRLF from all sorts of
/// places; neither helps the model, and both survive into the summary.
fn tidy_text(s: String) -> String {
    let s = s.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(s);
    if !s.contains('\r') {
        return s;
    }
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// Prefer a faithful decode, but a mis-encoded clipboard should degrade rather
/// than refuse to work - the original is still recoverable with --restore.
fn decode(bytes: Vec<u8>) -> (String, bool) {
    match String::from_utf8(bytes) {
        Ok(s) => (tidy_text(s), false),
        Err(e) => (tidy_text(String::from_utf8_lossy(e.as_bytes()).into_owned()), true),
    }
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
    let out = command(t.bin)
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

    let (text, lossy) = decode(out.stdout);
    if lossy {
        eprintln!("brevity: the clipboard was not valid UTF-8; unreadable characters were dropped");
    }
    Ok(text)
}

pub fn write(text: &str) -> Result<(), String> {
    let t = copy_tool()?;
    let mut child = command(t.bin)
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

#[cfg(test)]
mod tests {
    use super::{decode, tidy_text, utf8_locale};

    #[test]
    fn ordinary_text_is_returned_untouched() {
        let (text, lossy) = decode("café — “smart” 🎯".as_bytes().to_vec());
        assert_eq!(text, "café — “smart” 🎯");
        assert!(!lossy);
    }

    #[test]
    fn mac_roman_bytes_degrade_instead_of_failing() {
        // What pbpaste emits for "café" with no locale set - the bug report.
        let (text, lossy) = decode(vec![b'c', b'a', b'f', 0x8e]);
        assert!(lossy, "should have reported a lossy decode");
        assert!(text.starts_with("caf"), "readable part must survive: {text:?}");
    }

    #[test]
    fn a_byte_order_mark_is_stripped() {
        assert_eq!(tidy_text("\u{feff}hello".to_string()), "hello");
        assert_eq!(tidy_text("mid\u{feff}dle".to_string()), "mid\u{feff}dle", "only a leading BOM");
    }

    #[test]
    fn line_endings_are_normalized() {
        assert_eq!(tidy_text("a\r\nb\rc\nd".to_string()), "a\nb\nc\nd");
    }

    #[test]
    fn text_without_carriage_returns_is_left_alone() {
        let s = "already\nfine\n".to_string();
        assert_eq!(tidy_text(s.clone()), s);
    }

    #[test]
    fn the_forced_locale_is_one_the_platform_actually_has() {
        let l = utf8_locale();
        assert!(l.to_lowercase().contains("utf-8"), "{l} is not a UTF-8 locale");
        if cfg!(target_os = "macos") {
            assert_eq!(l, "en_US.UTF-8", "macOS does not reliably ship C.UTF-8");
        }
    }
}
