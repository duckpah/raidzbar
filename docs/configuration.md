# Configuration

raidzbar is configured with a single TOML file and targets **niri on Wayland only**.

[If you are a newbie then this is worth looking.](#notable-information-and-configuration-understandings)

## Config file

Lookup order - the app will look for config here (sort based on priority):

1. `$RAIDZBAR_CONFIG`
2. `$XDG_CONFIG_HOME/raidzbar/config.toml` (or `~/.config/raidzbar/config.toml`)
3. `config/config.toml` in the current directory or any parent
4. `config/config.toml` next to the executable or any parent

If none exist, the default config will be written to the XDG path and loaded.
The file is read once at startup; restart raidzbar after editing it.

Useful environment variables:

| Variable | Purpose |
| --- | --- |
| `RAIDZBAR_CONFIG` | Override the config path |
| `NIRI_SOCKET` | niri IPC socket, used by the `workspaces` and `window` modules |
| `RUST_LOG` | Log level, e.g. `RUST_LOG=debug` |

`NIRI_SOCKET` is normally set only when raidzbar is launched by niri. For example,
add this to niri's config:

```kdl
spawn-at-startup "raidzbar"
```

## Minimal example

```toml
[bar.main]
height = 30
groups_left = [["clock"]]
groups_center = [["window"]]
groups_right = [["network", "volume"]]

[module.clock]
type = "clock"
format = "%H:%M"

[module.window]
type = "window"

[module.network]
type = "network"

[module.volume]
type = "volume"
```

## Notable information and configuration understandings

Each `[bar.<name>]` creates an independent bar. You can have multiple bars in the same configuration file.
Modules name in groups_ are personal choice. You can basically name it whatever you want. But there must be a corresponding [module.<name>] with its type in it. "type" is fixed so if you name it wrong then will not be registered as a valid module.

```toml
[bar.skibidi]                            # this is the bar name, name it whatever you want
"bar options"                            # this is bar options, or easier words, it's the configuration for the bar
groups_left = [["skibidi", "random"]]    # modules that will stay in the left of the bar, name it whatever you want, this will also decide the order of modules, the "skibidi" modules will be to the far left and then "random"
groups_right = [["skibidi"], ["random"]] # if you want the modules' order to change dynamically, place them in 2 seperate tables
groups_center = []

[module.skibidi]                         # every modules you list out in [bar.<name>] must have a corresponding module section, or else it will be ignored
type = ""                                # this is the type of the module, this is fixed and cannot be changed, types of modules are listed below
"module options"                         # this is similar to bar options, just for modules, more information below


```

For custom modules, which means that modules with types that are not supported, you will want to create a script that run septomlate apps or commands.

## Bar options

| Key | Default | Notes |
| --- | --- | --- |
| `height` | `30` | Bar height in pixels; also used for the layer-shell exclusive zone |
| `position` | `"top"` | `"top"` or `"bottom"` |
| `background` | `"#1e1e2e"` | Bar surface color; `"#00000000"` makes it fully transparent |
| `foreground` | `"#cdd6f4"` | Default text color |
| `font` | `"monospace"` | Pango font family |
| `font_size` | `14.0` | Pango font size |
| `bold` | `false` | Render all text bold |
| `tebelowoffset` | `0.0` | Vertical text adjustment |
| `pabelow_left`, `padding_right` | `8.0` | Default module padding |
| `refresh_interval_ms` | `200` | Bar redraw/poll cadence |
| `groups_left`, `groups_center`, `groups_right` | none | Layout; each inner array is one pill |
| `modules_left`, `modules_center`, `modules_right` | none | Flat layout; each module gets its own pill |
| `margin_top`, `margin_bottom` | `0.0` | Pill inset from bar edges |
| `margin_left`, `margin_right` | `0.0` | Outer pill inset |
| `group_spacing` | `0.0` | Gap between pills |
| `corner_radius` | `0.0` | Pill corner radius |
| `module_background` | none | Shared pill background |
| `autohide` | `false` | Hide the bar when the pointer leaves |
| `autohide_delay_ms` | `400` | Delay before hiding |
| `autohide_show_delay_ms` | `0` | Delay before revealing |
| `autohide_reveal` | `3` | Pixels left visible while hidden |
| `autohide_zone_enabled` | `true` | Limit the reveal hot zone |
| `autohide_reveal_zone` | `0` | Hot-zone width; `0` means the whole edge |
| `autohide_zone_side` | `"right"` | `"left"` or `"right"` |

`groups_*` takes precedence over `modules_*` on the same side. Without
`module_background`, only the bar surface is visible behind modules.

## Colors, fonts, and spacing

Colors accept `#RRGGBB` or `#RRGGBBAA`. Invalid colors and sizes are logged and
fall back to defaults. Use a Nerd Font if your config uses glyph icons.

## Common module options

| Key | Notes |
| --- | --- |
| `type` | Required module type |
| `format` | Module-specific template (see below) |
| `foreground`, `background` | Override the bar defaults |
| `padding_left`, `padding_right` | Override module padding |
| `refresh_interval_ms` | Update interval; minimum 10 ms |
| `icon` | Single icon/label, where supported |

Default module interval is 1 second, except `window` and `workspaces` (100 ms).
`bluetooth` polls BlueZ every 2 seconds by default (`poll_interval_ms`), while its
redraw interval remains 1 second.

## Modules

| Type | Options | Notes |
| --- | --- | --- |
| `clock` | `format` | Local time via chrono; default `%H:%M:%S` |
| `custom` | `command` | Runs `sh -c <command>` and shows trimmed stdout |
| `cpu` | `icon` | Average CPU usage |
| `memory` | `format`, `icon` | RAM usage; binary units |
| `network` | `interface`, `format` | Throughput from `/proc/net/dev`; auto-detects the default interface |
| `temperature` | `sensor`, `format`, `icon`, `icon_low`, `icon_medium`, `icon_high`, `warn_threshold`, `critical_threshold`, `warn_color`, `critical_color`, `unavailable_color` | Picks the hottest matching hwmon sensor; thresholds default to 70/90 °C |
| `battery` | `device`, `icon_low`, `icon_medium`, `icon_high`, `icon_full` | Sysfs battery; auto-detects the first battery |
| `brightness` | `backend`, `device`, `show_slider`, `slider_width`, `max_brightness`, `scroll_step`, `icon_gap`, icons, glyphs, colors | `backend`: `auto`, `sysfs`, or `brightnessctl` |
| `volume` | `backend`, `target`, `device`, `show_slider`, `slider_width`, `max_volume`, `scroll_step`, `icon_gap`, icons, glyphs, colors | `backend`: `auto`, `wpctl`, or `pactl`; `target`: `sink` or `source` |
| `bluetooth` | `format`, `icon_on`, `icon_off`, `icon_no_controller`, `poll_interval_ms` | BlueZ over D-Bus |
| `playback` | `format`, `icon`, `icon_unavailable`, `icon_previous`, `icon_stop`, `icon_play`, `icon_pause`, `icon_next` | MPRIS over D-Bus |
| `workspaces` | `count`, `labels`, `active_color`, `occupied_color`, `empty_color` | niri workspaces; left-click focuses a workspace |
| `window` | `max_length`, `empty_label` | niri `FocusedWindow`; long titles are truncated with `…` |
| `tray` | `icon_size`, `icon_spacing` | StatusNotifierItem tray |

### Format tokens

| Module | Tokens |
| --- | --- |
| `memory` | `{icon}`, `{used}`, `{total}`, `{available}`, `{percent}` |
| `network` | `{interface}`, `{download}`, `{upload}` |
| `temperature` | `{icon}`, `{temp}`, `{max}`, `{critical}` |
| `bluetooth` | `{icon}`, `{status}` |
| `playback` | `{icon}`, `{title}`, `{artist}`, `{track}`, `{player}`, `{status}`, `{buttons}` |

`playback` buttons are clickable: previous, stop, play/pause, next. Place
`{buttons}` where the controls should appear; omitting it hides the controls.

### Sliders and clicks

`volume` and `brightness` show a slider by default (`show_slider = true`).
Defaults: `slider_width = 20`, `max_volume`/`max_brightness = 100`,
`scroll_step = 5`, `icon_gap = 1`. Slider glyphs can be changed with
`glyph_left`, `glyph_filled`, `glyph_empty`, and `glyph_right`.

- `volume`: click the icon to toggle mute; click the slider to set the level;
  scroll to nudge it. In compact mode (`show_slider = false`) the whole module
  toggles mute.
- `brightness`: click the slider to set the level; scroll to nudge it. Clicking
  the label does nothing.
- `tray`: left-click activates, right-click opens the item's DBusMenu inline,
  middle-click sends `SecondaryActivate`.

## Notable behavior

- The bar is a Wayland layer-shell surface; niri reserves space for it.
- Autohide is pointer-driven and leaves a reveal strip at the selected edge.
- Popups and tray menus are drawn by raidzbar itself and anchored to the bar.
- If another StatusNotifier watcher is already running, raidzbar still registers
  as a host and listens for items, but tray behavior may depend on that watcher.
- Multiple bars are supported, each with its own surface and modules.
- Missing external tools (for example `wpctl`, `pactl`, `brightnessctl`) are
  reported in the log; affected modules show an unavailable state or fall back
  to another backend.
