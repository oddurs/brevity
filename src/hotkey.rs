//! `brevity --install-hotkey`: bind a key without a trip through a settings UI.
//!
//! Every platform here is driven by a text config file or a CLI, so the whole
//! thing is scriptable. macOS is the awkward one: any third-party hotkey daemon
//! needs an Accessibility grant that only the user can give, so the best this
//! can do is install skhd, write the binding, and open the right settings pane.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::home;

const BEGIN: &str = "# >>> brevity >>>";
const END: &str = "# <<< brevity <<<";

/// A chord, held platform-independently so each target can spell it its own way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeySpec {
    ctrl: bool,
    alt: bool,
    shift: bool,
    /// Command on macOS, Super elsewhere.
    cmd: bool,
    key: char,
}

impl KeySpec {
    pub fn parse(s: &str) -> Result<KeySpec, String> {
        let mut spec = KeySpec { ctrl: false, alt: false, shift: false, cmd: false, key: '\0' };
        for part in s.split(['+', '-', ' ']).filter(|p| !p.is_empty()) {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => spec.ctrl = true,
                "alt" | "opt" | "option" => spec.alt = true,
                "shift" => spec.shift = true,
                "cmd" | "command" | "super" | "win" | "meta" => spec.cmd = true,
                other => {
                    let mut chars = other.chars();
                    match (chars.next(), chars.next()) {
                        (Some(c), None) if c.is_ascii_alphanumeric() => spec.key = c,
                        _ => return Err(format!(
                            "'{other}' is not a modifier or a single key. Try something like ctrl+alt+cmd+b"
                        )),
                    }
                }
            }
        }
        if spec.key == '\0' {
            return Err(format!("'{s}' names no key, only modifiers"));
        }
        if !(spec.ctrl || spec.alt || spec.cmd) {
            return Err(format!(
                "'{s}' needs at least one of ctrl, alt or cmd, or it would fire while you type"
            ));
        }
        Ok(spec)
    }

    pub fn default_for_platform() -> KeySpec {
        // Cmd is a crowded modifier on macOS but ⌃⌥⌘ is reliably free; on Linux
        // Super is usually claimed by the desktop, so leave it out.
        KeySpec { ctrl: true, alt: true, shift: false, cmd: cfg!(target_os = "macos"), key: 'b' }
    }

    fn plus_shift(&self) -> KeySpec {
        KeySpec { shift: true, ..*self }
    }

    /// What to print back at the user.
    pub fn human(&self) -> String {
        let mut s = String::new();
        if cfg!(target_os = "macos") {
            if self.ctrl {
                s.push('⌃');
            }
            if self.alt {
                s.push('⌥');
            }
            if self.shift {
                s.push('⇧');
            }
            if self.cmd {
                s.push('⌘');
            }
        } else {
            for (on, name) in
                [(self.ctrl, "Ctrl"), (self.alt, "Alt"), (self.shift, "Shift"), (self.cmd, "Super")]
            {
                if on {
                    let _ = write!(s, "{name}+");
                }
            }
        }
        s.push(self.key.to_ascii_uppercase());
        s
    }

    fn skhd(&self) -> String {
        let mut mods: Vec<&str> = Vec::new();
        for (on, name) in
            [(self.ctrl, "ctrl"), (self.alt, "alt"), (self.shift, "shift"), (self.cmd, "cmd")]
        {
            if on {
                mods.push(name);
            }
        }
        format!("{} - {}", mods.join(" + "), self.key)
    }

    fn gnome(&self) -> String {
        let mut s = String::new();
        for (on, name) in [
            (self.ctrl, "<Ctrl>"),
            (self.alt, "<Alt>"),
            (self.shift, "<Shift>"),
            (self.cmd, "<Super>"),
        ] {
            if on {
                s.push_str(name);
            }
        }
        s.push(self.key);
        s
    }

    /// sway and i3 share a syntax; Mod1 is Alt, Mod4 is Super.
    fn sway(&self) -> String {
        let mut mods: Vec<&str> = Vec::new();
        for (on, name) in
            [(self.ctrl, "Ctrl"), (self.alt, "Mod1"), (self.shift, "Shift"), (self.cmd, "Mod4")]
        {
            if on {
                mods.push(name);
            }
        }
        format!("{}+{}", mods.join("+"), self.key)
    }

    fn hypr(&self) -> (String, char) {
        let mut mods: Vec<&str> = Vec::new();
        for (on, name) in
            [(self.ctrl, "CTRL"), (self.alt, "ALT"), (self.shift, "SHIFT"), (self.cmd, "SUPER")]
        {
            if on {
                mods.push(name);
            }
        }
        (mods.join(" "), self.key.to_ascii_uppercase())
    }
}

