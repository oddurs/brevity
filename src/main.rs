//! brevity - summarize the clipboard, chime, put the summary back.

mod clipboard;
mod config;
mod hotkey;
mod provider;
mod secrets;
mod sound;

use std::io::Read;
use std::process::{Command, Stdio};

use config::{cache_dir, config_dir, env_file_path, Config, Env, Notify, Ui};
use sound::Chime;

const USAGE: &str = "\
brevity - summarize whatever is on the clipboard, then put the summary back.

USAGE:
    brevity [OPTIONS]

OPTIONS:
    -s, --style <NAME>        Use the prompt defined by BREVITY_PROMPT_<NAME>
    -p, --print               Also write the summary to stdout
    -n, --no-replace          Leave the clipboard alone (implies --print)
        --stdin               Summarize stdin instead of the clipboard
        --allow-secrets       Summarize even if the text looks like a credential
        --restore             Put the last replaced clipboard contents back

    --install-hotkey [KEY]    Bind a global hotkey, default ctrl+alt+cmd+b
    --uninstall-hotkey        Remove the bindings --install-hotkey wrote

        --chime [error]       Play the success (or failure) sound and exit
        --config              Show the resolved configuration (keys redacted)
        --init                Create the env file if it does not exist
        --edit                Open the env file in $EDITOR
    -h, --help                Show this help
    -V, --version             Show the version

CONFIG:
    ";

const TEMPLATE: &str = include_str!("../.env.example");

fn main() {
    let code = run();
    std::process::exit(code);
}

fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut style: Option<String> = None;
    let mut print = false;
    let mut replace = true;
    let mut from_stdin = false;
    let mut allow_secrets = false;

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}{}", env_file_path().display());
                return 0;
            }
            "-V" | "--version" => {
                println!("brevity {}", env!("CARGO_PKG_VERSION"));
                return 0;
            }
            "--init" => return init_env_file(),
            "--edit" => return edit_env_file(),
            "--restore" => return restore(),
            "--install-hotkey" => {
                // An optional chord may follow; anything starting with `-` is not one.
                let spec = it.clone().next().filter(|a| !a.starts_with('-')).cloned();
                if spec.is_some() {
                    it.next();
                }
                return match hotkey::install(spec.as_deref()) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("brevity: {e}");
                        1
                    }
                };
            }
            "--uninstall-hotkey" => {
                return match hotkey::uninstall() {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("brevity: {e}");
                        1
                    }
                };
            }
            "--chime" => {
                let ui = Ui::load(&Env::load());
                let which = match it.next().map(|s| s.as_str()) {
                    Some("error") => Chime::Error,
                    _ => Chime::Done,
                };
                sound::play(&ui, which);
                return 0;
            }
            "--config" => return show_config(),
            "-s" | "--style" => match it.next() {
                Some(v) => style = Some(v.clone()),
                None => {
                    eprintln!("brevity: --style needs a name");
                    return 2;
                }
            },
            "-p" | "--print" => print = true,
            "-n" | "--no-replace" => {
                replace = false;
                print = true;
            }
            "--stdin" | "-" => from_stdin = true,
            "--allow-secrets" => allow_secrets = true,
            other => {
                if let Some(v) = other.strip_prefix("--style=") {
                    style = Some(v.to_string());
                } else {
                    eprintln!("brevity: unknown option '{other}' (try --help)");
                    return 2;
                }
            }
        }
    }

    let env = Env::load();
    let ui = Ui::load(&env);

    let cfg = match Config::load(&env, style) {
        Ok(c) => c,
        Err(e) => return fail(&ui, &e),
    };

    let input = if from_stdin {
        let mut buf = String::new();
        match std::io::stdin().read_to_string(&mut buf) {
            Ok(_) => buf,
            Err(e) => return fail(&cfg.ui, &format!("could not read stdin: {e}")),
        }
    } else {
        match clipboard::read() {
            Ok(t) => t,
            Err(e) => return fail(&cfg.ui, &e),
        }
    };

    let text = input.trim();
    if text.is_empty() {
        return fail(
            &cfg.ui,
            "the clipboard holds no text - an image, a file or an empty clipboard \
             has nothing to summarize",
        );
    }
    let chars = text.chars().count();
    if chars < cfg.min_input_chars {
        return fail(
            &cfg.ui,
            &format!(
                "only {chars} characters - below BREVITY_MIN_INPUT_CHARS ({})",
                cfg.min_input_chars
            ),
        );
    }
    if chars > cfg.max_input_chars {
        return fail(
            &cfg.ui,
            &format!(
                "{chars} characters is over BREVITY_MAX_INPUT_CHARS ({}); nothing was sent",
                cfg.max_input_chars
            ),
        );
    }

    // Nothing here can be undone once the request leaves: the provider has the
    // text and the history file has a copy. Check before, not after.
    if !(allow_secrets || cfg.allow_secrets) {
        if let Some(kind) = secrets::detect(text) {
            return fail(
                &cfg.ui,
                &format!(
                    "that looks like {kind}, so it was not sent to {}. \
                     Use --allow-secrets, or set BREVITY_ALLOW_SECRETS=true, if you meant it.",
                    cfg.provider_name
                ),
            );
        }
    }

    let summary = match provider::summarize(&cfg, text) {
        Ok(s) => s,
        Err(e) => return fail(&cfg.ui, &e),
    };

    if replace {
        if cfg.history {
            save_original(text);
        }
        if let Err(e) = clipboard::write(&summary) {
            return fail(&cfg.ui, &e);
        }
    }

    sound::play(&cfg.ui, Chime::Done);

    if print {
        println!("{summary}");
    }
    if cfg.ui.notify == Notify::Always {
        let words = summary.split_whitespace().count();
        notify("Brevity", &format!("{chars} chars -> {words} words"));
    }
    0
}

