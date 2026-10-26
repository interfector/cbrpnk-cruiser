# AGENTS.md - CBRPNK//CRUISER

This file is the operating manual for anyone (human or AI agent) working on
this repository. Read it before changing code. Keep it current when the
architecture, controls, or scope change.

## 0. Read this first

- The product is a **real file browser and shell launcher** with a Gibson
  mainframe / Tron visual language. It is not a screensaver, a fake terminal,
  or a tech demo that cannot launch anything.
- The reference implementation is in `xcruiser-0.30/`. It is read-only
  reference material. Do not edit it, build it, or copy source from it.
- The reference is GPL software. This project is also
  **GPL-2.0-or-later**. See section 12.
- Usability beats cinematography. If a visual effect makes it harder to find
  or launch a file, remove the effect.
- When in doubt: small, deterministic, testable changes. No new framework,
  no async runtime, no web view, and no file mutation without human approval.

---

## 1. Project snapshot

| Item | Decision |
| --- | --- |
| Working name | **CBRPNK//CRUISER** |
| Binary | `cbrpnk-cruiser` |
| Language | Rust (stable, edition 2021) |
| Rendering | `macroquad` + custom shaders |
| Platform priority | Linux first; macOS best effort; Windows later |
| Input priority | Keyboard-first, mouse-assisted |
| Project state | Greenfield; reference only |
| License | GPL-2.0-or-later |

The application starts in a directory, turns that directory into a navigable
"sector" in a neon-grid environment, and lets the user inspect entries and
launch programs. It must work on a normal filesystem, including `/`, `$HOME`,
mount points, symlinks, permission errors, hidden files, and non-UTF-8 names.

---

## 2. Product definition

### 2.1 One-sentence pitch

A cyberpunk mainframe interface that lets you fly through your actual
filesystem, inspect what is there, and launch programs without leaving the
world.

### 2.2 Core loop

1. Boot into a short, skippable mainframe sequence.
2. Land in the current directory (or a path supplied on the command line).
3. The directory's entries appear as nodes arranged in a stable spatial
   layout.
4. The user steers the camera, locks onto a node, and reads its details.
5. `Enter` enters a directory, launches an executable, or opens a regular
   file with the system handler.
6. `:` opens a command console for quick built-ins and program launches.
7. `T` opens an external terminal; `Ctrl+T` opens a real PTY shell in an
   in-app window.
8. The user can go back to the parent, jump home, filter, rescan, or quit.

### 2.3 Success criteria

- A new user can find `$HOME`, enter a subdirectory, select a script, and run
  it within 30 seconds without reading documentation.
- The displayed entries are the real directory entries, not mock data.
- Symlink loops, permission errors, huge directories, and broken links do not
  hang or crash the app.
- Launching a program or opening a file produces visible success or a useful
  error message.
- The app holds 60 FPS at 1080p on integrated graphics with default effects.
- Every action is reachable from the keyboard.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` pass.

### 2.4 Non-goals for v1

Do not implement these unless the human explicitly expands the scope:

- File editing, move/copy/delete/rename, or trash management.
- Embedding arbitrary GUI applications inside the world.
- Recursive indexing of the entire disk at startup.
- Network access, cloud sync, telemetry, accounts, or plugins.
- Thumbnails, media playback, or an embedded browser.
- A game engine (Bevy, Fyrox, Godot), an immediate-mode GUI framework (egui,
  iced), or a web front end.
- Windows-specific polish before Linux is solid.

The app is **read-only** in v1: browse, inspect, launch, and open. An in-app
shell can do whatever the user explicitly types into it, but the cruiser itself
never mutates files. If built-in file mutation is added later, it must be a
separate, confirmed feature with its own tests and safety design.

---

## 3. Visual direction

### 3.1 Influences, translated

| Influence | What to take | What to avoid |
| --- | --- | --- |
| Gibson mainframe (Hackers) | Near-black void, phosphor green readouts, wireframe skyline, boot/status text, "live system" feeling | Fake hacking text, movie quotes, cluttered ASCII noise |
| Tron | Cyan circuit lines, concentric rings, black glass, identity-disc reticle, light trails | Literal film assets, logos, costume colors everywhere |
| XCruiser | Filesystem-as-space, directories as rings, files as bodies, links as connections, free-flight navigation | Fixed 256-entry limits, blocking scans, fragile C string handling |

Keep the palette restrained: about 80% dark negative space and 20% signal
color. Glow is a highlight, not a texture. Labels must remain readable at all
times.

Use original names and artwork. Do not put "Gibson" or "Tron" trademarks in
the app title, UI, or marketing copy. "Mainframe" and "grid" are generic
descriptions; use those.

### 3.2 Palette

Centralize every color in `src/theme.rs`. No hex literals outside that file
(except test fixtures).

| Role | Hex | Use |
| --- | --- | --- |
| Void | `#010409` | Background, empty space |
| Deep panel | `#061018` | HUD panels, console background |
| Grid dim | `#0B2B33` | Background grid, distant structure |
| Cyan primary | `#00E5FF` | Primary UI, current sector, focus |
| Cyan dim | `#007A8A` | Secondary lines, inactive nodes |
| Phosphor green | `#39FF14` | System status, successful actions, sparingly |
| Amber | `#FFB000` | Warnings, executables, "deploy" actions |
| Magenta | `#FF2A6D` | Symlinks, errors, destructive-adjacent states |
| Ice white | `#E6FFFF` | Selected text, high-emphasis labels |
| Text muted | `#7FA6AD` | Secondary text, metadata |
| Fault red | `#FF3B30` | Hard errors only |