/// Replace the block between our markers, or append one. Anything the user
/// wrote outside the markers is left exactly as it was.
fn merge_block(existing: &str, body: &str) -> String {
    let block = format!("{BEGIN}\n{}\n{END}", body.trim_end());

    if let (Some(start), Some(end)) = (existing.find(BEGIN), existing.find(END)) {
        if start < end {
            let mut out = String::with_capacity(existing.len() + block.len());
            out.push_str(&existing[..start]);
            out.push_str(&block);
            out.push_str(&existing[end + END.len()..]);
            return out;
        }
    }

    let mut out = existing.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&block);
    out.push('\n');
    out
}

fn strip_block(existing: &str) -> String {
    let (Some(start), Some(end)) = (existing.find(BEGIN), existing.find(END)) else {
        return existing.to_string();
    };
    if start > end {
        return existing.to_string();
    }

    let head = existing[..start].trim_end();
    let tail = existing[end + END.len()..].trim_start_matches('\n').trim_end();

    match (head.is_empty(), tail.is_empty()) {
        (true, true) => String::new(),
        (true, false) => format!("{tail}\n"),
        (false, true) => format!("{head}\n"),
        (false, false) => format!("{head}\n\n{tail}\n"),
    }
}

fn write_merged(path: &Path, body: &str) -> Result<bool, String> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let merged = merge_block(&existing, body);
    if merged == existing {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    }
    std::fs::write(path, merged).map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(true)
}

/// The absolute path to this binary - hotkey daemons never inherit your PATH.
fn binary() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "brevity".to_string())
}

