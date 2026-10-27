use std::path::Path;

use macroquad::prelude::{
    draw_line, draw_rectangle, draw_rectangle_lines, screen_height, screen_width, Color,
};

use crate::fs::Entry;
use crate::render::text::{Align, Fonts};
use crate::theme::{
    alpha, AMBER, CONSOLE_BUFFER, CONSOLE_LINES, CONSOLE_LINE_HEIGHT, CONSOLE_MAX_LINES,
    CONSOLE_MIN_LINES, CONSOLE_TOP_RESERVE, CYAN, CYAN_DIM, DEEP_PANEL, FAULT, FONT_BODY,
    FONT_SMALL, FONT_TINY, FONT_TITLE, GREEN, HUD_GAP, ICE, MAGENTA, PAD, PANEL_ALPHA, TEXT_MUTED,
    VOID,
};
use crate::util::{elide, format_mode, format_size, format_time};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Normal,
    Command,
    Success,
    Error,
    Muted,
}

#[derive(Debug, Clone)]
pub struct ConsoleLine {
    pub text: String,
    pub tone: Tone,
}

#[derive(Debug, Clone)]
pub struct ConsoleState {
    pub visible: bool,
    pub input: String,
    pub lines: Vec<ConsoleLine>,
    pub scroll: usize,
    pub visible_lines: usize,
    pub min_lines: usize,
    pub max_lines: usize,
}

impl Default for ConsoleState {
    fn default() -> Self {
        Self {
            visible: false,
            input: String::new(),
            lines: vec![
                ConsoleLine {
                    text: "console ready. type help for commands.".to_string(),
                    tone: Tone::Muted,
                },
                ConsoleLine {
                    text: "leading ! enables explicit shell mode.".to_string(),
                    tone: Tone::Muted,
                },
            ],
            scroll: 0,
            visible_lines: CONSOLE_LINES,
            min_lines: CONSOLE_MIN_LINES,
            max_lines: CONSOLE_MAX_LINES,
        }
    }
}

impl ConsoleState {
    pub fn configured(visible_lines: usize, min_lines: usize, max_lines: usize) -> Self {
        let min_lines = min_lines.max(1);
        let max_lines = max_lines.max(min_lines);
        Self {
            visible_lines: visible_lines.clamp(min_lines, max_lines),
            min_lines,
            max_lines,
            ..Self::default()
        }
    }

    pub fn push(&mut self, text: impl Into<String>, tone: Tone) {
        self.lines.push(ConsoleLine {
            text: text.into(),
            tone,
        });
        if self.lines.len() > CONSOLE_BUFFER {
            let overflow = self.lines.len() - CONSOLE_BUFFER;
            self.lines.drain(0..overflow);
        }
        self.scroll = 0;
    }

    pub fn scroll_up(&mut self, amount: usize) {
        let max = self.lines.len().saturating_sub(1);
        self.scroll = (self.scroll + amount).min(max);
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.scroll = self.scroll.saturating_sub(amount);
    }

    pub fn resize(&mut self, delta: i32) {
        let next = self.visible_lines as i32 + delta;
        self.visible_lines = next.clamp(self.min_lines as i32, self.max_lines as i32) as usize;
    }
}

#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub tone: Tone,
    pub age: f32,
}

#[derive(Debug, Clone)]
pub struct HandshakeLine {
    pub text: String,
    pub age: f32,
}

#[derive(Debug, Clone)]
pub struct TopHud<'a> {
    pub cwd: &'a Path,
    pub sector_name: &'a str,
    pub total_entries: usize,
    pub visible_entries: usize,
    pub hidden_filtered: usize,
    pub truncated: bool,
    pub show_hidden: bool,
    pub frozen: bool,
    pub locked: bool,
    pub scanning: bool,
    pub scan_ms: f32,
    pub host: &'a str,
    pub time: &'a str,
    pub fps: i32,
}

pub struct Hints<'a> {
    pub statuses: &'a [StatusMessage],
    pub filter: Option<&'a str>,
    pub filter_count: usize,
    pub filter_editing: bool,
}

