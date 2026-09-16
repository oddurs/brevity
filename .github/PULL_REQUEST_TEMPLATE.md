## What

<!-- What changes, in a sentence or two. -->

## Why

<!-- The problem this solves. Link an issue with "Closes #123" if there is one. -->

## How it was tested

<!-- Commands you ran, providers you tried, platform you were on.
     CI has no clipboard, so if you touched src/clipboard.rs say which of
     pbcopy / wl-copy / xclip / xsel you tested and on what. -->

## Checklist

- [ ] `cargo fmt --all` and `cargo clippy --all-targets --all-features -- -D warnings` are clean
- [ ] `cargo test` passes
- [ ] New or changed `BREVITY_*` variables are documented in `.env.example` and `README.md`
- [ ] `CHANGELOG.md` has an entry under `## [Unreleased]`
- [ ] The PR title is a Conventional Commit (it becomes the commit on `main`)