fn fail(ui: &Ui, msg: &str) -> i32 {
    eprintln!("brevity: {msg}");
    sound::play(ui, Chime::Error);
    if ui.notify != Notify::Off {
        notify("Brevity failed", msg);
    }
    1
}

fn notify(title: &str, body: &str) {
    let ok = if cfg!(target_os = "macos") {
        let script =
            format!("display notification \"{}\" with title \"{}\"", escape(body), escape(title));
        spawn("osascript", &["-e", &script])
    } else {
        spawn("notify-send", &["-a", "brevity", title, body])
    };
    let _ = ok;
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " ")
}

fn spawn(bin: &str, args: &[&str]) -> bool {
    Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn history_path() -> std::path::PathBuf {
    cache_dir().join("last-original.txt")
}

/// The replaced text can be anything that was on the clipboard - a password, a
/// token, a private message. Keep it readable only by its owner.
fn save_original(text: &str) {
    let path = history_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        }
    }
    if std::fs::write(&path, text).is_err() {
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
}

fn restore() -> i32 {
    let env = Env::load();
    let ui = Ui::load(&env);
    let path = history_path();
    match std::fs::read_to_string(&path) {
        Ok(text) if !text.is_empty() => match clipboard::write(&text) {
            Ok(()) => {
                sound::play(&ui, Chime::Done);
                0
            }
            Err(e) => fail(&ui, &e),
        },
        _ => fail(&ui, &format!("nothing to restore (no {})", path.display())),
    }
}

fn init_env_file() -> i32 {
    let path = env_file_path();
    if path.exists() {
        println!("{} already exists", path.display());
        return 0;
    }
    if let Err(e) = std::fs::create_dir_all(config_dir()) {
        eprintln!("brevity: could not create {}: {e}", config_dir().display());
        return 1;
    }
    if let Err(e) = std::fs::write(&path, TEMPLATE) {
        eprintln!("brevity: could not write {}: {e}", path.display());
        return 1;
    }
    // The file holds API keys.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    println!("wrote {}\nAdd your API key with: brevity --edit", path.display());
    0
}

fn edit_env_file() -> i32 {
    let path = env_file_path();
    if !path.exists() {
        let code = init_env_file();
        if code != 0 {
            return code;
        }
    }
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| if cfg!(target_os = "macos") { "open".into() } else { "nano".into() });
    match Command::new(&editor).arg(&path).status() {
        Ok(s) if s.success() => 0,
        Ok(_) => 1,
        Err(e) => {
            eprintln!("brevity: could not run {editor}: {e}");
            1
        }
    }
}

fn show_config() -> i32 {
    let env = Env::load();
    let path = env_file_path();
    println!("env file        {} {}", path.display(), if path.exists() { "" } else { "(missing)" });
    match Config::load(&env, None) {
        Ok(c) => {
            println!("provider        {}", c.provider_name);
            println!("model           {}", c.model);
            println!("base url        {}", c.base_url);
            println!(
                "api key         {}",
                match &c.api_key {
                    Some(k) => redact(k),
                    None => "(none - local endpoint)".to_string(),
                }
            );
            println!("max tokens      {}", c.max_tokens);
            println!("max words       {}", c.max_words);
            println!("effort          {}", c.effort.clone().unwrap_or_else(|| "(unset)".into()));
            println!(
                "temperature     {}",
                c.temperature.map(|t| t.to_string()).unwrap_or_else(|| "(unset)".into())
            );
            println!("timeout         {}s", c.timeout.as_secs());
            println!("style           {}", c.style.clone().unwrap_or_else(|| "default".into()));
            println!("input range     {}..{} chars", c.min_input_chars, c.max_input_chars);
            println!("sound           {}", if c.ui.sound { "on" } else { "off" });
            println!(
                "notify          {}",
                match c.ui.notify {
                    Notify::Off => "off",
                    Notify::Errors => "errors",
                    Notify::Always => "always",
                }
            );
            println!(
                "history         {}",
                if c.history { history_path().display().to_string() } else { "off".into() }
            );
            0
        }
        Err(e) => {
            println!("status          NOT READY: {e}");
            1
        }
    }
}

fn redact(k: &str) -> String {
    let n = k.chars().count();
    if n <= 8 {
        return "*".repeat(n);
    }
    let head: String = k.chars().take(6).collect();
    let tail: String = k.chars().skip(n - 4).collect();
    format!("{head}...{tail} ({n} chars)")
}

#[cfg(test)]
mod tests {
    use super::{escape, redact};

    #[test]
    fn keys_are_redacted_to_something_recognizable_but_useless() {
        let out = redact("sk-ant-api03-SECRETSECRETSECRET-9xyz");
        assert!(out.starts_with("sk-ant"), "{out}");
        assert!(out.ends_with("(36 chars)"), "{out}");
        assert!(!out.contains("SECRET"), "redaction leaked the key: {out}");
    }

    #[test]
    fn short_keys_reveal_nothing_at_all() {
        assert_eq!(redact("abcd1234"), "********");
    }

    #[test]
    fn notification_text_cannot_break_out_of_applescript() {
        let out = escape("he said \"hi\"\\n\nnew line");
        assert!(!out.contains('\n'), "newlines would truncate the osascript line");
        assert!(!out.contains("\"hi\""), "unescaped quotes would end the string early: {out}");
    }
}