pub fn draw_top(fonts: &Fonts, data: &TopHud, time: f32) {
    let width = screen_width();
    let left_width = (width * 0.55).min(660.0);
    let left_height = 82.0;
    panel(PAD, PAD, left_width, left_height, alpha(CYAN_DIM, 0.7));
    let x = PAD + 12.0;
    let mut y = PAD + 9.0;
    let path = data.cwd.to_string_lossy();
    fonts.draw_elided(
        &path,
        x,
        y,
        FONT_BODY,
        left_width - 24.0,
        ICE,
        true,
        Align::Left,
    );
    y += 24.0;
    let flags = format!(
        "{}{}{}{}",
        if data.show_hidden { "HIDDEN " } else { "" },
        if data.frozen { "FROZEN " } else { "" },
        if data.locked { "LOCK " } else { "" },
        if data.truncated { "TRUNCATED" } else { "" }
    );
    let sector = format!("SECTOR // {}", data.sector_name);
    fonts.draw(
        &sector,
        x,
        y,
        FONT_SMALL,
        alpha(CYAN, 0.9),
        true,
        Align::Left,
    );
    let sector_width = fonts.width(&sector, FONT_SMALL, true);
    fonts.draw(
        &format!(
            "{} ENTRIES  {} HIDDEN{}",
            data.visible_entries,
            data.hidden_filtered,
            if data.truncated { "  [CAPPED]" } else { "" }
        ),
        x + sector_width + 18.0,
        y,
        FONT_SMALL,
        TEXT_MUTED,
        false,
        Align::Left,
    );
    y += 20.0;
    let scan = if data.scanning {
        "SCANNING...".to_string()
    } else {
        format!(
            "{} VISIBLE / {} TOTAL  ·  SCAN {:.1} ms",
            data.visible_entries, data.total_entries, data.scan_ms
        )
    };
    fonts.draw(
        &scan,
        x,
        y,
        FONT_TINY,
        alpha(GREEN, 0.75),
        false,
        Align::Left,
    );
    if !flags.is_empty() {
        let flags_width = fonts.width(&flags, FONT_TINY, true);
        fonts.draw(
            &flags,
            PAD + left_width - 12.0,
            PAD + left_height - 20.0,
            FONT_TINY,
            alpha(AMBER, 0.8),
            true,
            Align::Right,
        );
        let _ = flags_width;
    }

    let right_width = 270.0;
    let right_x = width - PAD - right_width;
    panel(right_x, PAD, right_width, left_height, alpha(CYAN_DIM, 0.7));
    fonts.draw(
        data.host,
        right_x + 12.0,
        PAD + 9.0,
        FONT_BODY,
        ICE,
        true,
        Align::Left,
    );
    fonts.draw(
        data.time,
        right_x + right_width - 12.0,
        PAD + 9.0,
        FONT_BODY,
        alpha(GREEN, 0.95),
        true,
        Align::Right,
    );
    let pulse = 0.5 + 0.5 * (time * 2.0).sin();
    let status = if data.scanning {
        "LINK ACTIVE  /  SCAN"
    } else {
        "LINK ACTIVE  /  READY"
    };
    fonts.draw(
        status,
        right_x + 12.0,
        PAD + 35.0,
        FONT_TINY,
        alpha(GREEN, 0.55 + pulse * 0.25),
        true,
        Align::Left,
    );
    if cfg!(debug_assertions) {
        fonts.draw(
            &format!("{} FPS", data.fps),
            right_x + 12.0,
            PAD + 54.0,
            FONT_TINY,
            TEXT_MUTED,
            false,
            Align::Left,
        );
    }
    fonts.draw(
        "F1 HELP  ·  ESC QUIT",
        right_x + right_width - 12.0,
        PAD + 54.0,
        FONT_TINY,
        TEXT_MUTED,
        false,
        Align::Right,
    );
}

