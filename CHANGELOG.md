# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Configuration keys are part of the public interface: renaming or removing a
`BREVITY_*` variable is a breaking change.

## [Unreleased]

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
