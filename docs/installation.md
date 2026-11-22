# Installation

`raidzbar` targets **niri on Wayland**. It does not support X11 or other
compositors.

[Worth looking information](#diagnostics)

## Requirements

- Linux with niri running
- Rust 1.85+ (edition 2024) and Cargo
- Cairo, Pango, Wayland, and xkbcommon development files
- A Nerd Font (optional)

Most of the packages mentioned here are already installed along with niri. So technically you only need rust and pkgconf. Feel free to uninstall them after finished building.

### Arch Linux

```sh
sudo pacman -S --needed base-devel rust pkgconf cairo pango wayland libxkbcommon
```

### Debian / Ubuntu

```sh
sudo apt install build-essential pkg-config libcairo2-dev libpango1.0-dev \
  libwayland-dev libxkbcommon-dev
```

### Fedora

```sh
sudo dnf install gcc make pkgconf-pkg-config cairo-devel pango-devel \
  wayland-devel libxkbcommon-devel
```

Install Rust with [rustup](https://rustup.rs/) if your distro package is too old.

## Build and install

Local build - for your user only:

```sh
cd ./raidzbar

cargo build --release
install -Dm755 target/release/raidzbar ~/.local/bin/raidzbar
```

Alternatively, let Cargo install it:

```sh
cargo install --path .
```

If you want to install it system-wide:

```sh
cargo build --release
sudo install -Dm755 target/release/raidzbar /usr/bin/raidzbar
```

## Configure

On first launch, raidzbar writes the default config to:

```text
$XDG_CONFIG_HOME/raidzbar/config.toml
```

or `~/.config/raidzbar/config.toml` when `XDG_CONFIG_HOME` is unset.

Use `RAIDZBAR_CONFIG` to point at a different file. See
[configuration.md](configuration.md) for all options.

## Start with niri

Add this to your niri config:

```kdl
spawn-at-startup "raidzbar"
```

Use the absolute path if you setup it locally:

```kdl
spawn-at-startup "/home/<username>/.local/bin/raidzbar"
```

You can also run it manually from a niri session:

```sh
raidzbar
```
## Diagnostics

You this to see the logs of raidzbar

```sh
RUST_LOG=debug raidzbar
```

If you want to disable warnings and only show errors, use this:

```sh
RUST_LOG=error raidzbar
```

Add this to niri config if you also want to disable warnings when you **auto startup raidzbar**:

```sh
environment {
  RUST_LOG "error"
}
```

## Runtime tools

Only install what you need for the modules you use:

| Feature | Requirement |
| --- | --- |
| Workspaces / window title | niri and `$NIRI_SOCKET` (set automatically when launched by niri) |
| Volume | `wpctl` (PipeWire) or `pactl` (PulseAudio) |
| Brightness | `/sys/class/backlight` access or `brightnessctl` |
| Temperature | Kernel hwmon sensors |
| Battery | `/sys/class/power_supply` |
| Bluetooth | BlueZ over D-Bus |
| Media playback | An MPRIS-compatible player and a session D-Bus |
| System tray | A session D-Bus and StatusNotifierItem-capable applications |