pub fn draw_selection(fonts: &Fonts, selected: Option<&Entry>, parent: Option<&Path>) {
    let width = (screen_width() * 0.42).min(540.0);
    let height = 126.0;
    let x = PAD;
    let y = screen_height() - height - PAD;
    panel(x, y, width, height, alpha(CYAN_DIM, 0.65));
    let content_x = x + 12.0;
    let mut top = y + 8.0;
    match selected {
        Some(entry) => {
            let color = if entry.link_broken || entry.metadata_error.is_some() {
                FAULT
            } else {
                ICE
            };
            fonts.draw_elided(
                &entry.display_name(),
                content_x,
                top,
                FONT_BODY,
                width - 24.0,
                color,
                true,
                Align::Left,
            );
            top += 23.0;
            fonts.draw(
                entry.kind_label(),
                content_x,
                top,
                FONT_TINY,
                kind_color(entry),
                true,
                Align::Left,
            );
            let kind_width = fonts.width(entry.kind_label(), FONT_TINY, true);
            let mut details = format!(
                "{}  ·  {}  ·  {}",
                format_size(entry.size),
                format_time(entry.modified),
                format_mode(entry.mode)
            );
            if !entry.readable {
                details.push_str("  ·  UNREADABLE");
            }
            if entry.executable {
                details.push_str("  ·  EXEC");
            }
            fonts.draw(
                &details,
                content_x + kind_width + 14.0,
                top,
                FONT_TINY,
                TEXT_MUTED,
                false,
                Align::Left,
            );
            top += 22.0;
            if let Some(target) = &entry.link_target {
                let target = target.to_string_lossy();
                fonts.draw_elided(
                    &format!("PORTAL → {target}"),
                    content_x,
                    top,
                    FONT_TINY,
                    width - 24.0,
                    alpha(MAGENTA, 0.9),
                    false,
                    Align::Left,
                );
                if entry.link_broken {
                    fonts.draw(
                        "BROKEN LINK",
                        x + width - 12.0,
                        top,
                        FONT_TINY,
                        FAULT,
                        true,
                        Align::Right,
                    );
                }
                top += 20.0;
            }
            if let Some(error) = &entry.metadata_error {
                fonts.draw_elided(
                    error,
                    content_x,
                    top,
                    FONT_TINY,
                    width - 24.0,
                    alpha(FAULT, 0.8),
                    false,
                    Align::Left,
                );
            }
        }
        None if parent.is_some() => {
            let parent = parent.unwrap_or(Path::new("/"));
            fonts.draw("..", content_x, top, FONT_BODY, ICE, true, Align::Left);
            top += 23.0;
            fonts.draw(
                "PARENT SECTOR",
                content_x,
                top,
                FONT_TINY,
                CYAN,
                true,
                Align::Left,
            );
            top += 22.0;
            fonts.draw_elided(
                &parent.to_string_lossy(),
                content_x,
                top,
                FONT_TINY,
                width - 24.0,
                TEXT_MUTED,
                false,
                Align::Left,
            );
            top += 22.0;
            fonts.draw(
                "ENTER go back  ·  Backspace also returns",
                content_x,
                top,
                FONT_TINY,
                alpha(TEXT_MUTED, 0.8),
                false,
                Align::Left,
            );
        }
        None => {
            fonts.draw(
                "NO TARGET",
                content_x,
                top,
                FONT_BODY,
                alpha(CYAN_DIM, 0.8),
                true,
                Align::Left,
            );
            top += 24.0;
            fonts.draw(
                "aim the reticle at a node",
                content_x,
                top,
                FONT_SMALL,
                TEXT_MUTED,
                false,
                Align::Left,
            );
            top += 20.0;
            fonts.draw(
                "Tab to lock focus  ·  ? for help",
                content_x,
                top,
                FONT_TINY,
                alpha(TEXT_MUTED, 0.75),
                false,
                Align::Left,
            );
        }
    }
}

