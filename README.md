# CBRPNK//CRUISER

A cyberpunk filesystem cruiser: fly through a real directory, inspect entries,
and launch programs without leaving the world. In-app PTY terminal windows let
you keep shells and TUI programs inside the cruiser.

CBRPNK//CRUISER is a **read-only** browser and launcher. It does not edit,
move, rename, or delete files. It is currently Linux-first.

## Build and run

Install the usual Rust and Linux graphics prerequisites. On Debian/Ubuntu:

```sh
sudo apt install build-essential pkg-config libx11-dev libxi-dev \
  libgl1-mesa-dev libasound2-dev
```

Then:

```sh
cargo run --release
cargo run --release -- /some/path
cargo run --release -- --no-crt
```

CLI options:

```text
cbrpnk-cruiser [PATH]
  --config <PATH>   load an explicit config file
  --no-crt          disable CRT/bloom post-processing
  --hidden          show hidden files at startup
  -h, --help        print help
  -V, --version     print version
```

Checks:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```

## Install as a desktop session

CBRPNK//CRUISER can appear in the display manager's session menu next to
AwesomeWM, Sway, and other desktops. It is not an X11 window manager and does
not manage other applications' top-level windows. It does manage its own
in-app terminal windows. Selecting this session launches the cruiser directly
as the session application in `$HOME`, sized to the full display.

The session is an X11 session because the current Linux backend is X11.

```sh
cargo build --release
sudo SKIP_BUILD=1 packaging/install-session.sh
```

The installer writes:

```text
/usr/local/bin/cbrpnk-cruiser
/usr/local/bin/cbrpnk-cruiser-session
/usr/share/xsessions/cbrpnk-cruiser.desktop
```

Log out, choose **CBRPNK//CRUISER** in the session menu, then log in. To
uninstall, remove those three files.

Use `PREFIX` for a different install prefix and `DESTDIR` for packaging:

```sh
sudo env PREFIX=/usr SKIP_BUILD=1 packaging/install-session.sh
DESTDIR=/tmp/pkg SKIP_BUILD=1 packaging/install-session.sh
```

If you omit `SKIP_BUILD=1`, the installer runs `cargo build --release` itself.

## The sector

Real directories become galaxies, sized by their immediate child count.
Positions are derived from each entry's path, so the world is stable between
frames and rescans but does not force everything onto one fixed orbit.
Symlinks curve continuously toward their targets; links into a sibling
directory's contents arc to that galaxy.

## Controls

Keyboard is primary. Steering is joystick-style: mouse motion arms pointer
steering, and the reticle at screen center drives focus. Clicks never launch
anything. By default `auto_hide_mouse = true` hides the pointer while you
cruise; mouse movement, steering, and clicks still work. The pointer reappears
while an in-app terminal window has focus. Set `auto_hide_mouse = false` to
show it at all times.

| Input | Action |
| --- | --- |
| Arrow keys | Aim the reticle; in Lock, aim/cycle |
| `W` / `S` | Thrust forward / reverse |
| `A` / `D` | Strafe |
| `Q` / `E` | Move down / up |
| `Space` or left mouse | Thrust forward |
| Right mouse | Thrust reverse |
| Mouse position | Steer (pointer is a joystick; deadzone at center) |
| `Shift` | Boost |
| `Ctrl` | Brake |
| `J` / `K` | Previous / next node |
| `Tab` | Toggle Lock mode; complete input in Console |
| `Ctrl+Up` / `Ctrl+Down` | Adjust Console height |
| `Enter` | Enter directory, launch executable, open file, or enter `PARENT` |
| `Ctrl+P` | Open fuzzy quick switcher |
| `Ctrl+T` | Open a shell in an in-app terminal window |
| `Ctrl+Enter` | Run selection in a terminal window; directory opens a shell there |
| `F6` | Cycle terminal-window focus |
| `F7` | Keyboard window layout mode for the focused terminal |
| `F12` | Release terminal focus |
| `Shift+F12` | Close the focused terminal window |
| `Shift+PageUp` / `PageDown` | Scroll the focused terminal's output |
| `Backspace` or `H` | Parent directory |
| `Home` or `~` | Home directory |
| `/` | Filter the current sector |
| `:` | Command console |
| `O` | Open with the system handler |
| `R` | Recenter camera |
| `F` | Freeze motion |
| `F5` | Rescan |
| `Ctrl+H` | Toggle hidden files |
| `F1` or `?` | Help overlay |
| `F3` | Debug overlay |
| `F11` or `Alt+Enter` | Fullscreen |
| `Esc` | Close overlay, release lock, or quit |
| `Ctrl+Q` | Quit |

The `PARENT` gate above the sector is selectable when the current directory
has a parent. It is not shown at `/`.

Unlocked Cruise automatically enters a readable directory when you fly inside
its galaxy, and returns to the parent when you fly into the `PARENT` gate. Lock
mode, freeze, filters, and open scans disable proximity entry; use `Enter`
while locked. Set `auto_enter = false` to require `Enter` for both actions, or
lower `auto_enter_factor` to make directory entry stricter.

## Console

The console is a quick launcher, not a shell replacement. Commands are parsed
into argv without a shell by default.

```text
cd [path]        change sector
ls [path]        compact listing
open <path>      system handler
run <path> ...   execute directly (must be executable)
term [dir]       external terminal
reload           rescan
home / root      jump
help             console help
quit             exit
```

A leading `!` explicitly opts into shell execution:

```text
!ls -la | less
```

Output from console commands is captured and shown in the console. Interactive
programs should be run with `term`, or open a shell window with `Ctrl+T`.
`Tab` completes built-ins and the first path argument of `cd`, `ls`, `open`,
`run`, and `term`; multiple matches are listed in the console. Use `Ctrl+Up`
and `Ctrl+Down` to resize the console.

## Configuration

Config lives at `$XDG_CONFIG_HOME/cbrpnk-cruiser/config.toml` (normally
`~/.config/cbrpnk-cruiser/config.toml`). Missing or invalid config falls back
to safe defaults with a warning.

```toml
start_path = "/home/you"
show_hidden = false
max_entries = 5000
console_lines = 12       # initial Console height
console_min_lines = 4    # Ctrl+Down lower bound
console_max_lines = 24   # Ctrl+Up upper bound, raise for a taller Console
font_scale = 1.0
unfocused_opacity = 0.70 # unfocused terminal window opacity (0.0-1.0)
auto_enter = true        # false disables proximity entry; use Enter
auto_enter_factor = 0.5  # directory entry radius, fraction of galaxy radius
auto_hide_mouse = true   # hide pointer except when a terminal is focused
open_with_system = true  # false masks O and Enter's system-open action
enter_shell = false      # true makes Enter open an in-app shell
crt = true
reduced_motion = false
terminal = "foot"      # optional terminal override
shell = "/bin/sh"      # Ctrl+T shell and explicit ! commands
theme = "default"
log_level = "info"
```

`--no-crt` overrides `crt = true`. The hyphenated
`auto-hide-mouse = true` spelling is also accepted. `unfocused_opacity`
applies to unfocused in-app terminal windows and their text.

`auto_enter = false` disables automatic proximity navigation for directories
and the parent gate. `auto_enter_factor` is clamped to 0.0-1.0; it scales the
directory entry radius relative to the galaxy radius. A factor of `0.0`
disables directory entry while leaving the parent gate active.

Set `open_with_system = false` to mask `O` and Enter's system-open action for
regular files. Set `enter_shell = true` to make Enter open an in-app shell
instead: directories open there, while files and executables open a shell in
their containing directory. The explicit `open` console command is unchanged.

## Safety and comfort

- Focus, hover, and scanning never launch programs. Launching is explicit.
- Filenames are passed as arguments, never interpolated into a shell.
- `run` requires the executable bit and reports the OS error on failure.
- Console commands capture stdout/stderr up to 64 KiB.
- In-app terminal windows run real PTY children; closing a window kills the
  child. Terminal focus always has an explicit `F12` escape. Unfocused
  terminal windows fade to the configured `unfocused_opacity`.
- `--no-crt` and `reduced_motion` disable post-processing and reduce motion.
- This interface has continuous camera movement. Stop if you feel discomfort.

## Known limitations

- Pointer steering is armed by mouse motion or a thrust click; idle pointers
  do not rotate the camera.
- External command names and arguments are not tab-completed; completion
  covers built-ins and their first path argument.
- `.desktop` files are opened through the system handler, not parsed.
- In-app terminal windows track application cursor mode for TUI navigation,
  but do not support PTY mouse reporting, clipboard integration, or arbitrary
  GUI embedding.
- Windows is not yet a supported target; terminal windows are Linux-tested.

## License

GPL-2.0-or-later. See `LICENSE`. Bundled Noto Sans Mono fonts are licensed
under the Apache License 2.0; see `assets/fonts/LICENSE`.
