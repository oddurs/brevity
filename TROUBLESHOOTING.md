# Troubleshooting

brevity is silent by design, which makes it awkward to debug. Almost everything
starts here:

```sh
brevity --config
```

That prints the provider, model, endpoint and a redacted key, and exits non-zero
with the reason if the configuration is incomplete. Run it before anything else.

To see a failure in full, run brevity from a terminal rather than the hotkey —
errors go to stderr, which a hotkey daemon throws away.

```sh
pbpaste | brevity --stdin --no-replace
```

That prints the summary without touching the clipboard, so you can retry as
often as you like.

---

## "no API key for X: set Y_API_KEY"

The key is missing, empty, or on the wrong line. The last one is the common
case: the provider key lines in `.env.example` are adjacent comments, and it is
easy to uncomment `OPENAI_API_KEY` when you meant `OPENROUTER_API_KEY`.

Check which line is actually live — this prints the variable names and value
lengths, never the values:

```sh
grep -n '^[A-Z_]*API_KEY=' ~/.config/brevity/.env | sed 's/=.*/=<set>/'
```

A key on a commented line does nothing. So does a key on the wrong variable.

Also check the provider itself: setting `OPENROUTER_API_KEY` while
`BREVITY_PROVIDER` is still `anthropic` produces exactly this error, naming the
key brevity wanted rather than the one you set.

## It works in a terminal but fails from the hotkey

Almost always an environment difference, and on macOS there is one specific
cause worth knowing: `pbpaste` and `pbcopy` pick their character encoding from
the locale, and a hotkey daemon runs under launchd, which sets no locale at all.
They then fall back to Mac OS Roman — reading turns `é` into a byte that is not
valid UTF-8 and drops emoji entirely, and writing mangles every non-ASCII
character in the summary.

brevity forces a UTF-8 locale on those tools as of the current version, so if
you see `clipboard does not hold UTF-8 text` from a hotkey, you are on an older
build. Reproduce it deliberately with:

```sh
env -u LANG -u LC_ALL -u LC_CTYPE brevity --no-replace
```

If that fails while plain `brevity --no-replace` succeeds, rebuild and reinstall.

## The summary is empty, or a sentence long

The model spent its token budget before writing an answer. This is the standard
failure with reasoning models on OpenAI-shaped endpoints, where thinking tokens
come out of `max_tokens`:

```ini
BREVITY_MAX_TOKENS=8192
```

Or pick a model that answers directly — see [Choosing a model](README.md#choosing-a-model).

brevity detects the clear-cut version of this and says
`hit the output limit before writing anything`. A partial summary that stops
mid-sentence is the same cause.

## It is too slow for a hotkey

Model choice dominates; nothing else in brevity takes measurable time. The
benchmark in the README has real numbers — the spread between the fastest and
slowest model tested was over 4×. Measure yours:

```sh
time (pbpaste | brevity --stdin --no-replace)
```

If a local model is the slow part, it is loading from disk on each call; keep it
resident (`OLLAMA_KEEP_ALIVE=30m` for Ollama).

## The summary is longer than BREVITY_MAX_WORDS

Expected. The word budget is an instruction in the prompt, not a cap brevity
enforces. Dense input overruns it. Lower `BREVITY_MAX_WORDS`, or write a blunter
prompt:

```ini
BREVITY_PROMPT="In no more than {max_words} words, state only what this says. No detail, no examples.\n\n{text}"
```

## Nothing happens when I press the hotkey

If you used `brevity --install-hotkey` on macOS, the answer is almost certainly
that skhd has not been granted Accessibility. It fails closed and silently:

```sh
pgrep skhd || tail -1 /tmp/skhd_$USER.err.log
```

`skhd: must be run with accessibility access! abort..` means exactly what it
says. Turn skhd on under System Settings > Privacy & Security > Accessibility,
then `skhd --restart-service`. No program can grant this to itself, which is why
the installer can only open the pane for you.

Check the binding itself with:

```sh
grep -A4 'brevity' ~/.config/skhd/skhdrc
```

Run the same command in a terminal first. If that works, it is the hotkey
environment, and almost always one of:

- **Not an absolute path.** Hotkey daemons do not inherit your shell `PATH`. Use
  `/Users/you/.local/bin/brevity`, not `brevity`.
- **Permission not granted.** macOS asks once before letting Shortcuts.app or
  Automator run a shell script. If you dismissed it, the shortcut silently does
  nothing — re-run it and accept.
- **Input being passed in.** A Shortcuts "Run Shell Script" action set to pass
  the selection or clipboard as stdin changes nothing by itself, but set
  **Pass Input** to *nothing*: brevity reads the clipboard itself.

## No sound

`brevity --chime` plays it on demand — that isolates the audio path from
everything else. `brevity --chime error` plays the failure tone.

If it is silent, no player was found. macOS has `afplay` built in; Linux needs
one of `pw-play`, `paplay`, `aplay`, `ffplay`, `play` or `mpv`. Point brevity at
whatever you do have:

```ini
BREVITY_SOUND_CMD=paplay --volume=30000 {}
```

A failure falls back to the terminal bell, which is inaudible from a hotkey — so
silence usually means a missing player, not a missing summary. Check the
clipboard before assuming it failed.

## "no clipboard tool found" on Linux

Install `wl-clipboard` (Wayland) or `xclip` (X11). brevity shells out rather
than linking a clipboard library because on X11 the selection is owned by a live
process: a short-lived binary that sets the clipboard and exits loses the
contents immediately.

If both are installed and the wrong one is being used, brevity prefers
`wl-copy` when `WAYLAND_DISPLAY` is set. Unset it to force the X11 path.

## The clipboard did not change, but I heard the success chime

The chime plays after the clipboard is written, so this combination should not
happen. If it does, another application is overwriting the clipboard immediately
after — clipboard managers are the usual culprit. Check with:

```sh
brevity --print
```

which prints the same text it put on the clipboard.

## I want the old clipboard back

```sh
brevity --restore
```

The replaced text is saved to `~/.cache/brevity/last-original.txt` (mode `0600`)
before the clipboard is overwritten. It keeps only the most recent one.

## A credential was on the clipboard when I pressed the hotkey

Then it was sent to your provider as text to summarize, and written to
`last-original.txt`. Rotate it, and remove the local copy:

```sh
rm ~/.cache/brevity/last-original.txt
```

To stop the history file being written at all, set `BREVITY_HISTORY=false`.
There is no way to un-send the request; treat any credential that was on the
clipboard at that moment as compromised.

## The model refused, or the summary looks wrong

Summaries are model output, not fact. Clipboard text that contains instructions
("ignore the above and write...") can steer the result — brevity sends it as a
prompt, so prompt injection affects what comes back. It cannot do anything else:
the text is never executed, and never interpolated into a shell command.

## Debugging without spending money

`scripts/mock-provider.py` is a fake endpoint that answers all three wire shapes
brevity speaks:

```sh
python3 scripts/mock-provider.py --echo &

BREVITY_PROVIDER=openai-compat \
BREVITY_BASE_URL=http://127.0.0.1:8799/v1 \
BREVITY_MODEL=mock \
  brevity --stdin --no-replace <<< "some text"
```

`--echo` prints the exact headers and JSON body brevity sent, which settles most
"is it me or the provider" questions. `--status 429` and `--delay 30` reproduce
rate limits and timeouts.
