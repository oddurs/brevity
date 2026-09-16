# brevity

[![CI](https://github.com/oddurs/brevity/actions/workflows/ci.yml/badge.svg)](https://github.com/oddurs/brevity/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Copy something long. Press a key. A chime tells you the summary has replaced it
on your clipboard. No window, no terminal, no output.

Rust, single binary, macOS and Linux. Works with Anthropic, OpenAI, OpenRouter,
Gemini, Groq and friends, or a model running on your own machine.

## Install

```sh
git clone https://github.com/oddurs/brevity
cd brevity
./install.sh          # builds, installs to ~/.local/bin, writes the env file
brevity --edit        # add an API key
```

Needs a Rust toolchain ([rustup.rs](https://rustup.rs)). Prebuilt binaries for
macOS (Apple Silicon and Intel) and Linux x86_64 are attached to each
[release](https://github.com/oddurs/brevity/releases).

Linux also needs a clipboard tool — `wl-clipboard` on Wayland, `xclip` on X11.
(A short-lived process cannot own the X11 clipboard by itself; `xclip` and
`wl-copy` fork a helper that keeps serving it, which is why brevity shells out
to them instead of linking a clipboard library.)

Then bind it to a key: see [`hotkeys/`](hotkeys/) for Shortcuts.app, Raycast,
Hammerspoon, skhd, GNOME, KDE, sway, Hyprland and i3.

## Configure

Everything lives in `~/.config/brevity/.env` (see [`.env.example`](.env.example)).
Real environment variables override it.

```ini
BREVITY_PROVIDER=anthropic
ANTHROPIC_API_KEY=sk-ant-...
```

That is the whole minimum configuration. Other backends:

| Backend | Settings |
|---|---|
| Anthropic | `BREVITY_PROVIDER=anthropic`, `ANTHROPIC_API_KEY` (model defaults to `claude-opus-5`) |
| OpenAI | `BREVITY_PROVIDER=openai`, `OPENAI_API_KEY`, `BREVITY_MODEL` |
| OpenRouter | `BREVITY_PROVIDER=openrouter`, `OPENROUTER_API_KEY`, `BREVITY_MODEL` |
| Gemini | `BREVITY_PROVIDER=google`, `GEMINI_API_KEY`, `BREVITY_MODEL` |
| Groq / DeepSeek / Mistral / Together | `BREVITY_PROVIDER=<name>`, `<NAME>_API_KEY`, `BREVITY_MODEL` |
| Ollama | `BREVITY_PROVIDER=ollama`, `BREVITY_MODEL=llama3.2` |
| LM Studio / llama.cpp / vLLM | `BREVITY_PROVIDER=lmstudio` \| `llamacpp` \| `vllm`, `BREVITY_MODEL` |
| Anything OpenAI-shaped | `BREVITY_PROVIDER=openai-compat`, `BREVITY_BASE_URL`, `BREVITY_MODEL` |

Local backends need no key. `BREVITY_BASE_URL` overrides the endpoint of any
provider, so proxies and gateways work too.

`brevity --config` prints what it actually resolved, with the key redacted.

## Change the prompt

The built-in prompt asks for the summary and nothing else, under
`BREVITY_MAX_WORDS` words, keeping names, numbers, dates, decisions and URLs.
Replace it wholesale:

```ini
BREVITY_MAX_WORDS=60
BREVITY_SYSTEM_PROMPT="You compress text. Output only the summary. Under {max_words} words."
BREVITY_PROMPT="Summarize this:\n\n{text}"
```

`{text}` is the clipboard contents and `{max_words}` is the word budget; both
work in either template.

### Named styles

Define as many prompts as you like and pick one per invocation:

```ini
BREVITY_PROMPT_BULLETS="Rewrite as at most 5 terse bullets. Output only bullets.\n\n{text}"
BREVITY_PROMPT_ACTIONS="List only the action items, owners and deadlines.\n\n{text}"
```

```sh
brevity --style bullets
```

Bind each style to its own key, or set `BREVITY_STYLE` to make one the default.

## Usage

```
brevity                 summarize the clipboard, chime, replace it
brevity --style NAME    use a named prompt
brevity --print         also print the summary
brevity --no-replace    leave the clipboard alone (implies --print)
brevity --stdin         summarize stdin instead:  git log | brevity --stdin -p
brevity --restore       put the replaced text back
brevity --chime         preview the sound (add `error` for the failure tone)
brevity --config        show the resolved configuration
brevity --edit          open the env file
```

## Behaviour worth knowing

- **The original is never lost.** The replaced text is written to
  `~/.cache/brevity/last-original.txt` before the clipboard is overwritten, and
  `brevity --restore` brings it back. Turn it off with `BREVITY_HISTORY=false`.
- **The clipboard is only touched on success.** Any failure leaves it exactly as
  it was.
- **Failure is audible.** A falling two-tone plus a desktop notification, since
  there is no terminal to read. `BREVITY_NOTIFY=off|errors|always`.
- **It refuses rather than truncates.** Input above `BREVITY_MAX_INPUT_CHARS`
  (200k) is never silently cut down — raise the limit if you mean it.
- **The chime is generated, not shipped.** A short rising blip, synthesized into
  `~/.cache/brevity/` on first run, played with `afplay` / `pw-play` / `paplay` /
  `aplay` / `ffplay` / `play` / `mpv`. Point `BREVITY_SOUND_FILE` at your own
  file, adjust `BREVITY_SOUND_VOLUME`, or set `BREVITY_SOUND=false`.
- **Effort, not model size, is the speed knob on Anthropic.**
  `BREVITY_EFFORT=low` is the default and is plenty for summarizing; raise it if
  you are summarizing something dense.

## Exit codes

`0` success · `1` failure (sound + notification) · `2` bad arguments.

## Contributing

Issues and pull requests are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md)
for the workflow, and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for the ground
rules. `scripts/mock-provider.py` lets you work on almost all of it without an
API key.

## Security

brevity sends your clipboard to whichever provider you configure, which is worth
thinking about before you bind it to a key. [SECURITY.md](SECURITY.md) spells out
exactly what goes where, what gets written to disk, and how to report a
vulnerability privately.

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual-licensed as above, without any additional terms or conditions.
