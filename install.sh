#!/usr/bin/env sh
# Build brevity and put it on your PATH.
#   ./install.sh              -> installs to ~/.local/bin
#   PREFIX=/usr/local ./install.sh
set -eu

PREFIX="${PREFIX:-$HOME/.local}"
BIN="$PREFIX/bin"
SRC_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

command -v cargo >/dev/null 2>&1 || {
  echo "brevity: cargo not found. Install Rust: https://rustup.rs" >&2
  exit 1
}

echo "==> building"
( cd "$SRC_DIR" && cargo build --release )

echo "==> installing to $BIN"
mkdir -p "$BIN"
install -m 0755 "$SRC_DIR/target/release/brevity" "$BIN/brevity"

echo "==> config"
"$BIN/brevity" --init || true

# Linux needs an external clipboard tool; macOS has pbcopy/pbpaste built in.
if [ "$(uname -s)" != "Darwin" ]; then
  if ! command -v wl-copy >/dev/null 2>&1 && ! command -v xclip >/dev/null 2>&1 && ! command -v xsel >/dev/null 2>&1; then
    echo "!! no clipboard tool found. Install one:" >&2
    echo "     Wayland: sudo apt install wl-clipboard   (or dnf/pacman)" >&2
    echo "     X11:     sudo apt install xclip" >&2
  fi
  if ! command -v notify-send >/dev/null 2>&1; then
    echo "   (optional) install libnotify-bin for desktop notifications"
  fi
fi

case ":$PATH:" in
  *":$BIN:"*) ;;
  *) echo "!! $BIN is not on your PATH. Add it:"
     echo "   fish:  fish_add_path $BIN"
     echo "   bash:  echo 'export PATH=\"$BIN:\$PATH\"' >> ~/.bashrc" ;;
esac

cat <<EOF

installed: $BIN/brevity

next:
  1. brevity --edit             add an API key (or point it at a local model)
  2. brevity --config           check what it resolved
  3. copy some text, then run:  brevity
  4. brevity --install-hotkey   bind it to a key
EOF
