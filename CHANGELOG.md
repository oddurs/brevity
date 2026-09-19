# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Configuration keys are part of the public interface: renaming or removing a
`BREVITY_*` variable is a breaking change.

## [Unreleased]

### Added

- Credentials on the clipboard are refused instead of summarized. API keys,
  tokens, private keys and JWTs are recognized by shape and stop the run before
  anything is sent. The mistake this prevents — copy a key, forget it is there,
  press the hotkey — cannot be undone afterwards: the text reaches the provider
  and the history file at once. `--allow-secrets` or `BREVITY_ALLOW_SECRETS=true`
  overrides it.

### Fixed

- Non-ASCII text from a hotkey. `pbpaste` and `pbcopy` take their encoding from
  the locale, and a hotkey daemon runs under launchd with no locale set, so they
  fell back to Mac OS Roman. Reading failed outright with `clipboard does not
  hold UTF-8 text` on anything containing an accent, a typographic dash or a
  smart quote, and — less visibly — writing mangled every non-ASCII character in
  the summary, so `café —` was pasted back as `caf‚àö¬© ‚Äû`. brevity now forces
  a UTF-8 locale on both tools. Running from a terminal masked this entirely,
  because interactive shells set `LANG`.
- A clipboard that cannot be decoded now degrades to a lossy read with a warning
  instead of refusing to run, and a BOM or CRLF line endings are stripped before
  the text reaches the model.
- The empty-clipboard message now mentions that images and files have nothing to
  summarize, which is the usual reason for it.

### Added

- `brevity --install-hotkey [KEY]` and `--uninstall-hotkey`. Binding a key was
  the only part of setup that meant clicking through a settings UI. It now
  installs skhd through Homebrew on macOS, and writes GNOME, sway, Hyprland or
  i3 configuration directly on Linux. Edits are confined to a marked block, so
  re-running is idempotent and existing bindings are never disturbed. macOS
  still requires an Accessibility grant, which no program can give itself; the
  installer opens the pane and says so.

- `TROUBLESHOOTING.md`, covering the failure modes that are hard to diagnose
  behind a hotkey: a key on the wrong line, empty summaries from reasoning
  models, silent hotkeys, missing audio players, and recovering a credential
  that was on the clipboard.

### Changed

- README: a "Choosing a model" section with measured latencies, and a worked
  OpenRouter example. Summarizing rewards speed and brevity over reasoning, and
  reasoning models spend `max_tokens` on thinking before they answer — both are
  now documented rather than discovered.
- `.env.example`: notes that `BREVITY_MAX_WORDS` asks rather than enforces, that
  `BREVITY_MAX_TOKENS` is the real ceiling and the wrong length knob, and that
  `BREVITY_EFFORT` is Anthropic-only.

## [0.1.0] - 2026-09-16

First release.

### Added

- Summarize the clipboard, chime, and replace it with the summary. No window,
  no terminal output.
- Providers: Anthropic, OpenAI, OpenRouter, Google Gemini, Groq, DeepSeek,
  Mistral, Together, plus Ollama, LM Studio, llama.cpp and vLLM for local
  models, and `openai-compat` for anything else that speaks
  `POST /chat/completions`.
- Configuration through `~/.config/brevity/.env`, overridable by real
  environment variables. `--init`, `--edit` and `--config` manage it.
- Editable prompts via `BREVITY_SYSTEM_PROMPT` and `BREVITY_PROMPT`, with
  `{text}` and `{max_words}` placeholders.
- Named prompt styles: define `BREVITY_PROMPT_<NAME>`, run `--style <name>`.
- `--restore`, undoing a replacement from `~/.cache/brevity/last-original.txt`.
- A generated chime — a rising blip on success, falling on failure — plus a
  desktop notification when something goes wrong.
- `--stdin`, `--print`, `--no-replace` and `--chime`.
- Hotkey recipes for Shortcuts.app, Raycast, Hammerspoon, skhd, GNOME, KDE,
  sway, Hyprland and i3.

[Unreleased]: https://github.com/oddurs/brevity/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/oddurs/brevity/releases/tag/v0.1.0