Do not use saturated neon for large filled areas. Lines, glyphs, and text
carry the color; the background stays dark.

### 3.3 Shape language

Give each entry kind a shape as well as a color. Color must never be the only
signal.

- **Directory / sector**: a small multi-orbit galaxy with a segmented core,
  cyan; proximity to the galaxy enters the sector.
- **Executable / program**: a diamond or chevron core, amber.
- **Regular file / data**: a square shard, cyan-white; size controls scale.
- **Symlink / portal**: an arc or wormhole curve, magenta; when the resolved
  target is another visible node in the sector, draw a curved arc to it. If the
  target is nested inside a visible directory instead, draw a continuous
  cross-galaxy arc to that directory's galaxy.
- **Special file / unknown**: a broken ring or question glyph, muted.
- **Locked selection**: four corner brackets plus a rotating outer ring.
- **Reticle**: a small identity-disc-like double arc with a center dot.

Labels are monospace, uppercase headers, sentence-case names as returned by
the filesystem (do not uppercase filenames). Billboard labels face the
camera and fade with distance. Hide labels that are behind the camera or
outside the viewport. Cap visible labels for performance.

### 3.4 Rendering language

- Draw the world with perspective projection on a dark void.
- Background: a faint circuit-grid floor and a low-contrast procedural
  wireframe skyline. The skyline seed must come from the current path, not
  from per-frame randomness.
- Foreground: sector rings, node glyphs, portal curves, motion trails.
- Use depth sorting for nodes. Distant nodes become smaller and dimmer; very
  distant nodes can become simple dots.
- Lines are 1-2 px at native resolution. The glow comes from a bloom pass,
  not from drawing thick lines.
- Text is rendered in screen space after the 3D pass, so labels and HUD stay
  crisp.
- Post-processing: bloom, subtle scanlines, vignette, and optional faint
  barrel/chromatic distortion. Scanline alpha should be around `0.05`, not a
  visible striped filter. Keep text legible.
- `--no-crt` and the config option must disable all post-processing while
  leaving the scene and palette intact.

### 3.5 Motion

- Boot sequence: 1.0-1.5 seconds maximum, skippable with any key or click.
- Sector transition: 180-260 ms, ease-out. No bounce, no excessive shake.
- Camera: damped, no roll by default. Respect a reduced-motion setting.
- Do not flash faster than 3 Hz. Do not use strobing as a warning.
- Freeze key and a "reduced motion" option are mandatory.
- Motion sickness warning belongs in the README and help overlay, as a nod to
  the reference. Keep it brief.

### 3.6 HUD

The HUD should feel like a mainframe console, not a dashboard.

- **Top left**: breadcrumb / current path, sector name, entry counts.
- **Top right**: host name, local time, scan status, and optionally FPS in
  debug builds. Do not show fake CPU percentages.
- **Bottom left**: selection card: name, kind, size, modified time,
  permissions, symlink target, executable status.
- **Bottom center**: context hint line, or the command input when open.
- **Bottom right**: compact controls hint (`?` for full help).
- **Center**: reticle and lock brackets.
- **Console overlay**: bottom-anchored, up to 12 visible lines, scrollable,
  with the command prompt on the last line.

Use plain language in hints. Stylized uppercase is fine for headings. Avoid
fake logs scrolling for decoration; if logs appear, they should be real
application events.

---

## 4. Interaction specification

### 4.1 Modes

| Mode | Purpose |
| --- | --- |
| `Boot` | Short startup sequence; any input skips |
| `Cruise` | Default navigation; steer the camera, focus follows the reticle |
| `Lock` | Selection is locked; cycle nodes and inspect without flying away |
| `Console` | Command input and output overlay |
| `Help` | Full controls and configuration help |
| `Confirm` | Reserved for future destructive actions; do not use for normal launches |

Only one overlay is open at a time. `Esc` closes the top overlay, then
releases lock, then opens the quit prompt or quits (choose one consistent
behavior and document it).

### 4.2 Default controls

Keyboard is the primary path. Mouse is an accelerator. Punctuation shortcuts
must use the produced character (`:`, `?`, `/`, `~`) as well as physical key
codes so non-US keyboard layouts work.

