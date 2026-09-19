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

Then bind it to a key:

```sh
brevity --install-hotkey            # ⌃⌥⌘B, and ⌃⌥⇧⌘B to undo
brevity --install-hotkey ctrl+alt+s # or pick your own chord
```

That writes the binding and starts the daemon that listens for it — skhd on
macOS (installed via Homebrew if absent), and your own compositor's config on
Linux, where GNOME, sway, Hyprland and i3 are configured directly. It only
rewrites the lines between its own markers, so it will not disturb bindings you
already have, and `brevity --uninstall-hotkey` removes them again.

**macOS asks for one thing back:** skhd needs Accessibility permission before it
can see key presses, which only you can grant. The installer opens the right
settings pane and tells you what to switch on. Grant it, then
`skhd --restart-service`.

Prefer to do it yourself, or on KDE? [`hotkeys/`](hotkeys/) has the syntax for
Shortcuts.app, Raycast, Hammerspoon, skhd, GNOME, KDE, sway, Hyprland and i3.

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

### Worked example: OpenRouter

One key reaches every model, which makes it the easiest place to try several
before settling. Three lines:

```ini
BREVITY_PROVIDER=openrouter
OPENROUTER_API_KEY=sk-or-v1-...
BREVITY_MODEL=google/gemini-3.1-flash-lite
```

The model is a full OpenRouter slug — `vendor/model`, not the vendor's own
model id. `curl -s https://openrouter.ai/api/v1/models | jq -r '.data[].id'`
lists every one, no key required.

## Choosing a model

Summarizing is undemanding work, so the differences that matter here are speed
and whether the model respects a length budget — not reasoning ability. A
measured comparison, same 60-line input, through OpenRouter:

| Model | Time | Words returned |
|---|---|---|
| `google/gemini-3.1-flash-lite` | 2.5s | 93 |
| `google/gemini-2.5-flash` | 3.1s | 97 |
| `anthropic/claude-haiku-4.5` | 5.2s | 139 |
| `anthropic/claude-opus-5` | 7.6s | 230 |
| `google/gemini-3.8-flash` | 10.9s | 14 |

Two things worth taking from that.

**Faster models are not worse at this.** The 2.5s summary and the 7.6s one were
both accurate; the slow one was simply longer. When the task is compression, a
small model that stays inside the budget beats a large one that overruns it.

**Beware thinking models.** `gemini-3.8-flash` reasons before answering, and on
an OpenAI-shaped endpoint those reasoning tokens are spent from `max_tokens`.
It burned almost the entire 1024-token budget thinking and had room for 14
words of actual summary. If you want a reasoning model, raise
`BREVITY_MAX_TOKENS` to several thousand — otherwise pick one that answers
directly.

Numbers are one run on one input on one day; re-measure rather than trust them:

```sh
time (pbpaste | brevity --stdin --no-replace)
```

### Length

`BREVITY_MAX_WORDS` is a request in the prompt, not an enforced cap — a model
can and will overrun it on dense input. `BREVITY_MAX_TOKENS` is the real
ceiling, and hitting it truncates mid-sentence, so leave it well clear of the
length you actually want. To make summaries shorter, lower `BREVITY_MAX_WORDS`
or write a blunter prompt; don't squeeze `BREVITY_MAX_TOKENS`.

### Effort (Anthropic only)

`BREVITY_EFFORT` maps to Anthropic's `output_config.effort` and defaults to
`low`, which is right for summarizing. It is silently unused by every other
provider, and models older than Claude 4.6 reject it — unset it there.

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
brevity --allow-secrets summarize even if it looks like a credential
brevity --restore       put the replaced text back
brevity --install-hotkey [KEY]   bind a global hotkey
brevity --uninstall-hotkey       remove the bindings it wrote
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
- **Credentials are refused, not summarized.** If the clipboard looks like an API
  key, token, private key or JWT, brevity stops before sending anything. You copy
  a key, you forget, you hit the hotkey — and it would otherwise be in your
  provider's logs and your history file, neither of which you can undo. Pass
  `--allow-secrets` or set `BREVITY_ALLOW_SECRETS=true` when you mean it.
- **Failure is audible.** A falling two-tone plus a desktop notification, since
  there is no terminal to read. `BREVITY_NOTIFY=off|errors|always`.
- **Transient failures are retried.** A rate limit or a 5xx gets up to
  `BREVITY_RETRIES` more attempts with backoff, honouring the server's
  `Retry-After` when it sends one. Behind a hotkey a single 429 would otherwise
  just mean nothing happened. Timeouts are not retried — that request already
  spent the whole budget, and doubling a silent wait is worse than failing.
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

## Troubleshooting

Nothing appears on screen when brevity works, and not much appears when it
doesn't — see [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for the failure modes
worth knowing, starting with `brevity --config`.

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