pub fn draw_hints(fonts: &Fonts, hints: &Hints, time: f32) {
    let width = 340.0;
    let height = 92.0;
    let x = screen_width() - PAD - width;
    let y = screen_height() - PAD - height;
    panel(x, y, width, height, alpha(CYAN_DIM, 0.6));
    let content_x = x + 12.0;
    let mut top = y + 8.0;
    fonts.draw(
        "CONTROLS",
        content_x,
        top,
        FONT_TINY,
        alpha(CYAN, 0.9),
        true,
        Align::Left,
    );
    top += 18.0;
    for line in [
        "ENTER act  ·  TAB lock  ·  : console",
        "Ctrl+T window  ·  : console  ·  ? help",
        "WASD/arrows move  ·  F6 term  ·  F freeze",
    ] {
        fonts.draw(
            line,
            content_x,
            top,
            FONT_TINY,
            TEXT_MUTED,
            false,
            Align::Left,
        );
        top += 16.0;
    }

    if let Some(filter) = hints.filter {
        let panel_width = (screen_width() * 0.48).min(660.0);
        let panel_height = 54.0;
        let panel_x = (screen_width() - panel_width) * 0.5;
        let panel_y = screen_height() - panel_height - PAD - 100.0;
        panel(
            panel_x,
            panel_y,
            panel_width,
            panel_height,
            alpha(CYAN, 0.65),
        );
        let cursor = if hints.filter_editing {
            if ((time * 2.0) as i32) % 2 == 0 {
                "_"
            } else {
                " "
            }
        } else {
            ""
        };
        fonts.draw(
            &format!("/{filter}{cursor}"),
            panel_x + 12.0,
            panel_y + 8.0,
            FONT_BODY,
            ICE,
            true,
            Align::Left,
        );
        let right = if hints.filter_editing {
            format!("{} MATCH · ENTER KEEP · ESC CLEAR", hints.filter_count)
        } else {
            format!("{} MATCH · / EDIT · ESC CLEAR", hints.filter_count)
        };
        fonts.draw(
            &right,
            panel_x + panel_width - 12.0,
            panel_y + 12.0,
            FONT_TINY,
            alpha(TEXT_MUTED, 0.9),
            false,
            Align::Right,
        );
    }

    let mut top = screen_height() - PAD - 126.0;
    for message in hints.statuses.iter().rev().take(3) {
        let fade = (1.0 - message.age / 7.0).clamp(0.0, 1.0);
        let color = tone_color(message.tone);
        let width = fonts.width(&message.text, FONT_TINY, false);
        fonts.draw_elided(
            &message.text,
            screen_width() * 0.5 + width * 0.5,
            top,
            FONT_TINY,
            screen_width() * 0.55,
            alpha(color, 0.85 * fade),
            false,
            Align::Right,
        );
        top -= 17.0;
    }
}

fn fit_console_lines(requested: usize, screen_height: f32) -> usize {
    let available = (screen_height - PAD * 2.0 - CONSOLE_TOP_RESERVE).max(0.0);
    let max_lines = (((available - 24.0) / CONSOLE_LINE_HEIGHT).floor() as i32 - 2).max(1);
    requested.min(max_lines as usize)
}

