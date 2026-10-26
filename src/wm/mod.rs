pub mod input;
mod pty;
pub mod render;

use std::num::NonZeroU16;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::launch::process::plan_executable;
use crate::render::text::Fonts;
use crate::theme::{
    PAD, WM_CASCADE_STEP, WM_DEFAULT_HEIGHT, WM_DEFAULT_WIDTH, WM_MAX_WINDOWS, WM_MIN_HEIGHT,
    WM_MIN_WIDTH, WM_PADDING, WM_SCROLLBACK, WM_START_TOP, WM_TERM_FONT, WM_TITLE_HEIGHT,
};
use pty::{PtyEvent, PtySession};

pub type WindowId = u64;

#[derive(Debug, Clone)]
pub enum WindowStatus {
    Running,
    Exited {
        success: bool,
        code: Option<i32>,
        signal: Option<String>,
    },
    Failed(String),
}

impl WindowStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    pub fn is_exited(&self) -> bool {
        matches!(self, Self::Exited { .. })
    }

    pub fn label(&self) -> String {
        match self {
            Self::Running => "RUNNING".to_string(),
            Self::Exited { success: true, .. } => "EXIT 0".to_string(),
            Self::Exited {
                success: false,
                code,
                signal,
            } => match (code, signal) {
                (_, Some(signal)) => format!("SIGNAL {signal}"),
                (Some(code), None) => format!("EXIT {code}"),
                (None, None) => "EXITED".to_string(),
            },
            Self::Failed(_) => "IO ERROR".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl WindowRect {
    pub fn contains(self, point: (f32, f32)) -> bool {
        point.0 >= self.x
            && point.0 <= self.x + self.w
            && point.1 >= self.y
            && point.1 <= self.y + self.h
    }

    pub fn right(self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(self) -> f32 {
        self.y + self.h
    }
}

struct WindowSpec {
    title: String,
    cwd: PathBuf,
    rect: WindowRect,
    rows: u16,
    cols: u16,
    session: PtySession,
    auto_close_on_exit: bool,
}

pub struct TerminalWindow {
    pub id: WindowId,
    pub title: String,
    pub cwd: PathBuf,
    pub rect: WindowRect,
    pub rows: u16,
    pub cols: u16,
    pub status: WindowStatus,
    parser: vt100::Parser,
    session: PtySession,
    scrollback: usize,
    last_size: (u16, u16),
    auto_close_on_exit: bool,
}

impl TerminalWindow {
    fn new(id: WindowId, spec: WindowSpec) -> Self {
        let rows = spec.rows.max(1);
        let cols = spec.cols.max(1);
        Self {
            id,
            title: spec.title,
            cwd: spec.cwd,
            rect: spec.rect,
            rows,
            cols,
            status: WindowStatus::Running,
            parser: vt100::Parser::new(nonzero(rows), nonzero(cols), WM_SCROLLBACK),
            session: spec.session,
            scrollback: 0,
            last_size: (rows, cols),
            auto_close_on_exit: spec.auto_close_on_exit,
        }
    }

    pub fn pid(&self) -> Option<u32> {
        self.session.pid()
    }

    pub fn title_rect(&self) -> WindowRect {
        WindowRect {
            x: self.rect.x,
            y: self.rect.y,
            w: self.rect.w,
            h: WM_TITLE_HEIGHT,
        }
    }

    pub fn content_rect(&self) -> WindowRect {
        content_geometry(self.rect)
    }

    pub fn close_rect(&self) -> WindowRect {
        let size = WM_TITLE_HEIGHT - 8.0;
        WindowRect {
            x: self.rect.right() - WM_PADDING - size,
            y: self.rect.y + 4.0,
            w: size,
            h: size,
        }
    }

    pub fn resize_rect(&self) -> WindowRect {
        let handle = crate::theme::WM_RESIZE_HANDLE;
        WindowRect {
            x: self.rect.right() - handle,
            y: self.rect.bottom() - handle,
            w: handle,
            h: handle,
        }
    }

    pub fn scrollback(&self) -> usize {
        self.scrollback
    }

    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    pub fn write(&self, bytes: &[u8]) -> bool {
        self.session.write(bytes)
    }

    pub fn scroll_up(&mut self, rows: usize) {
        let max = self.parser.screen().scrollback_len();
        self.scrollback = (self.scrollback + rows).min(max);
        self.apply_scrollback();
    }

    pub fn scroll_down(&mut self, rows: usize) {
        self.scrollback = self.scrollback.saturating_sub(rows);
        self.apply_scrollback();
    }

    fn apply_scrollback(&mut self) {
        self.parser.screen_mut().set_scrollback(self.scrollback);
    }

    fn poll_events(&mut self) {
        while let Some(event) = self.session.try_recv() {
            match event {
                PtyEvent::Output(bytes) => self.parser.process(&bytes),
                PtyEvent::Exited {
                    success,
                    code,
                    signal,
                } => {
                    self.status = WindowStatus::Exited {
                        success,
                        code,
                        signal,
                    };
                }
                PtyEvent::Error(message) => {
                    self.status = WindowStatus::Failed(message);
                }
            }
        }
        let max = self.parser.screen().scrollback_len();
        self.scrollback = self.scrollback.min(max);
        if self.scrollback > 0 {
            self.apply_scrollback();
        }
    }

    fn sync_size(&mut self, cell_width: f32, cell_height: f32) {
        let (rows, cols) = fit_terminal_grid(
            self.content_rect().w,
            self.content_rect().h,
            cell_width,
            cell_height,
        );
        if (rows, cols) == self.last_size {
            return;
        }
        self.last_size = (rows, cols);
        self.rows = rows;
        self.cols = cols;
        self.parser.set_size(nonzero(rows), nonzero(cols));
        self.session.resize(rows, cols);
        let max = self.parser.screen().scrollback_len();
        self.scrollback = self.scrollback.min(max);
        self.apply_scrollback();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragKind {
    Move,
    Resize,
}

#[derive(Debug, Clone, Copy)]
struct Drag {
    id: WindowId,
    kind: DragKind,
    grab: (f32, f32),
    start_mouse: (f32, f32),
    start_rect: WindowRect,
}

#[derive(Debug, Clone)]
pub struct ClosedWindow {
    pub id: WindowId,
    pub title: String,
    pub success: bool,
}

#[derive(Default)]
pub struct WindowManager {
    windows: Vec<TerminalWindow>,
    focused: Option<WindowId>,
    next_id: WindowId,
    drag: Option<Drag>,
}

impl WindowManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    pub fn focused_id(&self) -> Option<WindowId> {
        self.focused
    }

    pub fn has_focus(&self) -> bool {
        self.focused.is_some()
    }

    pub fn window(&self, id: WindowId) -> Option<&TerminalWindow> {
        self.windows.iter().find(|window| window.id == id)
    }

    pub fn window_mut(&mut self, id: WindowId) -> Option<&mut TerminalWindow> {
        self.windows.iter_mut().find(|window| window.id == id)
    }

    pub fn open_shell(
        &mut self,
        shell: Option<&Path>,
        cwd: &Path,
        fonts: &Fonts,
    ) -> Result<WindowId> {
        self.check_limit()?;
        let geometry = self.next_geometry();
        let (rows, cols) = grid_for(geometry, fonts);
        let session = PtySession::spawn_shell(shell, cwd, rows, cols)?;
        let title = shell
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "shell".to_string());
        Ok(self.push_window(WindowSpec {
            title,
            cwd: cwd.to_path_buf(),
            rect: geometry,
            rows,
            cols,
            session,
            auto_close_on_exit: true,
        }))
    }

    pub fn open_executable(&mut self, path: &Path, cwd: &Path, fonts: &Fonts) -> Result<WindowId> {
        self.check_limit()?;
        let plan = plan_executable(path, cwd, Vec::new())?;
        let geometry = self.next_geometry();
        let (rows, cols) = grid_for(geometry, fonts);
        let session = PtySession::spawn_program(&plan.program, &plan.args, &plan.cwd, rows, cols)?;
        let title = plan
            .program
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "program".to_string());
        Ok(self.push_window(WindowSpec {
            title,
            cwd: plan.cwd,
            rect: geometry,
            rows,
            cols,
            session,
            auto_close_on_exit: false,
        }))
    }

    fn push_window(&mut self, spec: WindowSpec) -> WindowId {
        self.next_id = self.next_id.wrapping_add(1);
        let id = self.next_id;
        self.windows.push(TerminalWindow::new(id, spec));
        self.focused = Some(id);
        id
    }

    fn check_limit(&self) -> Result<()> {
        if self.windows.len() >= WM_MAX_WINDOWS {
            bail!("terminal window limit reached ({WM_MAX_WINDOWS})");
        }
        Ok(())
    }

    fn next_geometry(&self) -> WindowRect {
        cascade_geometry(
            self.windows.len(),
            macroquad::prelude::screen_width(),
            macroquad::prelude::screen_height(),
        )
    }

    pub fn poll(&mut self) {
        for window in &mut self.windows {
            window.poll_events();
        }
    }

    pub fn close_exited_shells(&mut self) -> Vec<ClosedWindow> {
        let closed: Vec<ClosedWindow> = self
            .windows
            .iter()
            .filter(|window| should_auto_close(window.auto_close_on_exit, &window.status))
            .map(|window| ClosedWindow {
                id: window.id,
                title: window.title.clone(),
                success: matches!(window.status, WindowStatus::Exited { success: true, .. }),
            })
            .collect();
        for window in &closed {
            self.close(window.id);
        }
        closed
    }

    pub fn sync_sizes(&mut self, fonts: &Fonts) {
        let (cell_width, cell_height) = cell_size(fonts);
        for window in &mut self.windows {
            window.sync_size(cell_width, cell_height);
        }
    }

    pub fn focus(&mut self, id: WindowId) {
        if self.window(id).is_some() {
            self.focused = Some(id);
        }
    }

    pub fn release_focus(&mut self) {
        self.focused = None;
        self.drag = None;
    }

    pub fn focus_next(&mut self) -> Option<WindowId> {
        if self.windows.is_empty() {
            self.focused = None;
            return None;
        }
        let current = self
            .focused
            .and_then(|id| self.windows.iter().position(|window| window.id == id));
        let next = next_index(current, self.windows.len());
        let id = self.windows[next].id;
        self.focused = Some(id);
        Some(id)
    }

    pub fn close(&mut self, id: WindowId) -> bool {
        let Some(index) = self.windows.iter().position(|window| window.id == id) else {
            return false;
        };
        self.windows.remove(index);
        if self.drag.is_some_and(|drag| drag.id == id) {
            self.drag = None;
        }
        if self.focused == Some(id) {
            self.focused = if self.windows.is_empty() {
                None
            } else {
                Some(self.windows[index.min(self.windows.len() - 1)].id)
            };
        }
        true
    }

    pub fn hit_test(&self, point: (f32, f32)) -> Option<WindowId> {
        if let Some(id) = self.focused {
            if self
                .window(id)
                .is_some_and(|window| window.rect.contains(point))
            {
                return Some(id);
            }
        }
        self.windows
            .iter()
            .rev()
            .find(|window| Some(window.id) != self.focused && window.rect.contains(point))
            .map(|window| window.id)
    }

    pub fn begin_drag(&mut self, id: WindowId, kind: DragKind, point: (f32, f32)) {
        let Some(window) = self.window(id) else {
            return;
        };
        let grab = (point.0 - window.rect.x, point.1 - window.rect.y);
        self.drag = Some(Drag {
            id,
            kind,
            grab,
            start_mouse: point,
            start_rect: window.rect,
        });
    }

    pub fn update_drag(&mut self, point: (f32, f32)) {
        let Some(drag) = self.drag else {
            return;
        };
        let Some(window) = self.windows.iter_mut().find(|window| window.id == drag.id) else {
            self.drag = None;
            return;
        };
        let screen_width = macroquad::prelude::screen_width();
        let screen_height = macroquad::prelude::screen_height();
        match drag.kind {
            DragKind::Move => {
                let max_x = (screen_width - PAD - window.rect.w).max(PAD);
                let max_y = (screen_height - PAD - window.rect.h).max(PAD);
                window.rect.x = (point.0 - drag.grab.0).clamp(PAD, max_x);
                window.rect.y = (point.1 - drag.grab.1).clamp(PAD, max_y);
            }
            DragKind::Resize => {
                let min_width = WM_MIN_WIDTH.min((screen_width - PAD * 2.0).max(180.0));
                let min_height = WM_MIN_HEIGHT.min((screen_height - PAD * 2.0).max(120.0));
                let max_width = (screen_width - PAD - window.rect.x).max(min_width);
                let max_height = (screen_height - PAD - window.rect.y).max(min_height);
                let width = (drag.start_rect.w + (point.0 - drag.start_mouse.0))
                    .clamp(min_width, max_width);
                let height = (drag.start_rect.h + (point.1 - drag.start_mouse.1))
                    .clamp(min_height, max_height);
                window.rect.w = width;
                window.rect.h = height;
            }
        }
    }

    pub fn nudge(&mut self, id: WindowId, dx: f32, dy: f32) {
        let screen_width = macroquad::prelude::screen_width();
        let screen_height = macroquad::prelude::screen_height();
        let Some(window) = self.window_mut(id) else {
            return;
        };
        let max_x = (screen_width - PAD - window.rect.w).max(PAD);
        let max_y = (screen_height - PAD - window.rect.h).max(PAD);
        window.rect.x = (window.rect.x + dx).clamp(PAD, max_x);
        window.rect.y = (window.rect.y + dy).clamp(PAD, max_y);
    }

    pub fn resize_by(&mut self, id: WindowId, dx: f32, dy: f32) {
        let screen_width = macroquad::prelude::screen_width();
        let screen_height = macroquad::prelude::screen_height();
        let Some(window) = self.window_mut(id) else {
            return;
        };
        let min_width = WM_MIN_WIDTH.min((screen_width - PAD * 2.0).max(180.0));
        let min_height = WM_MIN_HEIGHT.min((screen_height - PAD * 2.0).max(120.0));
        let max_width = (screen_width - PAD - window.rect.x).max(min_width);
        let max_height = (screen_height - PAD - window.rect.y).max(min_height);
        window.rect.w = (window.rect.w + dx).clamp(min_width, max_width);
        window.rect.h = (window.rect.h + dy).clamp(min_height, max_height);
    }

    pub fn end_drag(&mut self) {
        self.drag = None;
    }

    pub fn drag_id(&self) -> Option<WindowId> {
        self.drag.map(|drag| drag.id)
    }

    pub fn render_order(&self) -> Vec<WindowId> {
        let mut order: Vec<WindowId> = self
            .windows
            .iter()
            .filter(|window| Some(window.id) != self.focused)
            .map(|window| window.id)
            .collect();
        if let Some(focused) = self.focused {
            if self.window(focused).is_some() {
                order.push(focused);
            }
        }
        order
    }
}

fn should_auto_close(auto_close_on_exit: bool, status: &WindowStatus) -> bool {
    auto_close_on_exit && status.is_exited()
}

fn nonzero(value: u16) -> NonZeroU16 {
    NonZeroU16::new(value).unwrap_or(NonZeroU16::MIN)
}

fn cell_size(fonts: &Fonts) -> (f32, f32) {
    (
        fonts.width("M", WM_TERM_FONT, false).max(1.0),
        fonts.line_height(WM_TERM_FONT).max(1.0),
    )
}

fn content_geometry(rect: WindowRect) -> WindowRect {
    WindowRect {
        x: rect.x + WM_PADDING,
        y: rect.y + WM_TITLE_HEIGHT,
        w: (rect.w - WM_PADDING * 2.0).max(1.0),
        h: (rect.h - WM_TITLE_HEIGHT - WM_PADDING).max(1.0),
    }
}

fn grid_for(rect: WindowRect, fonts: &Fonts) -> (u16, u16) {
    let content = content_geometry(rect);
    let (cell_width, cell_height) = cell_size(fonts);
    fit_terminal_grid(content.w, content.h, cell_width, cell_height)
}

fn fit_terminal_grid(width: f32, height: f32, cell_width: f32, cell_height: f32) -> (u16, u16) {
    let cols = (width / cell_width.max(1.0)).floor().clamp(1.0, 1000.0) as u16;
    let rows = (height / cell_height.max(1.0)).floor().clamp(1.0, 1000.0) as u16;
    (rows, cols)
}

fn next_index(current: Option<usize>, length: usize) -> usize {
    match current {
        Some(index) if index + 1 < length => index + 1,
        Some(_) => 0,
        None => length.saturating_sub(1),
    }
}

fn cascade_geometry(index: usize, screen_width: f32, screen_height: f32) -> WindowRect {
    let available_width = (screen_width - PAD * 2.0).max(200.0);
    let available_height = (screen_height - PAD * 2.0).max(150.0);
    let width = WM_DEFAULT_WIDTH.min(available_width);
    let height = WM_DEFAULT_HEIGHT.min(available_height);
    let offset = (index % 6) as f32 * WM_CASCADE_STEP;
    let max_x = (screen_width - PAD - width).max(PAD);
    let max_y = (screen_height - PAD - height).max(PAD);
    WindowRect {
        x: (PAD + offset).min(max_x),
        y: (WM_START_TOP + offset).min(max_y),
        w: width,
        h: height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shells_auto_close_only_after_exit() {
        let running = WindowStatus::Running;
        let exited = WindowStatus::Exited {
            success: true,
            code: Some(0),
            signal: None,
        };
        let failed = WindowStatus::Failed("read failed".to_string());
        assert!(!should_auto_close(true, &running));
        assert!(should_auto_close(true, &exited));
        assert!(!should_auto_close(true, &failed));
        assert!(!should_auto_close(false, &exited));
    }

    #[test]
    fn content_geometry_leaves_title_and_padding() {
        let rect = WindowRect {
            x: 100.0,
            y: 200.0,
            w: 500.0,
            h: 300.0,
        };
        let content = content_geometry(rect);
        assert_eq!(content.x, 100.0 + WM_PADDING);
        assert_eq!(content.y, 200.0 + WM_TITLE_HEIGHT);
        assert_eq!(content.w, 500.0 - WM_PADDING * 2.0);
        assert_eq!(content.h, 300.0 - WM_TITLE_HEIGHT - WM_PADDING);
    }

    #[test]
    fn terminal_grid_has_minimum_one_cell() {
        assert_eq!(fit_terminal_grid(0.0, 0.0, 8.0, 16.0), (1, 1));
        assert_eq!(fit_terminal_grid(80.0, 64.0, 8.0, 16.0), (4, 10));
    }

    #[test]
    fn focus_cycle_wraps_and_starts_at_top() {
        assert_eq!(next_index(None, 3), 2);
        assert_eq!(next_index(Some(0), 3), 1);
        assert_eq!(next_index(Some(2), 3), 0);
    }

    #[test]
    fn cascade_stays_inside_small_screens() {
        let rect = cascade_geometry(5, 640.0, 480.0);
        assert!(rect.x >= PAD);
        assert!(rect.y >= PAD);
        assert!(rect.right() <= 640.0 - PAD + 0.01);
        assert!(rect.bottom() <= 480.0 - PAD + 0.01);
    }

    #[test]
    fn hit_testing_uses_rect_bounds() {
        let rect = WindowRect {
            x: 10.0,
            y: 20.0,
            w: 100.0,
            h: 50.0,
        };
        assert!(rect.contains((10.0, 20.0)));
        assert!(rect.contains((110.0, 70.0)));
        assert!(!rect.contains((9.9, 20.0)));
    }
}