fn have(bin: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {bin} >/dev/null 2>&1"))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Like `run_visible`, but swallows a failure's noise - used where a failure is
/// an expected branch rather than a problem.
fn run_quiet(bin: &str, args: &[&str]) -> bool {
    Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_visible(bin: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .status()
        .map_err(|e| format!("could not run {bin}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{bin} {} failed", args.join(" ")))
    }
}

pub fn install(spec: Option<&str>) -> Result<(), String> {
    let key = match spec {
        Some(s) => KeySpec::parse(s)?,
        None => KeySpec::default_for_platform(),
    };

    if cfg!(target_os = "macos") {
        install_macos(key)
    } else {
        install_linux(key)
    }
}

pub fn uninstall() -> Result<(), String> {
    let mut removed = Vec::new();
    for path in candidate_configs() {
        let Ok(existing) = std::fs::read_to_string(&path) else { continue };
        if !existing.contains(BEGIN) {
            continue;
        }
        std::fs::write(&path, strip_block(&existing))
            .map_err(|e| format!("could not write {}: {e}", path.display()))?;
        removed.push(path);
    }

    if cfg!(not(target_os = "macos")) && have("gsettings") {
        gnome_unbind();
    }

    if removed.is_empty() {
        println!("no brevity hotkey blocks found");
    } else {
        for p in &removed {
            println!("removed the brevity block from {}", p.display());
        }
        if cfg!(target_os = "macos") && have("skhd") {
            run_quiet("skhd", &["--restart-service"]);
        }
        println!("skhd/your window manager may need a reload to forget the binding");
    }
    Ok(())
}

fn candidate_configs() -> Vec<PathBuf> {
    let h = home();
    vec![
        h.join(".config/skhd/skhdrc"),
        h.join(".config/sway/config"),
        h.join(".config/i3/config"),
        h.join(".config/hypr/hyprland.conf"),
    ]
}

// ------------------------------------------------------------------- macOS

fn install_macos(key: KeySpec) -> Result<(), String> {
    if !have("brew") {
        return Err("Homebrew is needed to install skhd: https://brew.sh\n       \
                    Or bind the key yourself - see hotkeys/ in the repository."
            .to_string());
    }

    if !have("skhd") {
        println!("installing skhd (the hotkey daemon)...");
        run_visible("brew", &["tap", "koekeishiya/formulae"])?;
        run_visible("brew", &["install", "koekeishiya/formulae/skhd"])?;
    }

    let bin = binary();
    let body = format!(
        "# Written by `brevity --install-hotkey`. Edit freely; re-running rewrites\n\
         # only the lines between these markers.\n\
         {} : {bin}\n\
         {} : {bin} --restore",
        key.skhd(),
        key.plus_shift().skhd(),
    );

    let path = home().join(".config/skhd/skhdrc");
    let changed = write_merged(&path, &body)?;
    println!("{} {}", if changed { "wrote" } else { "already current:" }, path.display());

    // Restart picks up the new config; on a first install there is no service
    // to restart yet, and skhd says so loudly, so try it quietly and fall back.
    if !run_quiet("skhd", &["--restart-service"]) {
        run_visible("skhd", &["--start-service"])?;
    }

    println!();
    println!("  {}   summarize the clipboard", key.human());
    println!("  {}   put the original back", key.plus_shift().human());
    println!();
    println!("One thing left, and only you can do it: macOS requires that skhd be");
    println!("granted Accessibility before it can see key presses.");
    println!();
    println!("  System Settings > Privacy & Security > Accessibility > turn on skhd");
    println!();
    println!("Opening that pane now. After granting it, run:  skhd --restart-service");

    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    Ok(())
}

// ------------------------------------------------------------------- Linux

fn install_linux(key: KeySpec) -> Result<(), String> {
    let bin = binary();
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();

    // A running compositor is the strongest signal; fall back to the desktop name.
    if std::env::var("SWAYSOCK").is_ok() || desktop.contains("sway") {
        let body = format!(
            "bindsym {} exec {bin}\nbindsym {} exec {bin} --restore",
            key.sway(),
            key.plus_shift().sway()
        );
        let path = home().join(".config/sway/config");
        write_merged(&path, &body)?;
        println!("wrote {}", path.display());
        let _ = run_visible("swaymsg", &["reload"]);
        return done(key);
    }

    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() || desktop.contains("hyprland") {
        let (mods, k) = key.hypr();
        let (smods, sk) = key.plus_shift().hypr();
        let body =
            format!("bind = {mods}, {k}, exec, {bin}\nbind = {smods}, {sk}, exec, {bin} --restore");
        let path = home().join(".config/hypr/hyprland.conf");
        write_merged(&path, &body)?;
        println!("wrote {}", path.display());
        let _ = run_visible("hyprctl", &["reload"]);
        return done(key);
    }

    if desktop.contains("i3") {
        let body = format!(
            "bindsym {} exec --no-startup-id {bin}\nbindsym {} exec --no-startup-id {bin} --restore",
            key.sway(),
            key.plus_shift().sway()
        );
        let path = home().join(".config/i3/config");
        write_merged(&path, &body)?;
        println!("wrote {}", path.display());
        let _ = run_visible("i3-msg", &["reload"]);
        return done(key);
    }

    if desktop.contains("gnome") && have("gsettings") {
        gnome_bind(key, &bin)?;
        return done(key);
    }

    Err(format!(
        "no desktop this can configure automatically (XDG_CURRENT_DESKTOP={}).\n       \
         Bind {} to `{bin}` by hand - hotkeys/ in the repository has the syntax for\n       \
         KDE, GNOME, sway, Hyprland and i3.",
        if desktop.is_empty() { "unset" } else { &desktop },
        key.human()
    ))
}

fn done(key: KeySpec) -> Result<(), String> {
    println!();
    println!("  {}   summarize the clipboard", key.human());
    println!("  {}   put the original back", key.plus_shift().human());
    Ok(())
}

const GNOME_SLOT: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/brevity/";
const GNOME_SLOT_RESTORE: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/brevity-restore/";

fn gnome_bind(key: KeySpec, bin: &str) -> Result<(), String> {
    let list = gsettings_get("org.gnome.settings-daemon.plugins.media-keys", "custom-keybindings")
        .unwrap_or_else(|| "@as []".to_string());

    let mut slots: Vec<String> = list
        .trim()
        .trim_start_matches("@as ")
        .trim_matches(['[', ']'].as_ref())
        .split(',')
        .map(|s| s.trim().trim_matches('\'').to_string())
        .filter(|s| !s.is_empty())
        .collect();

    for slot in [GNOME_SLOT, GNOME_SLOT_RESTORE] {
        if !slots.iter().any(|s| s == slot) {
            slots.push(slot.to_string());
        }
    }
    let joined = slots.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(", ");
    gsettings_set_list(
        "org.gnome.settings-daemon.plugins.media-keys",
        "custom-keybindings",
        &format!("[{joined}]"),
    )?;

    for (slot, name, cmd, k) in [
        (GNOME_SLOT, "Brevity", bin.to_string(), key),
        (GNOME_SLOT_RESTORE, "Brevity restore", format!("{bin} --restore"), key.plus_shift()),
    ] {
        let schema =
            format!("org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:{slot}");
        gsettings_set(&schema, "name", name)?;
        gsettings_set(&schema, "command", &cmd)?;
        gsettings_set(&schema, "binding", &k.gnome())?;
    }
    println!("registered two GNOME custom shortcuts");
    Ok(())
}

fn gnome_unbind() {
    for slot in [GNOME_SLOT, GNOME_SLOT_RESTORE] {
        let schema =
            format!("org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:{slot}");
        let _ = Command::new("gsettings")
            .args(["reset-recursively", &schema])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn gsettings_get(schema: &str, k: &str) -> Option<String> {
    let out = Command::new("gsettings").args(["get", schema, k]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn gsettings_set(schema: &str, k: &str, v: &str) -> Result<(), String> {
    run_visible("gsettings", &["set", schema, k, v])
}

fn gsettings_set_list(schema: &str, k: &str, v: &str) -> Result<(), String> {
    run_visible("gsettings", &["set", schema, k, v])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(s: &str) -> KeySpec {
        KeySpec::parse(s).unwrap()
    }

    #[test]
    fn modifiers_parse_by_any_of_their_names() {
        assert_eq!(spec("ctrl+alt+cmd+b"), spec("control-option-command-b"));
        assert_eq!(spec("super+k"), spec("meta k"));
    }

    #[test]
    fn a_chord_needs_a_key_and_a_modifier() {
        assert!(KeySpec::parse("ctrl+alt").unwrap_err().contains("no key"));
        assert!(KeySpec::parse("b").unwrap_err().contains("ctrl"));
        assert!(
            KeySpec::parse("shift+b").unwrap_err().contains("ctrl"),
            "shift alone is not enough"
        );
        assert!(KeySpec::parse("ctrl+enter").unwrap_err().contains("single key"));
    }

    #[test]
    fn each_target_spells_the_same_chord_its_own_way() {
        let k = spec("ctrl+alt+cmd+b");
        assert_eq!(k.skhd(), "ctrl + alt + cmd - b");
        assert_eq!(k.gnome(), "<Ctrl><Alt><Super>b");
        assert_eq!(k.sway(), "Ctrl+Mod1+Mod4+b");
        assert_eq!(k.hypr(), ("CTRL ALT SUPER".to_string(), 'B'));
    }

    #[test]
    fn the_restore_binding_is_the_same_chord_plus_shift() {
        let k = spec("ctrl+alt+b").plus_shift();
        assert_eq!(k.skhd(), "ctrl + alt + shift - b");
        assert_eq!(k.sway(), "Ctrl+Mod1+Shift+b");
    }

    #[test]
    fn merging_into_an_empty_file_just_writes_the_block() {
        let out = merge_block("", "line one");
        assert!(out.starts_with(BEGIN));
        assert!(out.contains("line one"));
        assert!(out.trim_end().ends_with(END));
    }

    #[test]
    fn merging_preserves_what_the_user_wrote_around_it() {
        let existing = "# my own bindings\ncmd - x : say hi\n";
        let once = merge_block(existing, "new binding");
        assert!(once.contains("cmd - x : say hi"), "clobbered the user's config");
        assert!(once.contains("new binding"));
    }

    #[test]
    fn merging_is_idempotent_and_replaces_rather_than_stacks() {
        let first = merge_block("keep me\n", "v1");
        let second = merge_block(&first, "v2");
        let third = merge_block(&second, "v2");

        assert_eq!(second, third, "re-running with the same body must change nothing");
        assert!(second.contains("keep me"));
        assert!(second.contains("v2"));
        assert!(!second.contains("v1"), "old binding left behind: {second}");
        assert_eq!(second.matches(BEGIN).count(), 1, "markers accumulated");
    }

    #[test]
    fn text_after_the_block_survives_a_rewrite() {
        let existing = format!("before\n\n{BEGIN}\nold\n{END}\n\nafter\n");
        let out = merge_block(&existing, "new");
        assert!(out.contains("before"));
        assert!(out.contains("after"), "trailing config was dropped: {out}");
        assert!(out.contains("new") && !out.contains("old"));
    }

    #[test]
    fn uninstalling_leaves_only_what_the_user_wrote() {
        let existing = format!("mine\n\n{BEGIN}\nours\n{END}\n");
        let out = strip_block(&existing);
        assert!(out.contains("mine"));
        assert!(!out.contains("ours") && !out.contains(BEGIN), "{out}");
    }

    #[test]
    fn stripping_a_file_we_never_touched_changes_nothing() {
        assert_eq!(strip_block("just mine\n"), "just mine\n");
    }

    #[test]
    fn uninstalling_a_block_at_the_top_does_not_leave_blank_lines() {
        let existing = format!("{BEGIN}\nours\n{END}\n\n# my own\ncmd - x : echo hi\n");
        assert_eq!(strip_block(&existing), "# my own\ncmd - x : echo hi\n");
    }

    #[test]
    fn uninstalling_our_only_content_empties_the_file() {
        assert_eq!(strip_block(&format!("{BEGIN}\nours\n{END}\n")), "");
    }

    #[test]
    fn install_then_uninstall_returns_the_file_to_its_original_state() {
        for original in ["", "mine\n", "# head\n\nmine\n", "a\n\n\nb\n"] {
            let round_tripped = strip_block(&merge_block(original, "ours"));
            assert_eq!(round_tripped, original, "round trip changed {original:?}");
        }
    }

    #[test]
    fn the_default_chord_is_free_of_shift_so_restore_can_use_it() {
        let d = KeySpec::default_for_platform();
        assert!(!d.shift);
        assert!(d.ctrl && d.alt);
    }
}