pub fn draw_console(fonts: &Fonts, console: &ConsoleState, time: f32) {
    if !console.visible {
        return;
    }
    let line_height = CONSOLE_LINE_HEIGHT;
    let shown_lines = fit_console_lines(console.visible_lines, screen_height());
    let height = (shown_lines + 2) as f32 * line_height + 24.0;
    let width = screen_width() - PAD * 2.0;
    let x = PAD;
    let y = screen_height() - height - PAD;
    draw_rectangle(x, y, width, height, alpha(DEEP_PANEL, 0.97));
    draw_rectangle_lines(x, y, width, height, 1.0, alpha(CYAN, 0.7));
    fonts.draw(
        &format!(
            "CONSOLE  //  {}L  ·  ENTER RUN  ·  ESC CLOSE  ·  CTRL+UP/DOWN HEIGHT  ·  ! SHELL",
            shown_lines
        ),
        x + 12.0,
        y + 5.0,
        FONT_TINY,
        alpha(CYAN, 0.9),
        true,
        Align::Left,
    );
    let content_top = y + 24.0;
    let end = console.lines.len().saturating_sub(console.scroll);
    let start = end.saturating_sub(shown_lines);
    let mut line_y = content_top;
    for line in &console.lines[start..end] {
        fonts.draw_elided(
            &line.text,
            x + 16.0,
            line_y,
            FONT_SMALL,
            width - 32.0,
            tone_color(line.tone),
            line.tone == Tone::Command,
            Align::Left,
        );
        line_y += line_height;
    }
    let prompt_y = y + height - line_height - 8.0;
    draw_line(
        x + 12.0,
        prompt_y - 4.0,
        x + width - 12.0,
        prompt_y - 4.0,
        1.0,
        alpha(CYAN_DIM, 0.35),
    );
    let cursor = if ((time * 2.4) as i32) % 2 == 0 {
        "_"
    } else {
        " "
    };
    fonts.draw(
        &format!(":{}{}", console.input, cursor),
        x + 16.0,
        prompt_y,
        FONT_BODY,
        ICE,
        false,
        Align::Left,
    );
}

pub fn draw_handshake(fonts: &Fonts, line: &HandshakeLine, time: f32) {
    let width = screen_width();
    let age = line.age;
    let life = crate::theme::HANDSHAKE_LIFETIME;
    let t = (age / life).clamp(0.0, 1.0);
    let alpha_t = if t < 0.2 {
        t / 0.2
    } else if t > 0.8 {
        (1.0 - t) / 0.2
    } else {
        1.0
    };
    if alpha_t <= 0.0 {
        return;
    }
    let color = alpha(crate::theme::CYAN, 0.9 * alpha_t);
    let y = 92.0 + (1.0 - alpha_t) * 4.0;
    let glitch = if age < 0.12 && ((time * 18.0) as i32) % 3 == 0 {
        2.0
    } else {
        0.0
    };
    fonts.draw(
        &line.text,
        width * 0.5 + glitch,
        y,
        crate::theme::FONT_SMALL,
        color,
        true,
        Align::Center,
    );
}

