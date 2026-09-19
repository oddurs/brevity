//! Configuration: process env first, then an env file, then built-in defaults.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Provider {
    Anthropic,
    Google,
    /// Everything that speaks POST {base}/chat/completions.
    OpenAiCompat,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MaxTokensField {
    Auto,
    MaxTokens,
    MaxCompletionTokens,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Notify {
    Off,
    Errors,
    Always,
}

pub struct Config {
    pub provider: Provider,
    pub provider_name: String,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: String,
    pub extra_headers: Vec<(String, String)>,
    pub max_tokens: u32,
    pub max_tokens_field: MaxTokensField,
    pub temperature: Option<f64>,
    pub effort: Option<String>,
    pub timeout: Duration,

    pub system_prompt: String,
    pub prompt_template: String,
    pub max_words: usize,
    pub style: Option<String>,

    pub min_input_chars: usize,
    pub max_input_chars: usize,

    pub ui: Ui,
    pub history: bool,
    pub allow_secrets: bool,
}

/// Feedback settings. Loaded on their own so a misconfigured provider can still
/// buzz and notify rather than failing silently behind a hotkey.
pub struct Ui {
    pub sound: bool,
    pub sound_file: Option<String>,
    pub sound_error_file: Option<String>,
    pub sound_cmd: Option<String>,
    pub sound_volume: f32,
    pub notify: Notify,
}

impl Ui {
    pub fn load(env: &Env) -> Ui {
        let notify = match env.get_or("BREVITY_NOTIFY", "errors").trim().to_lowercase().as_str() {
            "off" | "never" | "0" | "false" | "no" => Notify::Off,
            "always" | "on" | "1" | "true" | "yes" => Notify::Always,
            _ => Notify::Errors,
        };
        Ui {
            sound: env.flag("BREVITY_SOUND", true),
            sound_file: env.get("BREVITY_SOUND_FILE"),
            sound_error_file: env.get("BREVITY_SOUND_ERROR_FILE"),
            sound_cmd: env.get("BREVITY_SOUND_CMD"),
            sound_volume: env.num("BREVITY_SOUND_VOLUME", 0.5f32).clamp(0.0, 1.0),
            notify,
        }
    }
}

/// `~/.config/brevity/.env` (honours XDG_CONFIG_HOME), or $BREVITY_ENV_FILE.
pub fn env_file_path() -> PathBuf {
    if let Ok(p) = std::env::var("BREVITY_ENV_FILE") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    config_dir().join(".env")
}

pub fn config_dir() -> PathBuf {
    if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
        if !x.is_empty() {
            return PathBuf::from(x).join("brevity");
        }
    }
    home().join(".config").join("brevity")
}

pub fn cache_dir() -> PathBuf {
    if let Ok(x) = std::env::var("XDG_CACHE_HOME") {
        if !x.is_empty() {
            return PathBuf::from(x).join("brevity");
        }
    }
    if cfg!(target_os = "macos") {
        home().join("Library/Caches/brevity")
    } else {
        home().join(".cache").join("brevity")
    }
}

pub fn home() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."))
}

/// Minimal dotenv parser. Real environment variables always win.
pub fn parse_env_file(path: &Path) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Ok(body) = std::fs::read_to_string(path) else { return out };
    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((k, v)) = line.split_once('=') else { continue };
        let key = k.trim().to_string();
        if key.is_empty() {
            continue;
        }
        let v = v.trim();
        let value = if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
            unescape(&v[1..v.len() - 1])
        } else if v.len() >= 2 && v.starts_with('\'') && v.ends_with('\'') {
            v[1..v.len() - 1].to_string()
        } else {
            // strip a trailing ` # comment` from unquoted values
            match v.find(" #") {
                Some(i) => v[..i].trim().to_string(),
                None => v.to_string(),
            }
        };
        out.insert(key, value);
    }
    out
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

pub struct Env {
    file: HashMap<String, String>,
    /// Tests build hermetic environments that ignore the real process env.
    process: bool,
}

impl Env {
    pub fn load() -> Env {
        Env { file: parse_env_file(&env_file_path()), process: true }
    }

