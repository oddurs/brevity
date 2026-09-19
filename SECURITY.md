# Security

## Reporting a vulnerability

Use [private vulnerability reporting][advisory] on this repository, or email
<oddurs@gmail.com>. Please don't open a public issue for a vulnerability.

Expect an acknowledgement within a week. brevity is a hobby-scale project run by
one person; there is no paid support and no bounty, but genuine reports get
genuine attention and credit in the release notes if you'd like it.

The latest release is the only supported version.

[advisory]: https://github.com/oddurs/brevity/security/advisories/new

## What brevity does with your data

Worth understanding before you bind it to a key, because the whole point of the
tool is to send your clipboard somewhere.

**Your clipboard text goes to the provider you configured.** The entire selection,
in one request, over HTTPS (or plain HTTP if you point `BREVITY_BASE_URL` at
something local). If that text is a password, a private key, a customer record or
an unpublished document, it has left your machine and is subject to that
provider's retention and training policy. Point `BREVITY_PROVIDER` at Ollama,
LM Studio, llama.cpp or vLLM to keep everything on the machine.

**Credentials on the clipboard are refused by default.** brevity checks for API
key, token, private key and JWT shapes before it sends anything, because the
mistake is easy to make and impossible to take back: copy a key, forget, press
the hotkey, and it is in your provider's logs and your history file. The check is
deliberately high-precision rather than exhaustive — it will not catch a password
or an unrecognized token format, so it is a safety net, not a guarantee.
`BREVITY_ALLOW_SECRETS=true` or `--allow-secrets` disables it.

**brevity talks to exactly one host** — the one `BREVITY_BASE_URL` resolves to, or
the configured provider's default. It contacts nothing else: no telemetry, no
update check, no analytics.

**Files it writes, all owned by you:**

| Path | Mode | Contents |
|---|---|---|
| `~/.config/brevity/.env` | `0600` | API keys |
| `~/.cache/brevity/last-original.txt` | `0600` | the text the last run replaced |
| `~/.cache/brevity/*.wav` | `0644` | the generated chime |

The history file exists so `brevity --restore` can undo a replacement, and it
holds whatever was on the clipboard — set `BREVITY_HISTORY=false` to stop
writing it. Nothing is ever deleted from it other than by being overwritten on
the next run.

**The environment file is read, never written** (except by `--init`, which only
creates it when absent). Real environment variables override it, so you can keep
keys in a password manager and export them in your shell instead.

## Hardening notes

- `~/.config/brevity/.env` is created `0600`. If you created it by hand, check
  it: `chmod 600 ~/.config/brevity/.env`.
- Keys never reach stdout, stderr or a notification. `brevity --config` prints
  only the first six and last four characters.
- brevity shells out to clipboard tools and audio players found on `PATH`. If an
  attacker controls your `PATH`, they control those; that is true of any program,
  but it is worth knowing that a hotkey daemon may use a different `PATH` than
  your shell.
- Untrusted clipboard text is only ever sent as a prompt, never executed or
  interpolated into a shell command. Prompt-injected text can, however, make the
  model return something misleading — the summary is model output, not a fact.
