use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use macroquad::prelude::{
    clear_background, get_char_pressed, get_fps, get_frame_time, get_time, is_key_pressed,
    is_quit_requested, next_frame, screen_height, screen_width, KeyCode,
};

use crate::camera::Camera;
use crate::cli::Cli;
use crate::config::{Config, ConfigLoad};
use crate::fs::{CountRequest, CountWorker, Entry, ScanRequest, ScanResult, ScanWorker};
use crate::input::{self, Intent};
use crate::launch::command::{self, Builtin, Command};
use crate::launch::process::{
    plan_executable, plan_external, plan_open, plan_shell, plan_terminal, spawn_detached,
    LaunchEvent, LaunchWorker, OutputStream,
};
use crate::launch::terminal::detect as detect_terminal;
use crate::layout::SectorLayout;
use crate::render::hud::{self, ConsoleState, Hints, StatusMessage, Tone, TopHud};
use crate::render::scene::{parent_gate_position, parent_gate_reached, Scene};
use crate::render::Renderer;
use crate::theme::{
    AUTO_ENTER_GUARD, BOOT_SECONDS, FOCUS_TOLERANCE_PX, MAX_DIRECTORY_COUNTS, STATUS_LIFETIME, VOID,
};
use crate::util::home_dir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Boot,
    Cruise,
    Console,
    Help,
}

#[derive(Debug, Clone)]
struct PendingNav {
    via_symlink: bool,
}

#[derive(Debug)]
struct FilterState {
    text: String,
}

pub struct App {
    renderer: Renderer,
    config: Config,
    cwd: PathBuf,
    canonical_cwd: Option<PathBuf>,
    entries: Vec<Entry>,
    layout: SectorLayout,

    scan_worker: ScanWorker,
    scan_generation: u64,
    scanning: bool,
    count_worker: CountWorker,
    count_generation: u64,
    directory_counts: HashMap<PathBuf, usize>,
    pending_nav: Option<PendingNav>,
    last_scan_ms: f32,
    total_entries: usize,
    visible_entries: usize,
    hidden_filtered: usize,
    entry_errors: usize,
    truncated: bool,
    canonical_history: Vec<PathBuf>,

    selected: Option<PathBuf>,
    locked: bool,
    filter: Option<FilterState>,
    applied_filter: Option<String>,

    mode: Mode,
    camera: Camera,
    auto_enter_guard: f32,
    time: f32,
    boot_started: f64,
    show_hidden: bool,
    show_debug: bool,
    fullscreen: bool,
    quit: bool,

    console: ConsoleState,
    history: Vec<String>,
    history_index: usize,
    launch_worker: LaunchWorker,
    next_task_id: u64,
    active_tasks: HashMap<u64, String>,

    statuses: Vec<StatusMessage>,
    host: String,
    last_frame_ms: f32,
}

impl App {
    pub fn new(cli: Cli, loaded: ConfigLoad, renderer: Renderer) -> Self {
        let config = loaded.config;
        let mut warnings = Vec::new();
        if let Some(warning) = loaded.warning {
            warnings.push(warning);
        }
        let start = initial_path(&cli, &config, &mut warnings);
        let show_hidden = cli.hidden || config.show_hidden;
        let host = read_host();

        let mut app = Self {
            renderer,
            config,
            cwd: start.clone(),
            canonical_cwd: None,
            entries: Vec::new(),
            layout: SectorLayout::default(),
            scan_worker: ScanWorker::spawn(),
            scan_generation: 0,
            scanning: false,
            count_worker: CountWorker::spawn(),
            count_generation: 0,
            directory_counts: HashMap::new(),
            pending_nav: None,
            last_scan_ms: 0.0,
            total_entries: 0,
            visible_entries: 0,
            hidden_filtered: 0,
            entry_errors: 0,
            truncated: false,
            canonical_history: Vec::new(),
            selected: None,
            locked: false,
            filter: None,
            applied_filter: None,
            mode: Mode::Boot,
            camera: Camera::new(),
            auto_enter_guard: 0.0,
            time: 0.0,
            boot_started: get_time(),
            show_hidden,
            show_debug: false,
            fullscreen: false,
            quit: false,
            console: ConsoleState::default(),
            history: Vec::new(),
            history_index: 0,
            launch_worker: LaunchWorker::spawn(),
            next_task_id: 0,
            active_tasks: HashMap::new(),
            statuses: Vec::new(),
            host,
            last_frame_ms: 0.0,
        };

        for warning in warnings {
            app.push_status(format!("CONFIG: {warning}"), Tone::Error);
            app.console.push(format!("config: {warning}"), Tone::Error);
        }
        app.start_scan(start.clone(), Some(PendingNav { via_symlink: false }));
        app
    }