| Input | Action |
| --- | --- |
| Arrow keys | Steer / aim the reticle; in Lock, cycle focus |
| `W` / `S` | Thrust forward / reverse |
| `A` / `D` | Strafe left / right |
| `Q` / `E` | Move down / up (optional, if the layout benefits) |
| `Space` or left mouse | Thrust forward |
| Right mouse | Thrust reverse |
| `Shift` | Boost while thrusting |
| `Ctrl` | Brake |
| Mouse position | Steer relative to center in Cruise (joystick style) |
| Mouse wheel | Adjust cruise speed, not camera FOV |
| `Tab` | Toggle Lock mode / cycle focus; completes input in Console |
| `Ctrl+Up` / `Ctrl+Down` | Increase / decrease Console height |
| `Ctrl+T` | Open a PTY shell in an in-app terminal window |
| `Ctrl+Enter` | Run selection in a terminal window; directory opens a shell there |
| `F6` | Cycle terminal-window focus |
| `F7` | Toggle keyboard window layout mode for the focused terminal |
| `F12` | Release terminal focus back to the cruiser |
| `Shift+F12` | Close the focused terminal window |
| `Shift+PageUp` / `Shift+PageDown` | Scroll the focused terminal's output |
| `J` / `K` | Previous / next node in the current sector |
| `Enter` | Context action: enter directory, launch executable, open file |
| `Backspace` or `H` | Go to parent directory |
| `~` or `Home` | Go to `$HOME` |
| `/` | Filter the current sector incrementally |
| `:` | Open the command console |
| `T` | Open a terminal at the current directory |
| `O` | Open the selected entry with the system handler |
| `R` | Recenter the ship on the sector axis |
| `F` | Freeze / unfreeze motion |
| `F5` | Rescan the current directory |
| `Ctrl+H` | Toggle hidden files |
| `F1` or `?` | Help overlay |
| `F3` | Debug overlay (debug builds only) |
| `F11` or `Alt+Enter` | Toggle fullscreen |
| `Ctrl+Q` or `Esc` from the root overlay | Quit |

Mouse clicks never launch anything. Steering is pointer-driven: mouse motion
arms the joystick-style pointer, and focus follows the reticle at screen
center. The pointer only steers while it is inside the window. Do not capture
or hide the OS pointer by default. Clicking a terminal window focuses it;
dragging its title moves it, dragging the lower-right corner resizes it, and
clicking `X` closes it and terminates the child process.

While a terminal window has focus, every key except the reserved focus/layout
and scrollback chords (`F6`, `F7`, `F12`, `Shift+F12`, `Shift+PageUp`,
`Shift+PageDown`) is sent to the PTY. In window layout mode, arrows move the
focused window and `Shift`+arrows resize it; `F7`, `Enter`, or `Esc` exits
layout mode. Press `F12` to return to cruiser controls. This modal escape
hatch is mandatory: a TUI may use `Esc`, `Ctrl+Q`, function keys, or almost any
other chord itself.

In unlocked Cruise, flying inside a readable directory's galaxy radius enters
that sector automatically. Flying into the parent gate returns to the parent
directory. Lock mode and filters disable proximity entry so inspection cannot
trigger navigation; use `Enter` while locked.

Terminal-window focus is a Cruise sub-mode, not a separate top-level mode. It
does not change the sector or camera; it only routes keyboard input to the
focused PTY. Releasing focus restores cruiser shortcuts immediately.

### 4.3 Focus and selection

- In Cruise, the focus is the node whose projected bounds are closest to the
  reticle within a tolerance. If nothing is close, show no selection.
- The parent gate is a focusable target when the current sector is not the
  root. Aim the reticle at it; `Enter` enters the parent. The selection card
  identifies it as `PARENT SECTOR`.
- Focus is indicated by brackets and a shape change, not color alone.
- Focus updates the selection card immediately.
- Lock mode keeps a stable selected path across rescans when possible.
- Selection identity is the absolute path, not the array index.
- If the selected entry disappears after a rescan, select the nearest
  surviving sibling and show a status message.

### 4.4 Action semantics

`Enter` on the focused node:

| Kind | Behavior |
| --- | --- |
| Directory (readable) | Enter the sector, animate, scan its children |
| Directory (unreadable) | Show a permission error; do not leave the current sector |
| Symlink to directory | Resolve and enter; detect loops |
| Executable regular file | Launch directly via `Command` in the current directory |
| Non-executable regular file | Open with `xdg-open` (Linux) or platform equivalent |
| Symlink to file | Act on the resolved target; show the target in the detail card |
| Socket, FIFO, device | Inspect only; do not execute or block |
| Broken symlink | Show a fault, keep the entry visible |

