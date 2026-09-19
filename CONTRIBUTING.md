# Contributing to brevity

Thanks for looking. brevity is small on purpose — a single binary, two
dependencies, no daemon. Changes that keep it that way are the easiest to merge.

## Getting set up

```sh
git clone https://github.com/oddurs/brevity
cd brevity
cargo build
cargo test
```

Linux also needs a clipboard tool: `wl-clipboard` (Wayland) or `xclip` (X11).

### Work without spending money

`scripts/mock-provider.py` is a fake LLM endpoint that answers all three wire
shapes brevity speaks. Use it for anything that isn't about the model itself.

```sh
python3 scripts/mock-provider.py --echo &

BREVITY_PROVIDER=openai-compat \
BREVITY_BASE_URL=http://127.0.0.1:8799/v1 \
BREVITY_MODEL=mock \
  cargo run -- --stdin --no-replace <<< "some long text"
```

`--echo` prints the request headers and body, which is the fastest way to check
you are sending what a provider expects. `--status 429`, `--delay 10` and
`--reply "..."` cover the unhappy paths, and `--fail-times 2 --retry-after 3`
makes the server fail a few times before recovering, which is how the retry
logic is tested.

## The workflow

`main` is always releasable. Everything else is a short-lived branch and a pull
request — including mine.

```sh
git switch -c fix/wayland-empty-clipboard
# ...work...
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
git push -u origin fix/wayland-empty-clipboard
gh pr create --fill
```

**Branch names** are `type/short-description`, using the same types as the
commits: `feat/`, `fix/`, `docs/`, `refactor/`, `test/`, `chore/`, `ci/`.

**Pull request titles** follow [Conventional Commits], because PRs are squashed
and the title becomes the commit message on `main`:

```
feat(providers): add Cohere
fix(clipboard): survive an empty Wayland selection
docs: explain BREVITY_EFFORT
```

Add a `!` (`feat(config)!: rename BREVITY_STYLE`) for anything that changes
behaviour people have configured.

**Keep branches short.** Rebase onto `main` rather than merging it in:

```sh
git fetch origin && git rebase origin/main
```

**Merging** is squash-only; the branch is deleted automatically. `main` is
protected: it takes a pull request with CI green, and it keeps a linear history.

## What CI checks

Every PR runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` on
Linux and macOS, `cargo package`, and an end-to-end smoke test against the mock
provider. Run those four commands locally and CI will almost never surprise you.

Headless runners have no clipboard, so the `pbcopy`/`wl-copy`/`xclip` paths are
**not** covered by CI. If you touch `src/clipboard.rs`, say in the PR which
platform and which tool you tested by hand.

## Adding a provider

Most new backends are a three-line change, because anything that speaks
`POST /chat/completions` already works via `BREVITY_PROVIDER=openai-compat`.
Adding a named shortcut:

1. `src/config.rs` — add an arm to the `match provider_name` block with its
   default base URL and the environment variable its key lives in.
2. `.env.example` — document it where the other providers are listed.
3. `README.md` — one row in the provider table.

A genuinely new wire shape (something that is neither Anthropic-, OpenAI- nor
Gemini-shaped) is a new function in `src/provider.rs` and a new `Provider`
variant. Keep the request building in that function; everything else stays
shape-agnostic.

## Style

- Run `cargo fmt`. `rustfmt.toml` is the whole configuration.
- No new dependencies without a reason in the PR description. Two is a feature.
- Errors people will actually see should say what to do about it — name the
  environment variable, print the path.
- Never log, print, or `{:?}` an API key. `Config`'s `Debug` is hand-written to
  redact it; keep it that way.

## Licensing

By contributing you agree that your work is dual-licensed under MIT and
Apache-2.0, matching the project.

[Conventional Commits]: https://www.conventionalcommits.org/