    pub async fn run(mut self) {
        loop {
            if is_quit_requested() {
                break;
            }
            let dt = get_frame_time().clamp(0.0, 0.05);
            self.last_frame_ms = dt * 1000.0;
            self.time += dt;
            self.poll_scans();
            self.poll_counts();
            self.poll_launches();
            self.update(dt);
            self.draw();
            next_frame().await;
            if self.quit {
                break;
            }
        }
    }

    fn update(&mut self, dt: f32) {
        self.age_statuses(dt);
        self.auto_enter_guard = (self.auto_enter_guard - dt).max(0.0);
        if input::ctrl_down() && is_key_pressed(KeyCode::Q) {
            self.quit = true;
            return;
        }
        if is_key_pressed(KeyCode::F3) {
            self.show_debug = !self.show_debug;
        }
        if is_key_pressed(KeyCode::F11) || (is_key_down_alt() && is_key_pressed(KeyCode::Enter)) {
            self.toggle_fullscreen();
            return;
        }

        match self.mode {
            Mode::Boot => {
                let elapsed = get_time() - self.boot_started;
                if input::any_input_pressed() || elapsed >= BOOT_SECONDS as f64 {
                    self.mode = Mode::Cruise;
                }
                return;
            }
            Mode::Help => {
                if is_key_pressed(KeyCode::F1)
                    || is_key_pressed(KeyCode::Escape)
                    || (input::shift_down() && is_key_pressed(KeyCode::Slash))
                {
                    self.mode = Mode::Cruise;
                }
                return;
            }
            Mode::Console => {
                self.handle_console_keys();
                return;
            }
            Mode::Cruise => {}
        }

        if self.filter.is_some() {
            self.handle_filter_keys();
            return;
        }

        if let Some(intent) = input::cruise_intent(self.locked) {
            self.handle_intent(intent);
            if self.quit {
                return;
            }
        }
        let camera_input = input::camera_input(self.locked);
        self.camera
            .update(dt, camera_input, self.config.reduced_motion);
        if self.try_auto_enter() {
            return;
        }
        self.update_focus();
    }

    fn try_auto_enter(&mut self) -> bool {
        if self.locked
            || self.camera.frozen
            || self.scanning
            || self.pending_nav.is_some()
            || self.filter.is_some()
            || self.applied_filter.is_some()
            || self.auto_enter_guard > 0.0
        {
            return false;
        }
        if let Some(parent) = self.cwd.parent() {
            if parent_gate_reached(self.camera.position) {
                let parent = parent.to_path_buf();
                self.auto_enter_guard = AUTO_ENTER_GUARD;
                self.navigate_to(parent, false);
                self.push_status("PARENT GATE ENTRY", Tone::Command);
                return true;
            }
        }

        let Some(entry) = self
            .layout
            .proximity_directory(&self.entries, self.camera.position)
        else {
            return false;
        };
        let path = entry.path.clone();
        let via_symlink = entry.kind == crate::fs::EntryKind::Symlink;
        let label = entry.display_name();
        self.auto_enter_guard = AUTO_ENTER_GUARD;
        self.navigate_to(path, via_symlink);
        self.push_status(format!("PROXIMITY ENTRY: {label}"), Tone::Command);
        true
    }