`O` always asks the system handler to open the path, regardless of executable
bit. `T` opens an external terminal at the current directory (or at a selected
directory's path). `Ctrl+Enter` opens an in-app PTY window: an executable runs
directly in it, a directory opens a shell there, and other entries are
refused. `Enter` must never pass untrusted filenames through a shell.

### 4.5 Command console

The console is a quick launcher, not a shell replacement.

Built-ins:

| Command | Behavior |
| --- | --- |
| `cd [path]` | Change sector; default is `$HOME`; supports `~` and `..` |
| `ls [path]` | Print a compact listing to the console |
| `open <path>` | Open with the system handler |
| `run <path> [args...]` | Execute a file directly; require executable bit |
| `term [dir]` | Open an external terminal at `dir` or the current directory |
| `reload` | Rescan the current directory |
| `home`, `root` | Jump to `$HOME` or `/` |
| `help` | Show console help |
| `quit` | Exit the application |

Rules:

- Parse external commands into argv with `shell-words`; do not implement a
  home-made quote parser and do not use a shell by default.
- `ls -la` should execute `ls` with `-la`, in the current directory, with no
  shell interpretation.
- A leading `!` is the **explicit** opt-in to shell execution:
  `!ls -la | less` runs through `$SHELL -lc` and is marked `SHELL MODE` in the
  status line. This is the only path that may interpret pipes, redirects, and
  variables.
- Show the exact argv and working directory in the status line before/while
  launching.
- Capture stdout and stderr for console commands with reader threads. Never
  block the render loop. Bound the buffer (for example 64 KiB) and mark
  truncation.
- Interactive programs need a terminal. If output stalls or the program
  appears to need a TTY, tell the user to use `term` or `Ctrl+T`; do not make
  console commands allocate a PTY automatically.
- `run` must check the executable bit on Unix. On other platforms use the
  platform's executable detection. If it is not executable, refuse and
  suggest `open` or `term`.
- `.desktop` files are handed to `xdg-open`/`gio open` in v1. Do not parse
  or execute their `Exec=` fields manually.
- Console height is adjustable with `Ctrl+Up` and `Ctrl+Down`, clamped between
  configurable `console_min_lines` and `console_max_lines`. `console_lines`
  sets the starting height.
- Keep history in memory for the session. Up/Down recalls. Tab completes
  built-ins and filesystem paths, not arbitrary shell aliases. Complete the
  first argument of `cd`, `ls`, `open`, `run`, and `term`; use directory-only
  matches for `cd` and `term`. Insert quoted paths when needed, extend a
  shared prefix, and list multiple matches in the console. Never run filesystem
  traversal or shell expansion during completion.

### 4.6 Launch safety

- Never launch on focus, hover, scan, or selection alone.
- No shell interpolation of file names. Use `Command::new(program).args(...)`.
- Preserve exact `OsString` paths for execution; display lossy UTF-8 only in
  the UI.
- Set the child working directory to the current sector unless the command
  specifies another directory.
- GUI programs are spawned detached. Do not wait for them on the render
  thread. Report the PID or an error.
- Do not elevate privileges, use `sudo`, modify setuid bits, or write to the
  filesystem.
- Do not add an "open terminal and paste this" feature that hides the
  command from the user.
- If a launch fails, show the OS error, the exact path, and the cwd in a
  dismissible status/console message.

---

## 5. Technical direction

### 5.1 Stack

Use Rust with:

- `macroquad` for the window, input, 2D/3D drawing, render targets, audio,
  and shader materials.
- `glam` for vectors/matrices (macroquad re-exports it; add it directly only
  if needed).
- `serde` + `toml` for config.
- `directories` for XDG config paths.
- `anyhow` + `thiserror` for errors.
- `shell-words` for command parsing.
- `log` + `env_logger` for logging.
- `portable-pty` for cross-platform PTY sessions in terminal windows.
- `vt100` for bounded VT100/xterm screen emulation.
- `tempfile` as a dev-dependency for filesystem tests.

Do not add Bevy, egui, iced, winit directly, tokio, async-std, or a web
framework. Macroquad owns the event loop. Use threads and `std::sync::mpsc`
for background work. Avoid `unwrap`/`expect` in runtime paths; init-time
`expect` with a clear message is acceptable.

### 5.2 Repository layout

```text
Cargo.toml
AGENTS.md
README.md
LICENSE
assets/
  fonts/            # bundled OFL font + license
  shaders/          # world, bloom, CRT shaders
packaging/
  cbrpnk-cruiser.desktop.in  # display-manager session entry
  cbrpnk-cruiser-session     # session launcher, starts in $HOME
  install-session.sh         # installs binary, launcher, session entry
src/
  main.rs           # thin entry point
  lib.rs            # library root for testable modules
  app.rs            # AppState, mode machine, intents
  config.rs         # config load/validate/defaults
  theme.rs          # all colors, sizes, typography constants
  fs/
    mod.rs          # public API
    entry.rs        # Entry, EntryKind, metadata
    scan.rs         # background directory scanner
    count.rs        # background child-count worker for galaxy sizing
  layout.rs         # deterministic node placement
  camera.rs         # camera, projection helpers, transitions
  input.rs          # key/mouse -> Intent mapping
  render/
    mod.rs          # frame orchestration
    scene.rs        # grid, skyline, rings, nodes, portals
    hud.rs          # breadcrumb, status, selection, console
    text.rs         # font loading, label layout, scaling
    post.rs         # bloom/CRT chain
  launch/
    mod.rs          # launch requests and events
    command.rs      # console parser + built-ins
    complete.rs     # console tab completion
    process.rs      # spawn/detach/capture
    terminal.rs     # external terminal detection/adapters
  wm/
    mod.rs          # terminal windows, focus, geometry, lifecycle
    pty.rs          # PTY worker threads and events
    input.rs        # key encoding for PTY input
    render.rs       # terminal cell/window rendering
  util.rs           # stable hashing, path display, formatting
tests/
  fs_scan.rs
  layout.rs
  launch.rs
```

Keep modules focused. Split files before they exceed roughly 500-600 lines.
Pure logic belongs in the library, not in draw callbacks.

### 5.3 Core state and data flow

Use an intent-based flow:

```text
input -> Intent -> AppState update -> side-effect request -> worker thread
                                                   |
                                                   v
                                            AppEvent channel -> AppState
                                                   |
                                                   v
                                                renderer
```

Suggested core types:

```rust
pub struct Entry {
    pub name: OsString,
    pub path: PathBuf,
    pub kind: EntryKind,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub mode: Option<u32>,
    pub executable: bool,
    pub readable: bool,
    pub link_target: Option<PathBuf>,
}

pub enum EntryKind { Directory, File, Symlink, Other }
```

The `AppState` owns the current directory, the current listing, the selected
path, camera, mode, console state, and config. The renderer is a pure
projection of state plus time; it must not perform I/O.

Use stable identifiers (absolute path) for selection and layout caching.
Do not use array indices as identity across rescans.

### 5.4 Filesystem rules

- Start at the CLI path if provided, else the config path, else the current
  working directory. Expand `~` only when reading UI input.
- Scan **one directory at a time**, lazily. Do not walk the whole filesystem.
- Use `std::fs::read_dir`; use `symlink_metadata` so symlinks are not
  followed during listing. Resolve link targets separately.
- Exclude `.` and `..` from the list. Parent navigation is an explicit action.
- Hide dotfiles by default. `Ctrl+H` and config toggle them. The hidden
  state must be visible in the HUD.
- Sort entries naturally: directories first, then executables, then files,
  then links/other; within groups use case-insensitive natural sort, with a
  stable tie-breaker on the raw `OsString`.
- Keep entries with metadata errors. Show them as unreadable rather than
  dropping them silently.
- Preserve non-UTF-8 names as `OsString`; display with `to_string_lossy()`.
  Never reconstruct a path by concatenating display strings.
- Cap a single listing at `max_entries` (default 5000). Read names, sort,
  then truncate; display `TRUNCATED` and the true count if known. Do not
  allocate unbounded vectors for pathological directories.
- Detect symlink loops when resolving/entering. Track visited canonical
  paths or a depth limit; show a fault instead of hanging.
- Handle permission errors per entry and per directory. Never crash on
  `EACCES`, `ENOENT`, `ELOOP`, or a disappearing entry.
- Cache listings by path for the session. `F5` forces a rescan. Directory
  watchers are optional post-v1; if added, debounce them.
- Scanning runs on a worker thread. Use a generation token so stale results
  are discarded. The UI must not wait for a scan.
- Galaxy sizing counts immediate child entries on a separate worker. Request
  counts only for directories in the current sector, cap the number of
  counted directories per scan, and discard stale generation results. A count
  failure is treated as zero children and never blocks navigation.

### 5.5 Spatial layout

`layout.rs` must be pure and deterministic:

- Input: a listing plus theme/layout constants.
- Output: a position, scale, and visual kind for each entry, using `glam`.
- Sort/group first, then place. Directories stay in an inner shell,
  executables in a middle shell, then files and links outward. Keep groups
  visually distinct without pinning every entry to the same fixed orbit.
- Derive each node's angle, radial offset, and height from a stable path hash.
  Positions must be path-keyed: adding or removing another entry must not move
  an unchanged node. No per-frame randomness or jitter.
- Directory nodes render as small galaxies whose orbit size grows with the
  immediate child count, clamped between theme minimum and maximum sizes.
  Missing counts use a neutral default. Orbit detail is bounded per frame.
- Resolve symlink targets lexically against the sector. Emit an arc to a
  visible target node when possible; otherwise resolve the deepest visible
  directory that contains the target and emit a cross-galaxy arc to it.
- Radius from size: use a cube-root or log scale with hard clamps. Do not
  let one huge file dominate the sector.
- Implement a stable FNV-1a hash locally; do not depend on
  `std::collections::hash_map::DefaultHasher` for cross-version stability.
- Add tests for determinism, bounds, path-keyed stability, galaxy size
  clamping, link resolution, empty directories, and pathological names.

### 5.6 Camera and navigation

- Camera has position, yaw, pitch, velocity, and damping. No roll by default.
- Projection: FOV around 60-70 degrees, near 0.03, far around 200. Keep near
  and far clipped so rings do not vanish through the camera.
- Cruise uses pointer-relative steering (joystick style) plus arrow keys.
  Mouse motion or a thrust click arms the pointer; an off-center pointer
  continues to steer until it returns to center. Do not capture or hide the
  OS mouse unless the user explicitly enables a mouse-look option.
- Left and right mouse buttons thrust forward and reverse. Button and pointer
  input is ignored while the pointer is outside the window.
- Unlocked Cruise automatically enters a readable directory when the camera
  crosses its galaxy entry radius, and returns to the parent when the camera
  reaches the parent gate. Apply a short post-navigation guard, recenter on
  arrival, and never auto-enter while locked, frozen, filtering, scanning, or
  holding a pending navigation.
- Damping and acceleration prevent twitchy motion. Cap velocity in the
  sector and across sector transitions.
- `R` recenters; `F` freezes. Do not add camera shake by default.
- Sector entry/exit uses an explicit view stack (current sector plus parent
  exit gate), not a full recursive tree. This keeps memory and scan cost
  bounded. The parent is always reachable with `Backspace`.
- Keep the parent gate visible behind/above the current sector so the user
  understands where "back" is.

### 5.7 Rendering pipeline

Per frame:

1. Clear to the void color.
2. Draw the background grid and procedural skyline at low intensity.
3. Draw portal curves/links and motion trails.
4. Draw sector rings and nodes, far to near.
5. Draw the reticle, lock brackets, and selection rings.
6. Project labels to screen space and draw them with the bundled monospace
   font.
7. Draw the HUD and any overlay.
8. Run the bloom/CRT post-process when enabled.
9. Present.

Use a render target for post-processing. Keep bloom at half or quarter
resolution. Avoid per-frame heap churn in hot paths; reuse buffers where it
matters, but do not contort the design prematurely. Embedded shaders via
`include_str!` are preferred for portability; keep the source files in
`assets/shaders/` and their licenses in the repo.

### 5.8 Launch and terminal integration

- `launch/process.rs` returns a `LaunchPlan` describing argv, cwd, and
  environment additions. Keep planning separate from execution so it is
  testable without spawning.
- Detached GUI launches use null stdin/stdout/stderr or a log file, and do
  not block. Console commands use captured pipes read by threads.
- In-app terminal windows use `portable-pty` with `vt100` emulation. Reader,
  writer, and child-wait work runs on worker threads; the render thread only
  polls events, processes bounded terminal output, and sends resize/keystroke
  messages. Closing a window kills the child. Keep scrollback bounded.
- `Ctrl+T` and `Ctrl+Enter` launch only on explicit keypress. PTY commands
  preserve `OsString` paths and argv; no shell interpolation or shell mode is
  added implicitly.
- Terminal detection order: config override, `$TERMINAL`, then common
  terminals (`wezterm`, `kitty`, `alacritty`, `foot`, `gnome-terminal`,
  `konsole`, `xfce4-terminal`, `xterm`). Search `PATH` explicitly. Each
  terminal has an adapter for its working-directory flag; test the adapter
  output, not the actual terminal.
- On Linux, use `xdg-open` for system opening. On macOS, use `open`. Do not
  invoke a shell to run `xdg-open`.
- If no terminal or opener exists, report a clear error. Do not silently do
  nothing.
- PID and exit status should be visible in the console. For detached
  programs, report "launched" rather than waiting forever. In-app terminal
  shell windows close when the shell exits; directly launched program windows
  show running/exited/IO-error state in the title bar and retain the final
  screen for inspection.

### 5.9 Config

- Load from `$XDG_CONFIG_HOME/cbrpnk-cruiser/config.toml` (or the platform
  equivalent), with a `--config` override.
- Missing config is fine. Invalid config produces a warning and safe defaults,
  not a crash.
- Suggested settings: `start_path`, `show_hidden`, `max_entries`,
  `console_lines`, `console_min_lines`, `console_max_lines`, `font_scale`,
  `crt`, `reduced_motion`, `terminal`, `shell`, `confirm_exec` (default false),
  `theme`, and log level.
- Keep all user-visible defaults documented in `README.md` and in `--help`.
- Do not write the config file unless the user explicitly saves settings.
- CLI: `cbrpnk-cruiser [PATH]`, `--no-crt`, `--hidden`, `--config <path>`,
  `--help`, `--version`.

### 5.10 Accessibility and comfort

- Every action must have a keyboard path. Terminal windows expose focus,
  cycle, close, scrollback, move, and resize without a mouse.
- Provide `--no-crt` and `reduced_motion`; respect them in shaders, camera
  damping, and transitions.
- Terminal focus must always have an unambiguous keyboard escape (`F12`) so a
  TUI cannot trap the user.
- Color is never the only indicator. Shapes, brackets, and text labels carry
  meaning too.
- Font scale must be adjustable without breaking the HUD.
- Do not flash, strobe, or shake. Avoid rapid camera rotations in overlays.
- Pause ambient motion when the window is unfocused.
- Include a short motion-sickness note in the README and help overlay.

---

## 6. Build, run, and test

### 6.1 Linux prerequisites

Ubuntu/Debian example:

```sh
sudo apt install build-essential pkg-config libx11-dev libxi-dev \
  libgl1-mesa-dev libasound2-dev
```

### 6.2 Commands

```sh
cargo run --release                 # start in the current directory
cargo run --release -- /some/path   # start in a specific directory
cargo run --release -- --no-crt     # disable post-processing
cargo test
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

Use `RUST_LOG=cbrpnk_cruiser=debug cargo run` for diagnostics. Do not leave
`dbg!` output in committed code.

Install as an X11 display-manager session with:

```sh
cargo build --release
sudo SKIP_BUILD=1 packaging/install-session.sh
```

The installer places the binary and session launcher in `$PREFIX/bin`
(default `/usr/local/bin`) and the session entry in
`/usr/share/xsessions`. It is a session entry like AwesomeWM or Sway, not an
X11 window manager; the cruiser manages only its in-app terminal windows.

### 6.3 CI expectation

On Linux, CI should run formatting, clippy, tests, and a release build.
Tests must not require a display server. If a GUI smoke test is added, keep it
behind an explicit feature or environment flag and never make headless CI
depend on it.

---

## 7. Coding rules for agents

1. **Read before writing.** Open the relevant module, its tests, and this
   file. For filesystem behavior, also read the reference map in section 11.
2. **Keep the reference read-only.** Never modify `xcruiser-0.30/`.
3. **No copied GPL code.** If you adapt an algorithm, rewrite it in Rust,
   document the source idea in a comment or commit message, and keep the
   project GPL-compatible.
4. **No new dependencies without justification.** If one is necessary,
   update the stack list here and explain why the standard library or an
   existing crate is insufficient.
5. **Keep I/O off the render thread.** Scans, launches, and process output
   belong in workers.
6. **No `unwrap`/`expect` on user or filesystem data.** Return `Result` and
   surface errors in the UI. Init-time `expect` is acceptable with a clear
   message.
7. **No `unsafe` unless there is no safe alternative.** If used, include a
   `// SAFETY:` comment and tests.
8. **Centralize visuals in `theme.rs`.** No scattered hex colors, font sizes,
   or magic layout numbers.
9. **Keep pure logic pure.** Layout, parsing, command planning, and config
   validation must be testable without a window.
10. **Write tests with the change.** Filesystem tests use `tempfile`; launch
    tests use fake commands and never run destructive operations.
11. **Prefer small, reviewable changes.** Do not mix refactors with feature
    work unless the refactor is required to land the feature safely.
12. **Update docs.** If controls, config keys, architecture, or scope change,
    update this file and `README.md` in the same change.
13. **Preserve user work.** Never run destructive commands, never delete
    user files, and never modify the filesystem in tests outside a tempdir.
14. **No secrets, no telemetry, no network calls.**
15. **Run the checks before declaring done.** Formatting, clippy, tests, and
    a build are the minimum bar.

---

## 8. Performance budgets

- Target 60 FPS at 1920x1080 on integrated graphics with default effects.
- Frame budget: 16.6 ms. Keep the main-thread work per frame below 8 ms in
  normal browsing.
- Directory scan latency: under 100 ms for a typical directory (hundreds of
  entries) on a local disk. Huge directories may take longer but must not
  block the UI.
- No main-thread filesystem call longer than roughly 5 ms; if one is
  unavoidable, move it to a worker.
- Render at most the current sector and its visible parent/exit gate. Cap
  visible labels (for example 150) and distant-node detail.
- Bloom at half or quarter resolution. Disable the effect under
  `--no-crt`.
- Add a debug overlay (`F3`) showing frame time, node count, visible label
  count, scan time, and worker queue depth. Do not ship it enabled.
- Profile before optimizing. Fix obvious allocations in hot paths, but do not
  sacrifice clarity for unmeasured micro-optimizations.

---

## 9. Roadmap and acceptance criteria

Implement in this order. Each milestone must leave the app runnable and the
checks green.

### M0 - Skeleton

- Cargo project with library + thin binary.
- Window opens, clears to void, stable game loop, graceful quit.
- Theme module, logger, CLI parsing, `--help`/`--version`.
- Acceptance: app launches and exits cleanly; format/clippy/test/build pass.

### M1 - Filesystem core

- `Entry`, metadata capture, natural sort, hidden handling, caps.
- Background scanner with generation tokens and error reporting.
- Tests for tempdirs, permissions, symlinks, non-UTF-8 names, huge listings.
- Acceptance: scanner never blocks the loop; stale results are ignored;
  errors are visible in a debug list or console.

### M2 - Spatial world

- Deterministic path-hashed layout for directories, files, executables, links.
- Child-count-based galaxy sizing and direct/cross-galaxy link arcs.
- Camera, projection, rings, node glyphs, labels, depth sorting.
- Cruise navigation with damping and freeze.
- Acceptance: a real directory renders with correct entry count; layout is
  stable across frames and rescans when unchanged.

### M3 - Browser UX

- Focus/selection, Lock mode, Enter/Backspace semantics, parent gate.
- Breadcrumb, selection card, status line, hidden toggle, filter, rescan,
  home/root jumps.
- Symlink handling and fault messages.
- Acceptance: navigate at least three levels deep and back; unreadable
  entries and symlink loops do not hang; keyboard-only operation works.

### M4 - Style and feel

- Final palette, grid, skyline, portal curves, reticle, transitions.
- Boot sequence, CRT/bloom shader, `--no-crt`, reduced motion, font scale.
- Acceptance: looks deliberate at 1080p; text remains readable; disabling
  effects works; no flashing or excessive motion.

### M5 - Launcher

- `Enter` launch/open semantics; `xdg-open` integration; terminal detection.
- Command console, built-ins, direct argv execution, explicit `!` shell mode.
- Launch planning tests, terminal adapter tests, error surfacing.
- Acceptance: launch an executable, open a text file with the system
  handler, open a real terminal at the current directory, and see success or
  a clear error for each.

### M5.5 - In-app terminal windows (explicit scope expansion)

- PTY shell and direct executable windows using `portable-pty` + `vt100`.
- Modal terminal focus with `F12` release, `F6` cycle, `F7` keyboard layout,
  `Shift+F12` close, and `Shift+PageUp/Down` scrollback.
- Mouse focus, drag, resize, and close; title-bar running/exited status.
- Application cursor keys are tracked so TUIs such as `htop` receive the
  correct arrow sequences.
- Shell windows close automatically when the shell exits; launched program
  windows retain their final screen.
- Acceptance: run a shell, `vim`, and `htop` in windows; all keys including
  `Esc`, `Ctrl+Q`, and arrows reach the TUI while focused; `F12` restores
  cruiser keys; closing a window terminates its child; no render-thread
  blocking.

### M6 - Polish and ship

- Complete config, help overlay, README, license and asset credits.
- Debug overlay, performance pass, packaging notes.
- Acceptance: clean build on Linux with documented dependencies; 10-minute
  browse/rescan soak without leaks or stalls; all v1 success criteria met.

Post-v1 candidates (do not start without approval): recursive search,
directory watcher, audio, file operations with undo/confirm, Windows/macOS
polish, desktop-entry handling, custom themes, plugin API.

---

## 10. Testing strategy

- **Unit tests**: layout determinism/bounds, natural sort, path display,
  config validation, command parsing, launch planning, terminal adapter
  construction, stable hashing.
- **Integration tests**: scan a tempdir with ordinary files, directories,
  symlinks, hidden files, unreadable entries (Unix), and non-UTF-8 names
  (Unix). Assert that no test touches the real home directory.
- **Process tests**: use temp shell scripts or `/bin/echo` to verify argv,
  cwd, and error paths. Never run `rm`, `mv`, or anything destructive.
- **PTY tests**: spawn harmless commands such as `/bin/echo` in a PTY and
  assert output, exit status, and resize behavior. Time out instead of
  blocking forever; clean up children.
- **Manual smoke test**: start in `/`, `$HOME`, and a deep project directory;
  enter/leave, toggle hidden, filter, rescan, open a text file, launch a
  script, open a terminal, quit. Open an in-app shell, run `vim` and `htop`,
  verify focus/release, scrollback, layout mode, resize, and close.
- **Performance check**: debug overlay with a large but safe temp directory;
  verify frame time and scan responsiveness.
- Do not add brittle screenshot tests in v1. Visual review is manual until
  the renderer stabilizes.

---

## 11. XCruiser reference map

Study these files for ideas. Do not copy code, constants, or data structures
wholesale. The reference is a 2003 C/X11 program with known limits; our job is
to keep the spatial idea and fix the usability and safety problems.

| Reference | Learn | Known issue to avoid |
| --- | --- | --- |
| `zone.c` `calc_pos`, `calc_dir` | Deterministic placement from names, ring planes | Character-arithmetic layout is hard to reason about; use stable keys and tests |
| `zone.c` `scan_dir` | Lazy expansion of directories, radius from size | Static 256-star buffer, blocking scan, `sprintf` into fixed buffers |
| `zone.c` `enter_galaxy`, `leave_galaxy` | Frame/view stack for nested sectors | Fixed 64-frame limit; unclear exit behavior |
| `zone.c` `move_ship_polar`, `move_ship_cartesian` | Steering vs. parallel flight, damping/limits | Motion-sickness-inducing defaults; no reduced-motion option |
| `draw.c` `trans_pos`, `clip_zplane` | Perspective projection and near-plane clipping | Manual clipping is easy to break; use tested math |
| `draw.c` `draw_universe`, `precalc_pos` | Traversal, z-order, scaled positions | Entire visible tree recomputed each frame; no culling by sector |
| `draw.c` `draw_galaxy`, `draw_planet`, `draw_wormhole` | Ring geometry, labels, link curves, size-based radius | Label overlap and thick-wave rendering not addressed |
| `MainScreen.c` idle loop | Input-action-state-refresh cycle | Single-threaded scanning, partial launch stub, leaks |
| `MainScreen.c` translations | Keyboard/mouse bindings and flying modes | Hard-coded defaults; awkward keyboard support |
| `info.c` | HUD layout, size formatting, alerts | Cosmetic text only; no actionable errors |
| `xcruiser.c`, `XCruiser.ad`, `README` | Defaults, palette, bootstrapping | Ancient font/resource assumptions |
| `xcruiser2` | A modified binary with autopilot/dialog experiments | Treat as an experiment, not a specification |

The important idea to preserve is simple: **directory trees become spatial
places you can remember**. The important things to improve are lazy scanning,
stable layout, readable HUD, keyboard access, real launching, explicit safety,
and accessible motion.

---

## 12. Licensing and originality

- The project license is **GPL-2.0-or-later**, compatible with the GPL
  reference. Keep `LICENSE` at the repository root.
- If you port an algorithm from `xcruiser-0.30/`, preserve the GPL terms and
  document the origin. If you produce a clean-room implementation, say so in
  the commit message.
- The reference has no warranty; do not imply endorsement by its author.
- Bundle only fonts, shaders, sounds, and images that are compatible with the
  project license and include their license files under `assets/`.
- Do not use Hackers/Tron logos, stills, clips, sound effects, or trademarks.
  The visual language is inspiration, not reproduction.
- Keep the app name original. "CBRPNK//CRUISER" is the working name.

---

## 13. Definition of done

A change is done when:

- [ ] It builds with `cargo build --release`.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes.
- [ ] `cargo test` passes, including new tests for new pure logic.
- [ ] I/O is off the render thread, and errors are visible to the user.
- [ ] Keyboard access exists for every new user-facing action.
- [ ] Colors/shapes/fonts come from `theme.rs`, not ad-hoc literals.
- [ ] The reference tree is untouched and no GPL-incompatible code was added.
- [ ] README/AGENTS.md are updated if controls, config, or scope changed.
- [ ] The app still launches, navigates, and quits cleanly in a real session.

If any of these is impossible, stop and ask the human instead of shipping a
partial hidden failure.