pub fn draw_help(fonts: &Fonts) {
    let width = screen_width();
    let height = screen_height();
    draw_rectangle(0.0, 0.0, width, height, alpha(VOID, 0.94));
    let x = PAD * 2.0;
    let column = (width - x * 2.0) / 4.0;
    let mut top = 48.0;
    fonts.draw(
        "CBRPNK//CRUISER  //  FLIGHT MANUAL",
        x,
        top,
        FONT_TITLE,
        ICE,
        true,
        Align::Left,
    );
    top += 36.0;
    fonts.draw(
        "ESC or F1 closes this overlay",
        x,
        top,
        FONT_SMALL,
        TEXT_MUTED,
        false,
        Align::Left,
    );
    top += 34.0;
    let sections: [(&str, &[&str]); 4] = [
        (
            "NAVIGATION",
            &[
                "Arrows / mouse  aim / steer",
                "L / R mouse     thrust forward / reverse",
                "W / S           thrust forward / reverse",
                "A / D           strafe left / right",
                "Q / E           move down / up",
                "Space           thrust",
                "Shift           boost",
                "Ctrl            brake",
                "Mouse wheel     cruise speed",
                "R               recenter",
                "F               freeze / unfreeze",
            ],
        ),
        (
            "FILES",
            &[
                "Enter           enter / launch / open",
                "Tab             toggle lock",
                "J / K           previous / next node",
                "Backspace / H   parent directory",
                "Home / ~        home directory",
                "/               filter sector",
                "Ctrl+H          toggle hidden files",
                "F5              rescan",
                "O               open with system handler",
            ],
        ),
        (
            "CONSOLE",
            &[
                ":               open console",
                "cd [path]       change sector",
                "ls [path]       compact listing",
                "open <path>     system handler",
                "run <path> ...  execute directly",
                "term [dir]      external terminal",
                "reload          rescan sector",
                "home / root     jump",
                "Tab             complete built-ins/paths",
                "Ctrl+Up/Down    resize console",
                "!<command>      explicit shell mode",
                "quit            exit application",
            ],
        ),
        (
            "TERMINAL WINDOWS",
            &[
                "Ctrl+T          shell in app window",
                "Ctrl+Enter      run selection in window",
                "F6              cycle window focus",
                "F7              keyboard window layout",
                "F12             release terminal focus",
                "Shift+F12       close focused window",
                "Shift+PgUp/Dn   scroll terminal output",
                "Layout mode     arrows move / shift resize",
                "Click title     focus / drag window",
                "Click X         close window",
                "Drag corner     resize window",
                "Focused input   goes to the program",
                "Unfocused       cruiser keys still work",
            ],
        ),
    ];
    for (index, (title, lines)) in sections.iter().enumerate() {
        let column_x = x + column * index as f32;
        let mut y = top;
        fonts.draw(
            title,
            column_x,
            y,
            FONT_SMALL,
            alpha(AMBER, 0.95),
            true,
            Align::Left,
        );
        y += 24.0;
        for line in *lines {
            fonts.draw(
                line,
                column_x,
                y,
                FONT_SMALL,
                TEXT_MUTED,
                false,
                Align::Left,
            );
            y += 20.0;
        }
    }
    top += 300.0;
    fonts.draw(
        "Comfort: --no-crt disables post-processing. Use F to freeze motion.",
        x,
        top,
        FONT_TINY,
        alpha(CYAN, 0.75),
        false,
        Align::Left,
    );
    fonts.draw(
        "Motion note: this interface has continuous camera movement. Stop if you feel discomfort.",
        x,
        top + 18.0,
        FONT_TINY,
        alpha(TEXT_MUTED, 0.85),
        false,
        Align::Left,
    );
    fonts.draw(
        "Launching is explicit: focus alone never runs anything.",
        x,
        top + 36.0,
        FONT_TINY,
        alpha(GREEN, 0.8),
        false,
        Align::Left,
    );
}

pub fn draw_boot(fonts: &Fonts, path: &Path, progress: f32, time: f32) {
    let width = screen_width();
    let height = screen_height();
    draw_rectangle(0.0, 0.0, width, height, VOID);
    let center = width * 0.5;
    fonts.draw(
        "CBRPNK//CRUISER",
        center,
        height * 0.38,
        FONT_TITLE + 8.0,
        ICE,
        true,
        Align::Center,
    );
    fonts.draw(
        "MAINFRAME LINK ESTABLISHED",
        center,
        height * 0.38 + 40.0,
        FONT_SMALL,
        alpha(GREEN, 0.8),
        true,
        Align::Center,
    );
    let bar_width = (width * 0.42).min(520.0);
    let bar_x = center - bar_width * 0.5;
    let bar_y = height * 0.55;
    draw_rectangle_lines(bar_x, bar_y, bar_width, 12.0, 1.0, alpha(CYAN_DIM, 0.7));
    draw_rectangle(
        bar_x + 2.0,
        bar_y + 2.0,
        (bar_width - 4.0) * progress.clamp(0.0, 1.0),
        8.0,
        alpha(CYAN, 0.8),
    );
    fonts.draw_elided(
        &path.to_string_lossy(),
        center,
        bar_y + 28.0,
        FONT_SMALL,
        width * 0.8,
        alpha(TEXT_MUTED, 0.9),
        false,
        Align::Center,
    );
    let dots = ".".repeat((time * 4.0) as usize % 4);
    fonts.draw(
        &format!("MOUNTING SECTOR{dots}"),
        center,
        bar_y - 24.0,
        FONT_SMALL,
        alpha(CYAN, 0.85),
        false,
        Align::Center,
    );
    fonts.draw(
        "PRESS ANY KEY OR CLICK TO SKIP",
        center,
        height * 0.72,
        FONT_TINY,
        alpha(TEXT_MUTED, 0.75),
        false,
        Align::Center,
    );
}