    fn handle_intent(&mut self, intent: Intent) {
        match intent {
            Intent::Quit => self.quit = true,
            Intent::Help => {
                self.mode = Mode::Help;
                self.console.visible = false;
            }
            Intent::Fullscreen => {}
            Intent::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                self.push_status(
                    if self.show_hidden {
                        "HIDDEN FILES ON"
                    } else {
                        "HIDDEN FILES OFF"
                    },
                    Tone::Normal,
                );
                self.rescan();
            }
            Intent::Rescan => self.rescan(),
            Intent::Activate => self.activate_selected(),
            Intent::Parent => self.go_parent(),
            Intent::Home => self.go_home(),
            Intent::Root => self.navigate_to(PathBuf::from("/"), false),
            Intent::ToggleLock => self.toggle_lock(),
            Intent::PrevNode => self.cycle_node(-1),
            Intent::NextNode => self.cycle_node(1),
            Intent::Open => self.open_selected(),
            Intent::Terminal => self.start_terminal(None),
            Intent::Console => self.open_console(),
            Intent::Filter => self.open_filter(),
            Intent::Recenter => {
                self.camera.recenter();
                self.push_status("CAMERA RECENTERED", Tone::Normal);
            }
            Intent::Freeze => {
                self.camera.toggle_freeze();
                self.push_status(
                    if self.camera.frozen {
                        "MOTION FROZEN"
                    } else {
                        "MOTION RESUMED"
                    },
                    Tone::Normal,
                );
            }
        }
    }

    fn handle_filter_keys(&mut self) {
        let mut changed = false;
        if is_key_pressed(KeyCode::Escape) {
            self.filter = None;
            self.applied_filter = None;
            self.rebuild_layout();
            self.update_focus();
            self.push_status("FILTER CLEARED", Tone::Normal);
            return;
        }
        if is_key_pressed(KeyCode::Enter) {
            let text = self
                .filter
                .as_ref()
                .map(|filter| filter.text.trim().to_string())
                .unwrap_or_default();
            self.applied_filter = (!text.is_empty()).then_some(text.clone());
            self.filter = None;
            self.rebuild_layout();
            self.update_focus();
            if text.is_empty() {
                self.push_status("FILTER CLEARED", Tone::Normal);
            } else {
                self.push_status(format!("FILTER: {text}"), Tone::Normal);
            }
            return;
        }
        if is_key_pressed(KeyCode::Backspace) {
            if let Some(filter) = &mut self.filter {
                filter.text.pop();
                changed = true;
            }
        }
        while let Some(character) = get_char_pressed() {
            if !character.is_control() {
                if let Some(filter) = &mut self.filter {
                    filter.text.push(character);
                    changed = true;
                }
            }
        }
        if changed {
            self.rebuild_layout();
            self.update_focus();
        }
    }

    fn open_filter(&mut self) {
        let text = self.applied_filter.clone().unwrap_or_default();
        self.filter = Some(FilterState { text });
        self.rebuild_layout();
        self.push_status("FILTER MODE", Tone::Command);
    }

    fn handle_console_keys(&mut self) {
        if is_key_pressed(KeyCode::Escape) {
            self.close_console();
            return;
        }
        if is_key_pressed(KeyCode::Enter) {
            self.submit_console();
            return;
        }
        if is_key_pressed(KeyCode::Backspace) {
            self.console.input.pop();
            return;
        }
        if is_key_pressed(KeyCode::Up) {
            self.recall_history(-1);
            return;
        }
        if is_key_pressed(KeyCode::Down) {
            self.recall_history(1);
            return;
        }
        if is_key_pressed(KeyCode::PageUp) {
            self.console.scroll_up(6);
            return;
        }
        if is_key_pressed(KeyCode::PageDown) {
            self.console.scroll_down(6);
            return;
        }
        while let Some(character) = get_char_pressed() {
            if !character.is_control() {
                self.console.input.push(character);
            }
        }
    }

    fn recall_history(&mut self, direction: i32) {
        if self.history.is_empty() {
            return;
        }
        if direction < 0 {
            self.history_index = self.history_index.saturating_sub(1);
        } else {
            self.history_index = (self.history_index + 1).min(self.history.len());
        }
        self.console.input = if self.history_index < self.history.len() {
            self.history[self.history_index].clone()
        } else {
            String::new()
        };
    }

    fn submit_console(&mut self) {
        let line = std::mem::take(&mut self.console.input);
        let line = line.trim().to_string();
        if line.is_empty() {
            return;
        }
        if self.history.last() != Some(&line) {
            self.history.push(line.clone());
        }
        self.history_index = self.history.len();
        self.console.scroll = 0;
        self.console.push(format!(":{line}"), Tone::Command);
        match command::parse_line(&line) {
            Ok(parsed) => self.execute_command(parsed),
            Err(error) => {
                self.console.push(error.to_string(), Tone::Error);
                self.push_status(error.to_string(), Tone::Error);
            }
        }
    }

    fn execute_command(&mut self, command: Command) {
        match command {
            Command::Shell(shell) => {
                let plan = plan_shell(&shell, &self.shell_path(), &self.cwd);
                self.start_captured(plan, true);
            }
            Command::External(words) => match plan_external(&words, &self.cwd) {
                Ok(plan) => self.start_captured(plan, false),
                Err(error) => {
                    self.console.push(error.to_string(), Tone::Error);
                    self.push_status(error.to_string(), Tone::Error);
                }
            },
            Command::Builtin(builtin) => self.execute_builtin(builtin),
        }
    }

    fn execute_builtin(&mut self, builtin: Builtin) {
        match builtin {
            Builtin::Cd(path) => {
                let target = match path {
                    Some(path) => self.resolve_path(&path),
                    None => home_dir().unwrap_or_else(|| PathBuf::from("/")),
                };
                self.navigate_to(target, false);
            }
            Builtin::Ls(path) => match path {
                Some(path) => {
                    let target = self.resolve_path(&path);
                    self.start_listing(target);
                }
                None => self.list_current(),
            },
            Builtin::Open(path) => {
                let target = self.resolve_path(&path.to_string_lossy());
                self.open_path(&target);
            }
            Builtin::Run { path, args } => {
                let target = self.resolve_path(&path.to_string_lossy());
                let args: Vec<OsString> = args.into_iter().map(OsString::from).collect();
                match plan_executable(&target, &self.cwd, args) {
                    Ok(plan) => self.start_captured(plan, false),
                    Err(error) => {
                        self.console.push(error.to_string(), Tone::Error);
                        self.push_status(error.to_string(), Tone::Error);
                    }
                }
            }
            Builtin::Term(path) => {
                let target = path.map(|path| self.resolve_path(&path));
                self.start_terminal(target);
            }
            Builtin::Reload => {
                self.rescan();
                self.console.push("rescan requested", Tone::Success);
            }
            Builtin::Home => {
                let home = home_dir().unwrap_or_else(|| PathBuf::from("/"));
                self.navigate_to(home, false);
            }
            Builtin::Root => self.navigate_to(PathBuf::from("/"), false),
            Builtin::Help => self.console_help(),
            Builtin::Quit => self.quit = true,
        }
    }

    fn console_help(&mut self) {
        for line in [
            "built-ins: cd [path], ls [path], open <path>, run <path> [args...]",
            "           term [dir], reload, home, root, help, quit",
            "external:  ls -la, git status, etc. run as argv without a shell",
            "shell:     !<command> runs through $SHELL -lc, explicitly",
            "keys:      Up/Down history, PageUp/PageDown scroll, Esc close",
        ] {
            self.console.push(line, Tone::Muted);
        }
    }

    fn list_current(&mut self) {
        self.console
            .push(format!("{}:", self.cwd.display()), Tone::Normal);
        for entry in &self.entries {
            let marker = if entry.kind == crate::fs::EntryKind::Directory {
                "/"
            } else if entry.executable {
                "*"
            } else if entry.kind == crate::fs::EntryKind::Symlink {
                "@"
            } else {
                ""
            };
            self.console
                .push(format!("{}{marker}", entry.display_name()), Tone::Normal);
        }
        if self.truncated {
            self.console.push("[truncated]", Tone::Error);
        }
    }

    fn start_listing(&mut self, path: PathBuf) {
        let id = self.next_id();
        self.active_tasks
            .insert(id, format!("ls {}", path.display()));
        self.console
            .push(format!("$ ls {}", path.display()), Tone::Command);
        self.launch_worker
            .list_directory(id, path, self.show_hidden);
    }

    fn start_captured(&mut self, plan: crate::launch::LaunchPlan, shell_mode: bool) {
        let id = self.next_id();
        self.active_tasks.insert(id, plan.label.clone());
        self.console
            .push(format!("$ {}", plan.display_command()), Tone::Command);
        self.console
            .push(format!("  cwd {}", plan.cwd.display()), Tone::Muted);
        if shell_mode {
            self.console.push("  SHELL MODE", Tone::Error);
            self.push_status("SHELL MODE", Tone::Error);
        }
        self.launch_worker.run(id, plan);
    }

    fn poll_launches(&mut self) {
        while let Some(event) = self.launch_worker.try_recv() {
            match event {
                LaunchEvent::Started {
                    id,
                    label,
                    pid,
                    command,
                    cwd,
                } => {
                    self.active_tasks.insert(id, label.clone());
                    self.console.push(
                        format!("  pid {pid}  {command}  ({})", cwd.display()),
                        Tone::Muted,
                    );
                }
                LaunchEvent::Output { stream, text, .. } => {
                    let tone = if stream == OutputStream::Stderr {
                        Tone::Error
                    } else {
                        Tone::Normal
                    };
                    self.console.push(text, tone);
                }
                LaunchEvent::Finished {
                    id,
                    success,
                    code,
                    truncated,
                } => {
                    self.active_tasks.remove(&id);
                    let message = match (success, code) {
                        (true, Some(code)) => format!("[exit {code}]"),
                        (false, Some(code)) => format!("[exit {code} / FAILED]"),
                        (_, None) => "[exit / signal]".to_string(),
                    };
                    if truncated {
                        self.console
                            .push("[output truncated at 64 KiB]", Tone::Error);
                    }
                    self.console.push(
                        message.clone(),
                        if success { Tone::Success } else { Tone::Error },
                    );
                    self.push_status(message, if success { Tone::Success } else { Tone::Error });
                }
                LaunchEvent::Failed { id, message } => {
                    self.active_tasks.remove(&id);
                    self.console.push(format!("error: {message}"), Tone::Error);
                    self.push_status(format!("LAUNCH FAILED: {message}"), Tone::Error);
                }
            }
        }
    }

    fn poll_scans(&mut self) {
        while let Some(result) = self.scan_worker.try_recv() {
            if result.generation != self.scan_generation {
                continue;
            }
            self.apply_scan(result);
        }
    }

    fn poll_counts(&mut self) {
        while let Some(result) = self.count_worker.try_recv() {
            if result.generation != self.count_generation {
                continue;
            }
            self.directory_counts.extend(result.counts);
            self.rebuild_layout();
        }
    }

    fn apply_scan(&mut self, result: ScanResult) {
        let ScanResult {
            path,
            canonical_path,
            generation: _,
            entries,
            total_entries,
            visible_entries,
            hidden_filtered,
            entry_errors,
            truncated,
            error,
            duration,
        } = result;

        let navigation = self.pending_nav.take();
        let fatal = error.is_some() && entries.is_empty();
        if fatal {
            self.scanning = false;
            if navigation.is_some() {
                self.auto_enter_guard = AUTO_ENTER_GUARD * 6.0;
            }
            let message = format!(
                "SCAN FAILED: {}: {}",
                path.display(),
                error.as_deref().unwrap_or("unknown error")
            );
            self.push_status(message.clone(), Tone::Error);
            self.console.push(message, Tone::Error);
            return;
        }

        if let Some(nav) = &navigation {
            if nav.via_symlink {
                if let Some(canonical) = &canonical_path {
                    if self.canonical_history.iter().any(|path| path == canonical) {
                        self.scanning = false;
                        self.auto_enter_guard = AUTO_ENTER_GUARD * 6.0;
                        let message = format!("SYMLINK LOOP REFUSED: {}", canonical.display());
                        self.push_status(message.clone(), Tone::Error);
                        self.console.push(message, Tone::Error);
                        return;
                    }
                }
            }
        }

        let previous_selected = self.selected.clone();
        self.cwd = path;
        self.canonical_cwd = canonical_path.clone();
        self.entries = entries;
        self.count_generation = self.count_generation.wrapping_add(1);
        self.directory_counts.clear();
        self.total_entries = total_entries;
        self.visible_entries = visible_entries;
        self.hidden_filtered = hidden_filtered;
        self.entry_errors = entry_errors;
        self.truncated = truncated;
        self.last_scan_ms = duration.as_secs_f32() * 1000.0;
        self.scanning = false;

        if navigation.is_some() {
            self.selected = None;
            self.locked = false;
            self.filter = None;
            self.applied_filter = None;
            self.camera.recenter();
            self.auto_enter_guard = AUTO_ENTER_GUARD;
            if let Some(canonical) = canonical_path {
                self.canonical_history.push(canonical);
                if self.canonical_history.len() > 64 {
                    self.canonical_history.drain(0..16);
                }
            }
        }

        self.rebuild_layout();
        self.request_directory_counts();

        if navigation.is_none() {
            if let Some(previous) = previous_selected {
                let keep = self.cwd.parent() == Some(previous.as_path())
                    || self.entries.iter().any(|entry| entry.path == previous);
                if keep {
                    self.selected = Some(previous);
                } else if let Some(node) = self.layout.nearest_sibling(&previous) {
                    self.selected = Some(node.path.clone());
                    self.push_status(
                        format!("SELECTION MOVED: {}", node.path.display()),
                        Tone::Normal,
                    );
                }
            }
        }

        if let Some(error) = error {
            self.push_status(format!("SCAN WARNING: {error}"), Tone::Error);
        }
        if self.entry_errors > 0 {
            self.push_status(
                format!("{} ENTRY ERRORS (kept visible)", self.entry_errors),
                Tone::Error,
            );
        }
        if self.truncated {
            self.push_status("SECTOR TRUNCATED AT MAX_ENTRIES", Tone::Error);
        }
    }

    fn start_scan(&mut self, path: PathBuf, navigation: Option<PendingNav>) {
        self.scan_generation = self.scan_generation.wrapping_add(1);
        self.scanning = true;
        self.pending_nav = navigation;
        self.scan_worker.request(ScanRequest {
            path,
            generation: self.scan_generation,
            show_hidden: self.show_hidden,
            max_entries: self.config.max_entries,
        });
    }

    fn rescan(&mut self) {
        self.start_scan(self.cwd.clone(), None);
        self.push_status("RESCANNING SECTOR", Tone::Command);
    }

    fn navigate_to(&mut self, path: PathBuf, via_symlink: bool) {
        if path == self.cwd && !via_symlink {
            self.rescan();
            return;
        }
        self.auto_enter_guard = AUTO_ENTER_GUARD;
        self.push_status(format!("OPEN {}", path.display()), Tone::Command);
        self.start_scan(path, Some(PendingNav { via_symlink }));
    }

    fn go_parent(&mut self) {
        match self.cwd.parent() {
            Some(parent) => {
                let parent = parent.to_path_buf();
                self.navigate_to(parent, false);
            }
            None => self.push_status("ALREADY AT ROOT", Tone::Normal),
        }
    }

    fn go_home(&mut self) {
        match home_dir() {
            Some(home) => self.navigate_to(home, false),
            None => self.push_status("HOME IS NOT SET", Tone::Error),
        }
    }

    fn activate_selected(&mut self) {
        if self.selected_is_parent() {
            self.go_parent();
            return;
        }
        let Some(entry) = self.selected_entry().cloned() else {
            self.push_status("NO TARGET LOCKED", Tone::Normal);
            return;
        };
        if entry.link_broken {
            self.push_status(
                format!("BROKEN PORTAL: {}", entry.display_name()),
                Tone::Error,
            );
            return;
        }
        if entry.is_dir_like() {
            if !entry.readable {
                self.push_status(
                    format!("UNREADABLE SECTOR: {}", entry.display_name()),
                    Tone::Error,
                );
                return;
            }
            self.navigate_to(
                entry.path.clone(),
                entry.kind == crate::fs::EntryKind::Symlink,
            );
        } else if entry.executable {
            self.launch_executable(&entry.path);
        } else if entry.kind == crate::fs::EntryKind::File {
            self.open_path(&entry.path);
        } else {
            self.push_status(
                format!("INSPECT ONLY: {}", entry.display_name()),
                Tone::Normal,
            );
        }
    }

    fn launch_executable(&mut self, path: &Path) {
        match plan_executable(path, &self.cwd, Vec::new()) {
            Ok(plan) => match spawn_detached(&plan) {
                Ok(pid) => {
                    let message = format!("LAUNCHED PID {pid}: {}", path.display());
                    self.push_status(message.clone(), Tone::Success);
                    self.console.push(message, Tone::Success);
                }
                Err(error) => {
                    let message = format!("LAUNCH FAILED: {error}");
                    self.push_status(message.clone(), Tone::Error);
                    self.console.push(message, Tone::Error);
                }
            },
            Err(error) => {
                let message = format!("LAUNCH FAILED: {error}");
                self.push_status(message.clone(), Tone::Error);
                self.console.push(message, Tone::Error);
            }
        }
    }

    fn open_selected(&mut self) {
        let path = if self.selected_is_parent() {
            self.cwd.parent().map(Path::to_path_buf)
        } else {
            self.selected_entry().map(|entry| entry.path.clone())
        };
        let Some(path) = path else {
            self.push_status("NO TARGET LOCKED", Tone::Normal);
            return;
        };
        self.open_path(&path);
    }

    fn open_path(&mut self, path: &Path) {
        match plan_open(path, &self.cwd) {
            Ok(plan) => match spawn_detached(&plan) {
                Ok(pid) => {
                    let message = format!("OPENED PID {pid}: {}", path.display());
                    self.push_status(message.clone(), Tone::Success);
                    self.console.push(message, Tone::Success);
                }
                Err(error) => {
                    let message = format!("OPEN FAILED: {error}");
                    self.push_status(message.clone(), Tone::Error);
                    self.console.push(message, Tone::Error);
                }
            },
            Err(error) => {
                let message = format!("OPEN FAILED: {error}");
                self.push_status(message.clone(), Tone::Error);
                self.console.push(message, Tone::Error);
            }
        }
    }

    fn start_terminal(&mut self, directory: Option<PathBuf>) {
        let selected_dir = if self.selected_is_parent() {
            self.cwd.parent().map(Path::to_path_buf)
        } else {
            self.selected_entry()
                .filter(|entry| entry.is_dir_like())
                .map(|entry| entry.path.clone())
        };
        let target = directory
            .or(selected_dir)
            .unwrap_or_else(|| self.cwd.clone());
        if !target.is_dir() {
            let message = format!("NOT A DIRECTORY: {}", target.display());
            self.push_status(message.clone(), Tone::Error);
            self.console.push(message, Tone::Error);
            return;
        }
        let Some(terminal) = detect_terminal(self.config.terminal.as_deref(), &target) else {
            let message = "NO TERMINAL FOUND: set terminal in config or $TERMINAL".to_string();
            self.push_status(message.clone(), Tone::Error);
            self.console.push(message, Tone::Error);
            return;
        };
        let plan = plan_terminal(terminal);
        match spawn_detached(&plan) {
            Ok(pid) => {
                let message = format!("TERMINAL PID {pid} AT {}", target.display());
                self.push_status(message.clone(), Tone::Success);
                self.console.push(message, Tone::Success);
            }
            Err(error) => {
                let message = format!("TERMINAL FAILED: {error}");
                self.push_status(message.clone(), Tone::Error);
                self.console.push(message, Tone::Error);
            }
        }
    }

    fn toggle_lock(&mut self) {
        if self.locked {
            self.locked = false;
            self.push_status("LOCK RELEASED", Tone::Normal);
            return;
        }
        if self.selected.is_none() {
            self.selected = self
                .focus_candidate()
                .or_else(|| self.layout.nodes.first().map(|node| node.path.clone()));
        }
        if self.selected.is_some() {
            self.locked = true;
            self.push_status("TARGET LOCKED", Tone::Command);
        } else {
            self.push_status("NO NODES TO LOCK", Tone::Normal);
        }
    }

    fn cycle_node(&mut self, direction: isize) {
        if self.layout.nodes.is_empty() {
            self.push_status("SECTOR IS EMPTY", Tone::Normal);
            return;
        }
        let current = self
            .selected
            .as_ref()
            .and_then(|path| self.layout.node_index_for(path));
        let length = self.layout.nodes.len() as isize;
        let index = match current {
            Some(index) => (index as isize + direction).rem_euclid(length),
            None if direction > 0 => 0,
            None => length - 1,
        };
        self.selected = Some(self.layout.nodes[index as usize].path.clone());
        self.locked = true;
    }

    fn update_focus(&mut self) {
        if self.mode != Mode::Cruise {
            return;
        }
        if self.locked {
            if self.selected_is_parent() {
                return;
            }
            if let Some(selected) = &self.selected {
                if self.layout.node_index_for(selected).is_some() {
                    return;
                }
            }
            self.selected = self
                .focus_candidate()
                .or_else(|| self.layout.nodes.first().map(|node| node.path.clone()));
            return;
        }
        self.selected = self.focus_candidate();
    }

    fn focus_candidate(&self) -> Option<PathBuf> {
        let width = screen_width();
        let height = screen_height();
        let center = macroquad::prelude::vec2(width * 0.5, height * 0.5);
        let mut best: Option<(f32, PathBuf)> = None;
        for node in &self.layout.nodes {
            let Some(projected) = self.camera.project(node.position, width, height) else {
                continue;
            };
            let screen_radius = (node.radius * projected.scale).clamp(3.0, 46.0);
            let distance = projected.screen.distance(center);
            if distance <= FOCUS_TOLERANCE_PX + screen_radius * 0.7
                && best
                    .as_ref()
                    .map(|(best_distance, _)| distance < *best_distance)
                    .unwrap_or(true)
            {
                best = Some((distance, node.path.clone()));
            }
        }
        if let Some((distance, radius, path)) = self.parent_gate_hit() {
            if distance <= FOCUS_TOLERANCE_PX + radius * 0.7
                && best
                    .as_ref()
                    .map(|(best_distance, _)| distance < *best_distance)
                    .unwrap_or(true)
            {
                best = Some((distance, path));
            }
        }
        best.map(|(_, path)| path)
    }

    fn parent_gate_hit(&self) -> Option<(f32, f32, PathBuf)> {
        let parent = self.cwd.parent()?;
        let width = screen_width();
        let height = screen_height();
        let center = macroquad::prelude::vec2(width * 0.5, height * 0.5);
        let projected = self.camera.project(parent_gate_position(), width, height)?;
        let radius = (projected.scale * 0.55).clamp(18.0, 54.0);
        Some((
            projected.screen.distance(center),
            radius,
            parent.to_path_buf(),
        ))
    }

    fn selected_is_parent(&self) -> bool {
        self.selected
            .as_deref()
            .is_some_and(|selected| self.cwd.parent() == Some(selected))
    }

    fn selected_parent(&self) -> Option<&Path> {
        if self.selected_is_parent() {
            self.cwd.parent()
        } else {
            None
        }
    }

    fn selected_entry(&self) -> Option<&Entry> {
        let path = self.selected.as_ref()?;
        self.entries.iter().find(|entry| &entry.path == path)
    }

    fn open_console(&mut self) {
        self.mode = Mode::Console;
        self.console.visible = true;
        self.console.input.clear();
        self.console.scroll = 0;
        self.history_index = self.history.len();
    }

    fn close_console(&mut self) {
        self.console.visible = false;
        self.mode = Mode::Cruise;
    }

    fn resolve_path(&self, input: &str) -> PathBuf {
        let path = crate::util::expand_tilde(input);
        if path.is_absolute() {
            path
        } else {
            self.cwd.join(path)
        }
    }

    fn shell_path(&self) -> PathBuf {
        self.config
            .shell
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("SHELL").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("/bin/sh"))
    }

    fn rebuild_layout(&mut self) {
        let indices = self.visible_indices();
        self.layout =
            SectorLayout::build_with_counts(&self.entries, &indices, &self.directory_counts);
    }

    fn request_directory_counts(&mut self) {
        let mut paths = Vec::new();
        for entry in &self.entries {
            if paths.len() >= MAX_DIRECTORY_COUNTS {
                break;
            }
            if entry.is_dir_like() && !self.directory_counts.contains_key(&entry.path) {
                paths.push(entry.path.clone());
            }
        }
        if paths.is_empty() {
            return;
        }
        self.count_worker.request(CountRequest {
            paths,
            generation: self.count_generation,
        });
    }

    fn visible_indices(&self) -> Vec<usize> {
        let filter = self.filter_text().map(|text| text.to_lowercase());
        match filter {
            Some(filter) => self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.display_name().to_lowercase().contains(&filter))
                .map(|(index, _)| index)
                .collect(),
            None => (0..self.entries.len()).collect(),
        }
    }

    fn filter_text(&self) -> Option<&str> {
        if let Some(filter) = &self.filter {
            if !filter.text.is_empty() {
                return Some(filter.text.as_str());
            }
        }
        self.applied_filter
            .as_deref()
            .filter(|text| !text.is_empty())
    }

    fn next_id(&mut self) -> u64 {
        self.next_task_id = self.next_task_id.wrapping_add(1);
        self.next_task_id
    }

    fn push_status(&mut self, text: impl Into<String>, tone: Tone) {
        self.statuses.push(hud::status_line(text, tone));
        if self.statuses.len() > 8 {
            self.statuses.drain(0..self.statuses.len() - 8);
        }
    }

    fn age_statuses(&mut self, dt: f32) {
        for status in &mut self.statuses {
            status.age += dt;
        }
        self.statuses.retain(|status| status.age <= STATUS_LIFETIME);
    }

    fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        macroquad::window::set_fullscreen(self.fullscreen);
    }

    fn draw(&self) {
        clear_background(VOID);
        if self.mode == Mode::Boot {
            let progress = ((get_time() - self.boot_started) / BOOT_SECONDS as f64) as f32;
            hud::draw_boot(&self.renderer.fonts, &self.cwd, progress, self.time);
            self.renderer.draw_post(self.time);
            return;
        }

        let scene = Scene {
            camera: &self.camera,
            layout: &self.layout,
            entries: &self.entries,
            selected: self.selected.as_deref(),
            locked: self.locked,
            sector_path: &self.cwd,
            time: self.time,
            reduced_motion: self.config.reduced_motion,
        };
        scene.draw(&self.renderer.fonts);

        if self.layout.nodes.is_empty() {
            let message = if self.filter_text().is_some() {
                "NO MATCHING NODES"
            } else if self.scanning {
                "SCANNING SECTOR..."
            } else {
                "SECTOR EMPTY / INSPECT CONSOLE"
            };
            hud::draw_empty(&self.renderer.fonts, message);
        }

        let clock = chrono::Local::now().format("%H:%M:%S").to_string();
        let sector_name = self
            .cwd
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/".to_string());
        let top = TopHud {
            cwd: &self.cwd,
            sector_name: &sector_name,
            total_entries: self.total_entries,
            visible_entries: self.visible_entries,
            hidden_filtered: self.hidden_filtered,
            truncated: self.truncated,
            show_hidden: self.show_hidden,
            frozen: self.camera.frozen,
            locked: self.locked,
            scanning: self.scanning,
            scan_ms: self.last_scan_ms,
            host: &self.host,
            time: &clock,
            fps: get_fps(),
        };
        hud::draw_top(&self.renderer.fonts, &top, self.time);

        if self.mode == Mode::Help {
            hud::draw_help(&self.renderer.fonts);
            self.renderer.draw_post(self.time);
            return;
        }

        hud::draw_selection(
            &self.renderer.fonts,
            self.selected_entry(),
            self.selected_parent(),
        );
        let filter = self.filter_text();
        let hints = Hints {
            statuses: &self.statuses,
            filter,
            filter_count: self.layout.nodes.len(),
            filter_editing: self.filter.is_some(),
        };
        hud::draw_hints(&self.renderer.fonts, &hints, self.time);

        if self.mode == Mode::Console {
            hud::draw_console(&self.renderer.fonts, &self.console, self.time);
        }

        if self.show_debug {
            let debug = format!(
                "DEBUG // F3\nframe {:.2} ms  fps {}\nnodes {}  visible {}  labels max 150\nscan {:.1} ms  tasks {}\nentries {}  errors {}\ncamera {:.1},{:.1},{:.1}\npost {}  locked {}",
                self.last_frame_ms,
                get_fps(),
                self.layout.nodes.len(),
                self.visible_entries,
                self.last_scan_ms,
                self.active_tasks.len(),
                self.entries.len(),
                self.entry_errors,
                self.camera.position.x,
                self.camera.position.y,
                self.camera.position.z,
                if self.renderer.post_enabled() { "on" } else { "off" },
                self.locked,
            );
            hud::draw_debug(&self.renderer.fonts, &debug);
        }

        self.renderer.draw_post(self.time);
    }
}

fn initial_path(cli: &Cli, config: &Config, warnings: &mut Vec<String>) -> PathBuf {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    let mut candidate = cli
        .path
        .clone()
        .or_else(|| config.start_path.clone())
        .unwrap_or_else(|| base.clone());
    if candidate.is_relative() {
        candidate = base.join(candidate);
    }
    if !candidate.exists() {
        warnings.push(format!(
            "start path does not exist, using {}",
            base.display()
        ));
        return base;
    }
    candidate
}

fn read_host() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty())
        })
        .unwrap_or_else(|| "LOCAL".to_string())
}

fn is_key_down_alt() -> bool {
    macroquad::prelude::is_key_down(KeyCode::LeftAlt)
        || macroquad::prelude::is_key_down(KeyCode::RightAlt)
}
