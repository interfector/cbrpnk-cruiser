# CBRPNK//CRUISER

A cyberpunk filesystem cruiser: fly through a real directory, inspect entries,
and launch programs without leaving the world.

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

## Controls

Keyboard is primary. Steering is joystick-style: mouse motion arms pointer
steering, and the reticle at screen center drives focus. Clicks never launch
anything.

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
| `Tab` | Toggle Lock mode |
| `Enter` | Enter directory, launch executable, open file, or enter `PARENT` |
| `Backspace` or `H` | Parent directory |
| `Home` or `~` | Home directory |
| `/` | Filter the current sector |
| `:` | Command console |
| `T` | Terminal at the current or selected directory |
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
its galaxy. Lock mode, freeze, filters, and open scans disable proximity entry;
use `Enter` while locked.

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
programs should be run with `term`.

## Configuration

Config lives at `$XDG_CONFIG_HOME/cbrpnk-cruiser/config.toml` (normally
`~/.config/cbrpnk-cruiser/config.toml`). Missing or invalid config falls back
to safe defaults with a warning.

```toml
start_path = "/home/you"
show_hidden = false
max_entries = 5000
font_scale = 1.0
crt = true
reduced_motion = false
terminal = "foot"      # optional terminal override
shell = "/bin/sh"      # used only by explicit ! commands
theme = "default"
log_level = "info"
```

`--no-crt` overrides `crt = true`.

## Safety and comfort

- Focus, hover, and scanning never launch programs. Launching is explicit.
- Filenames are passed as arguments, never interpolated into a shell.
- `run` requires the executable bit and reports the OS error on failure.
- Console commands capture stdout/stderr up to 64 KiB.
- `--no-crt` and `reduced_motion` disable post-processing and reduce motion.
- This interface has continuous camera movement. Stop if you feel discomfort.

## Known limitations

- Pointer steering is armed by mouse motion or a thrust click; idle pointers
  do not rotate the camera.
- Tab completion is not yet implemented in the console.
- `.desktop` files are opened through the system handler, not parsed.
- Windows is not yet a supported target.

## License

GPL-2.0-or-later. See `LICENSE`. Bundled Noto Sans Mono fonts are licensed
under the Apache License 2.0; see `assets/fonts/LICENSE`.
