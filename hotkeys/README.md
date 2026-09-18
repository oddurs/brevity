# Binding brevity to a key

Most people want:

```sh
brevity --install-hotkey
```

which does all of this for you on macOS, GNOME, sway, Hyprland and i3. The
recipes below are for KDE, for the tools it does not automate, and for anyone
who would rather write the config themselves.

Every recipe runs the same thing: the `brevity` binary, with no arguments, with
no terminal. Use the absolute path (`~/.local/bin/brevity`) - hotkey daemons
rarely inherit your shell's `PATH`.

## macOS

### Shortcuts.app (no extra software)

1. Shortcuts → File → New Shortcut, name it **Brevity**.
2. Add the action **Run Shell Script**.
   - Shell: `/bin/sh`
   - Script: `$HOME/.local/bin/brevity`
   - Turn **Pass Input** to *nothing* (brevity reads the clipboard itself).
3. In the shortcut's details pane (ⓘ), click **Add Keyboard Shortcut** and press
   your combination. `⌃⌥⌘B` is usually free.

### Raycast

Copy `raycast-brevity.sh` into your Raycast script-commands directory, then give
it a hotkey in Raycast → Extensions.

### Hammerspoon

Append `hammerspoon.lua` to `~/.hammerspoon/init.lua` and reload the config.

### skhd

Add the line in `skhdrc` to `~/.config/skhd/skhdrc`, then `skhd --restart-service`.

## Linux

### GNOME

Run `./gnome.sh` once. It registers `<Ctrl><Alt>b` as a custom shortcut.

### KDE Plasma

System Settings → Shortcuts → Add New → Command/URL, command
`/home/you/.local/bin/brevity`, then assign a key.

### Sway / Hyprland / i3

Copy the matching line out of `wm.conf` into your window-manager config.

---

Nothing appears on screen while it works. You get the chime when the summary is
on your clipboard; a lower double-tone plus a desktop notification if something
went wrong. `brevity --restore` puts the original text back.