    #[cfg(test)]
    pub fn from_pairs(pairs: &[(&str, &str)]) -> Env {
        Env {
            file: pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            process: false,
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        if self.process {
            if let Ok(v) = std::env::var(key) {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
        self.file.get(key).filter(|v| !v.is_empty()).cloned()
    }

    fn get_or(&self, key: &str, default: &str) -> String {
        self.get(key).unwrap_or_else(|| default.to_string())
    }

    fn num<T: std::str::FromStr>(&self, key: &str, default: T) -> T {
        self.get(key).and_then(|v| v.trim().parse().ok()).unwrap_or(default)
    }

    fn flag(&self, key: &str, default: bool) -> bool {
        match self.get(key) {
            Some(v) => {
                matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on" | "y")
            }
            None => default,
        }
    }
}

const DEFAULT_SYSTEM: &str = "You compress text for someone who will paste the result somewhere else. \
Reply with the summary and nothing else: no preamble, no sign-off, no meta commentary, no markdown code fences, no \"Summary:\" label. \
Write in the same language as the input. \
Keep concrete facts that carry the meaning: names, numbers, dates, decisions, deadlines, action items, and URLs. \
Stay under {max_words} words. Use tight prose by default; use a short bullet list only when the source is itself a list or covers several unrelated points.";

const DEFAULT_PROMPT: &str = "Summarize the text between the markers.\n\n<<<TEXT\n{text}\nTEXT>>>";

/// Written by hand so an API key can never reach a log line through `{:?}`.
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("provider", &self.provider_name)
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("max_tokens", &self.max_tokens)
            .field("effort", &self.effort)
            .field("style", &self.style)
            .finish_non_exhaustive()
    }
}

impl Config {
    pub fn load(env: &Env, style_override: Option<String>) -> Result<Config, String> {
        let provider_name = env.get_or("BREVITY_PROVIDER", "anthropic").trim().to_lowercase();

        let (provider, default_base, key_vars): (Provider, &str, &[&str]) =
            match provider_name.as_str() {
                "anthropic" | "claude" => {
                    (Provider::Anthropic, "https://api.anthropic.com", &["ANTHROPIC_API_KEY"])
                }
                "google" | "gemini" => (
                    Provider::Google,
                    "https://generativelanguage.googleapis.com",
                    &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
                ),
                "openai" => (Provider::OpenAiCompat, "https://api.openai.com/v1", &["OPENAI_API_KEY"]),
                "openrouter" => (
                    Provider::OpenAiCompat,
                    "https://openrouter.ai/api/v1",
                    &["OPENROUTER_API_KEY"],
                ),
                "groq" => (
                    Provider::OpenAiCompat,
                    "https://api.groq.com/openai/v1",
                    &["GROQ_API_KEY"],
                ),
                "deepseek" => (
                    Provider::OpenAiCompat,
                    "https://api.deepseek.com/v1",
                    &["DEEPSEEK_API_KEY"],
                ),
                "mistral" => (
                    Provider::OpenAiCompat,
                    "https://api.mistral.ai/v1",
                    &["MISTRAL_API_KEY"],
                ),
                "together" => (
                    Provider::OpenAiCompat,
                    "https://api.together.xyz/v1",
                    &["TOGETHER_API_KEY"],
                ),
                "ollama" => (Provider::OpenAiCompat, "http://localhost:11434/v1", &[]),
                "lmstudio" => (Provider::OpenAiCompat, "http://localhost:1234/v1", &[]),
                "llamacpp" | "llama.cpp" => (Provider::OpenAiCompat, "http://localhost:8080/v1", &[]),
                "vllm" => (Provider::OpenAiCompat, "http://localhost:8000/v1", &[]),
                "openai-compat" | "custom" | "local" => (Provider::OpenAiCompat, "", &[]),
                other => {
                    return Err(format!(
                        "unknown BREVITY_PROVIDER '{other}'. Known: anthropic, openai, openrouter, google, \
groq, deepseek, mistral, together, ollama, lmstudio, llamacpp, vllm, openai-compat"
                    ))
                }
            };

        let base_url = env
            .get("BREVITY_BASE_URL")
            .unwrap_or_else(|| default_base.to_string())
            .trim_end_matches('/')
            .to_string();
        if base_url.is_empty() {
            return Err(format!(
                "provider '{provider_name}' needs an endpoint: set BREVITY_BASE_URL in {}",
                env_file_path().display()
            ));
        }

        let api_key =
            env.get("BREVITY_API_KEY").or_else(|| key_vars.iter().find_map(|k| env.get(k)));
        if api_key.is_none() && !key_vars.is_empty() {
            return Err(format!(
                "no API key for '{provider_name}': set {} (or BREVITY_API_KEY) in {}",
                key_vars.join(" or "),
                env_file_path().display()
            ));
        }

        let model = match env.get("BREVITY_MODEL") {
            Some(m) => m,
            None if provider == Provider::Anthropic => "claude-opus-5".to_string(),
            None => {
                return Err(format!(
                    "set BREVITY_MODEL for provider '{provider_name}' in {}",
                    env_file_path().display()
                ))
            }
        };

        let style = style_override
            .or_else(|| env.get("BREVITY_STYLE"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "default");

        // A style named `bullets` looks for BREVITY_PROMPT_BULLETS / BREVITY_SYSTEM_PROMPT_BULLETS.
        let suffix = style
            .as_deref()
            .map(|s| format!("_{}", s.to_uppercase().replace(['-', ' ', '.'], "_")))
            .unwrap_or_default();
        let styled = |base: &str| -> Option<String> {
            if suffix.is_empty() {
                None
            } else {
                env.get(&format!("{base}{suffix}"))
            }
        };

        if let Some(s) = style.as_deref() {
            if styled("BREVITY_PROMPT").is_none() && styled("BREVITY_SYSTEM_PROMPT").is_none() {
                return Err(format!(
                    "style '{s}' is not defined: add BREVITY_PROMPT{suffix} (and/or BREVITY_SYSTEM_PROMPT{suffix}) to {}",
                    env_file_path().display()
                ));
            }
        }

        let system_prompt = styled("BREVITY_SYSTEM_PROMPT")
            .or_else(|| env.get("BREVITY_SYSTEM_PROMPT"))
            .unwrap_or_else(|| DEFAULT_SYSTEM.to_string());
        let prompt_template = styled("BREVITY_PROMPT")
            .or_else(|| env.get("BREVITY_PROMPT"))
            .unwrap_or_else(|| DEFAULT_PROMPT.to_string());

        let max_tokens_field = match env.get_or("BREVITY_MAX_TOKENS_FIELD", "auto").as_str() {
            "max_tokens" => MaxTokensField::MaxTokens,
            "max_completion_tokens" => MaxTokensField::MaxCompletionTokens,
            _ => MaxTokensField::Auto,
        };

        let mut extra_headers = Vec::new();
        if provider_name == "openrouter" {
            extra_headers
                .push(("X-Title".to_string(), env.get_or("BREVITY_OPENROUTER_TITLE", "Brevity")));
            if let Some(referer) = env.get("BREVITY_OPENROUTER_REFERER") {
                extra_headers.push(("HTTP-Referer".to_string(), referer));
            }
        }
        if let Some(raw) = env.get("BREVITY_EXTRA_HEADERS") {
            // "Header: value; Other: value"
            for part in raw.split(';') {
                if let Some((k, v)) = part.split_once(':') {
                    let (k, v) = (k.trim(), v.trim());
                    if !k.is_empty() && !v.is_empty() {
                        extra_headers.push((k.to_string(), v.to_string()));
                    }
                }
            }
        }

        Ok(Config {
            provider,
            provider_name,
            model,
            api_key,
            base_url,
            extra_headers,
            max_tokens: env.num("BREVITY_MAX_TOKENS", 1024u32),
            max_tokens_field,
            temperature: env.get("BREVITY_TEMPERATURE").and_then(|v| v.trim().parse().ok()),
            effort: env.get("BREVITY_EFFORT").or_else(|| {
                if provider == Provider::Anthropic {
                    Some("low".to_string())
                } else {
                    None
                }
            }),
            timeout: Duration::from_secs(env.num("BREVITY_TIMEOUT_SECS", 90u64)),
            system_prompt,
            prompt_template,
            max_words: env.num("BREVITY_MAX_WORDS", 120usize),
            style,
            min_input_chars: env.num("BREVITY_MIN_INPUT_CHARS", 0usize),
            max_input_chars: env.num("BREVITY_MAX_INPUT_CHARS", 200_000usize),
            ui: Ui::load(env),
            allow_secrets: env.flag("BREVITY_ALLOW_SECRETS", false),
            history: env.flag("BREVITY_HISTORY", true),
        })
    }

    pub fn render(&self, template: &str, text: &str) -> String {
        template.replace("{max_words}", &self.max_words.to_string()).replace("{text}", text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(body: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "brevity-test-{}-{}.env",
            std::process::id(),
            body.len()
        ));
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn env_file_parses_the_shapes_people_actually_write() {
        let path = write_tmp(
            "# a comment\n\
             BREVITY_PROVIDER=openai\n\
             export BREVITY_MODEL=gpt-test\n\
             BREVITY_PROMPT=\"line one\\nline two\"\n\
             BREVITY_SINGLE='raw \\n stays'\n\
             BREVITY_MAX_TOKENS=512 # trailing note\n\
             \n\
             =nokey\n",
        );
        let m = parse_env_file(&path);
        std::fs::remove_file(&path).ok();

        assert_eq!(m.get("BREVITY_PROVIDER").unwrap(), "openai");
        assert_eq!(
            m.get("BREVITY_MODEL").unwrap(),
            "gpt-test",
            "`export ` prefix should be ignored"
        );
        assert_eq!(
            m.get("BREVITY_PROMPT").unwrap(),
            "line one\nline two",
            "double quotes unescape"
        );
        assert_eq!(m.get("BREVITY_SINGLE").unwrap(), "raw \\n stays", "single quotes stay literal");
        assert_eq!(m.get("BREVITY_MAX_TOKENS").unwrap(), "512", "inline comment stripped");
        assert!(!m.contains_key(""));
    }

    #[test]
    fn anthropic_is_the_default_and_needs_only_a_key() {
        let env = Env::from_pairs(&[("ANTHROPIC_API_KEY", "sk-ant-x")]);
        let c = Config::load(&env, None).unwrap();
        assert_eq!(c.provider, Provider::Anthropic);
        assert_eq!(c.model, "claude-opus-5");
        assert_eq!(c.base_url, "https://api.anthropic.com");
        assert_eq!(c.effort.as_deref(), Some("low"));
        assert!(c.temperature.is_none(), "the Claude 5 family rejects temperature");
    }

    #[test]
    fn a_missing_key_is_an_error_not_a_silent_request() {
        let err = Config::load(&Env::from_pairs(&[]), None).unwrap_err();
        assert!(err.contains("ANTHROPIC_API_KEY"), "{err}");
    }

    #[test]
    fn local_providers_need_no_key_but_do_need_a_model() {
        let err =
            Config::load(&Env::from_pairs(&[("BREVITY_PROVIDER", "ollama")]), None).unwrap_err();
        assert!(err.contains("BREVITY_MODEL"), "{err}");

        let env = Env::from_pairs(&[("BREVITY_PROVIDER", "ollama"), ("BREVITY_MODEL", "llama3.2")]);
        let c = Config::load(&env, None).unwrap();
        assert_eq!(c.provider, Provider::OpenAiCompat);
        assert_eq!(c.base_url, "http://localhost:11434/v1");
        assert!(c.api_key.is_none());
        assert!(c.effort.is_none(), "effort is an Anthropic-only knob");
    }

    #[test]
    fn openai_compat_demands_an_endpoint() {
        let env = Env::from_pairs(&[("BREVITY_PROVIDER", "openai-compat"), ("BREVITY_MODEL", "m")]);
        assert!(Config::load(&env, None).unwrap_err().contains("BREVITY_BASE_URL"));
    }

    #[test]
    fn base_url_override_wins_and_loses_its_trailing_slash() {
        let env = Env::from_pairs(&[
            ("ANTHROPIC_API_KEY", "k"),
            ("BREVITY_BASE_URL", "http://proxy.local:8080/"),
        ]);
        assert_eq!(Config::load(&env, None).unwrap().base_url, "http://proxy.local:8080");
    }

    #[test]
    fn unknown_provider_lists_the_known_ones() {
        let env = Env::from_pairs(&[("BREVITY_PROVIDER", "skynet")]);
        let err = Config::load(&env, None).unwrap_err();
        assert!(err.contains("skynet") && err.contains("openrouter"), "{err}");
    }

    #[test]
    fn named_styles_resolve_and_typos_are_caught() {
        let env = Env::from_pairs(&[
            ("ANTHROPIC_API_KEY", "k"),
            ("BREVITY_PROMPT_BULLETS", "bulletize {text}"),
        ]);
        let c = Config::load(&env, Some("bullets".into())).unwrap();
        assert_eq!(c.prompt_template, "bulletize {text}");
        assert_eq!(c.style.as_deref(), Some("bullets"));

        let err = Config::load(&env, Some("bulets".into())).unwrap_err();
        assert!(err.contains("BREVITY_PROMPT_BULETS"), "{err}");
    }

    #[test]
    fn style_names_normalize_to_env_var_spelling() {
        let env = Env::from_pairs(&[
            ("ANTHROPIC_API_KEY", "k"),
            ("BREVITY_PROMPT_ACTION_ITEMS", "actions {text}"),
        ]);
        assert!(Config::load(&env, Some("action-items".into())).is_ok());
    }

    #[test]
    fn templates_substitute_both_placeholders() {
        let env = Env::from_pairs(&[("ANTHROPIC_API_KEY", "k"), ("BREVITY_MAX_WORDS", "40")]);
        let c = Config::load(&env, None).unwrap();
        assert_eq!(c.render("{max_words}w: {text}", "hi"), "40w: hi");
    }

    #[test]
    fn clipboard_text_is_never_treated_as_a_placeholder() {
        let env = Env::from_pairs(&[("ANTHROPIC_API_KEY", "k")]);
        let c = Config::load(&env, None).unwrap();
        // Text that itself contains {max_words} must survive verbatim.
        assert_eq!(c.render("<{text}>", "a {max_words} b"), "<a {max_words} b>");
    }
}