pub fn draw_empty(fonts: &Fonts, text: &str) {
    let center = screen_width() * 0.5;
    fonts.draw(
        text,
        center,
        screen_height() * 0.47,
        FONT_TITLE,
        alpha(CYAN_DIM, 0.65),
        true,
        Align::Center,
    );
}

pub fn draw_debug(fonts: &Fonts, text: &str) {
    let width = 320.0;
    let height = 20.0 * text.lines().count() as f32 + 16.0;
    let x = screen_width() - width - PAD;
    let y = PAD + 96.0;
    panel(x, y, width, height, alpha(AMBER, 0.55));
    let mut top = y + 8.0;
    for line in text.lines() {
        fonts.draw(
            line,
            x + 10.0,
            top,
            FONT_TINY,
            alpha(AMBER, 0.9),
            false,
            Align::Left,
        );
        top += 20.0;
    }
}

fn panel(x: f32, y: f32, width: f32, height: f32, border: Color) {
    draw_rectangle(x, y, width, height, alpha(DEEP_PANEL, PANEL_ALPHA));
    draw_rectangle_lines(x, y, width, height, 1.0, border);
    let accent = 14.0;
    draw_line(x, y, x + accent, y, 1.5, alpha(CYAN, 0.9));
    draw_line(x, y, x, y + accent, 1.5, alpha(CYAN, 0.9));
    draw_line(
        x + width,
        y + height,
        x + width - accent,
        y + height,
        1.5,
        alpha(CYAN, 0.55),
    );
    draw_line(
        x + width,
        y + height,
        x + width,
        y + height - accent,
        1.5,
        alpha(CYAN, 0.55),
    );
}

fn kind_color(entry: &Entry) -> Color {
    if entry.link_broken || entry.metadata_error.is_some() {
        FAULT
    } else if entry.kind == crate::fs::EntryKind::Directory {
        CYAN
    } else if entry.executable {
        AMBER
    } else if entry.kind == crate::fs::EntryKind::Symlink {
        MAGENTA
    } else {
        alpha(ICE, 0.8)
    }
}

fn tone_color(tone: Tone) -> Color {
    match tone {
        Tone::Normal => TEXT_MUTED,
        Tone::Command => alpha(CYAN, 0.95),
        Tone::Success => alpha(GREEN, 0.95),
        Tone::Error => alpha(FAULT, 0.95),
        Tone::Muted => alpha(TEXT_MUTED, 0.7),
    }
}

pub fn status_line(text: impl Into<String>, tone: Tone) -> StatusMessage {
    StatusMessage {
        text: text.into(),
        tone,
        age: 0.0,
    }
}

pub fn elided_path(path: &Path, max: usize) -> String {
    elide(&path.to_string_lossy(), max)
}

pub fn gap() -> f32 {
    HUD_GAP
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_height_resizes_and_clamps() {
        let mut console = ConsoleState::default();
        assert_eq!(console.visible_lines, CONSOLE_LINES);
        console.resize(2);
        assert_eq!(console.visible_lines, CONSOLE_LINES + 2);
        console.resize(1000);
        assert_eq!(console.visible_lines, CONSOLE_MAX_LINES);
        console.resize(-1000);
        assert_eq!(console.visible_lines, CONSOLE_MIN_LINES);
    }

    #[test]
    fn configured_console_bounds_are_respected() {
        let mut console = ConsoleState::configured(40, 6, 48);
        assert_eq!(console.visible_lines, 40);
        console.resize(1000);
        assert_eq!(console.visible_lines, 48);
        console.resize(-1000);
        assert_eq!(console.visible_lines, 6);
    }

    #[test]
    fn console_height_fits_the_window() {
        assert_eq!(fit_console_lines(12, 800.0), 12);
        assert_eq!(fit_console_lines(48, 800.0), 36);
        assert!(fit_console_lines(1000, 300.0) >= 1);
    }
}
