use macroquad::prelude::{draw_line, draw_rectangle, draw_rectangle_lines, Color};

use super::{cell_size, WindowId, WindowManager, WindowStatus};
use crate::render::text::{Align, Fonts};
use crate::theme::{
    alpha, ansi_color, AMBER, CYAN, CYAN_DIM, DEEP_PANEL, FAULT, FONT_TINY, GREEN, ICE, TEXT_MUTED,
    VOID, WM_CURSOR_BLINK_HZ, WM_DEFAULT_BG, WM_DEFAULT_FG, WM_TERM_FONT, WM_TITLE_HEIGHT,
};

pub fn draw(fonts: &Fonts, manager: &WindowManager, time: f32, layout_window: Option<WindowId>) {
    for id in manager.render_order() {
        if let Some(window) = manager.window(id) {
            draw_window(
                fonts,
                window,
                manager.focused_id() == Some(id),
                time,
                layout_window == Some(id),
            );
        }
    }
}

fn draw_window(
    fonts: &Fonts,
    window: &super::TerminalWindow,
    focused: bool,
    time: f32,
    layout_mode: bool,
) {
    let rect = window.rect;
    draw_rectangle(
        rect.x + 5.0,
        rect.y + 5.0,
        rect.w,
        rect.h,
        alpha(VOID, 0.55),
    );
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, alpha(DEEP_PANEL, 0.98));
    let border = if focused {
        AMBER
    } else {
        alpha(CYAN_DIM, 0.85)
    };
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.5, border);
    if focused {
        draw_rectangle_lines(
            rect.x - 3.0,
            rect.y - 3.0,
            rect.w + 6.0,
            rect.h + 6.0,
            1.0,
            alpha(AMBER, 0.35),
        );
    }

    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        WM_TITLE_HEIGHT,
        if focused {
            alpha(AMBER, 0.16)
        } else {
            alpha(CYAN_DIM, 0.2)
        },
    );
    draw_line(
        rect.x,
        rect.y + WM_TITLE_HEIGHT,
        rect.right(),
        rect.y + WM_TITLE_HEIGHT,
        1.0,
        alpha(border, 0.7),
    );

    let close = window.close_rect();
    draw_rectangle_lines(close.x, close.y, close.w, close.h, 1.0, alpha(border, 0.8));
    fonts.draw(
        "X",
        close.x + close.w * 0.5,
        close.y + 3.0,
        FONT_TINY,
        if focused { AMBER } else { TEXT_MUTED },
        true,
        Align::Center,
    );

    let hint = if layout_mode {
        "F7 DONE  ·  ARROWS MOVE  ·  SHIFT+ARROWS RESIZE"
    } else if focused {
        "F7 LAYOUT  ·  F6 NEXT  ·  F12 RELEASE  ·  SHIFT+F12 CLOSE"
    } else {
        ""
    };
    let hint_width = if hint.is_empty() {
        0.0
    } else {
        fonts.width(hint, FONT_TINY, true) + 16.0
    };
    let status = window.status.label();
    let status_color = match &window.status {
        WindowStatus::Running => alpha(GREEN, 0.9),
        WindowStatus::Exited { success: true, .. } => TEXT_MUTED,
        _ => FAULT,
    };
    let scroll_label = if window.scrollback() > 0 {
        format!("  ↑{}", window.scrollback())
    } else {
        String::new()
    };
    let shown_status = format!("{status}{scroll_label}");
    fonts.draw(
        &shown_status,
        close.x - 10.0,
        rect.y + 7.0,
        FONT_TINY,
        status_color,
        false,
        Align::Right,
    );
    if !hint.is_empty() && rect.w > hint_width + 260.0 {
        fonts.draw(
            hint,
            close.x - 10.0 - fonts.width(&shown_status, FONT_TINY, false) - 16.0,
            rect.y + 7.0,
            FONT_TINY,
            alpha(AMBER, 0.75),
            true,
            Align::Right,
        );
    }

    let title = format!("TERM // {}", window.title);
    let title_limit = if hint.is_empty() || rect.w <= hint_width + 260.0 {
        close.x - 10.0 - fonts.width(&shown_status, FONT_TINY, false) - 20.0 - rect.x
    } else {
        close.x - 10.0 - fonts.width(&shown_status, FONT_TINY, false) - 16.0 - hint_width - rect.x
    };
    fonts.draw_elided(
        &title,
        rect.x + 10.0,
        rect.y + 7.0,
        FONT_TINY,
        title_limit.max(40.0),
        ICE,
        true,
        Align::Left,
    );

    let content = window.content_rect();
    draw_rectangle(content.x, content.y, content.w, content.h, WM_DEFAULT_BG);
    let (cell_width, cell_height) = cell_size(fonts);
    draw_terminal(
        fonts,
        window,
        content,
        cell_width,
        cell_height,
        focused,
        time,
    );

    draw_line(
        rect.right() - 12.0,
        rect.bottom() - 4.0,
        rect.right() - 4.0,
        rect.bottom() - 12.0,
        1.0,
        alpha(border, 0.55),
    );
    draw_line(
        rect.right() - 8.0,
        rect.bottom() - 4.0,
        rect.right() - 4.0,
        rect.bottom() - 8.0,
        1.0,
        alpha(border, 0.55),
    );
}

fn draw_terminal(
    fonts: &Fonts,
    window: &super::TerminalWindow,
    content: super::WindowRect,
    cell_width: f32,
    cell_height: f32,
    focused: bool,
    time: f32,
) {
    let screen = window.screen();
    let (rows, cols) = screen.size();
    let rows = rows.get().min(window.rows.max(1));
    let cols = cols.get().min(window.cols.max(1));

    for row in 0..rows {
        let mut col = 0;
        while col < cols {
            let Some(cell) = screen.cell(row, col) else {
                break;
            };
            let style = cell_style(cell);
            let start = col;
            let mut text = String::new();
            while col < cols {
                let Some(cell) = screen.cell(row, col) else {
                    break;
                };
                if cell_style(cell) != style {
                    break;
                }
                if !cell.is_wide_continuation() {
                    let contents = cell.contents();
                    if contents.is_empty() {
                        text.push(' ');
                    } else {
                        text.push_str(contents);
                    }
                }
                col += 1;
            }

            let x = content.x + start as f32 * cell_width;
            let y = content.y + row as f32 * cell_height;
            let width = (col - start) as f32 * cell_width;
            if style.bg != WM_DEFAULT_BG {
                draw_rectangle(x, y, width, cell_height, style.bg);
            }
            if text.chars().any(|character| character != ' ') {
                let color = if style.dim {
                    alpha(style.fg, 0.6)
                } else {
                    style.fg
                };
                fonts.draw(&text, x, y, WM_TERM_FONT, color, style.bold, Align::Left);
            }
            if style.underline {
                draw_line(
                    x,
                    y + cell_height - 2.0,
                    x + width,
                    y + cell_height - 2.0,
                    1.0,
                    alpha(style.fg, 0.8),
                );
            }
        }
    }

    if focused
        && window.scrollback() == 0
        && !screen.hide_cursor()
        && ((time * WM_CURSOR_BLINK_HZ) as i32) % 2 == 0
    {
        let (row, col) = screen.cursor_position();
        if row < rows && col < cols {
            let x = content.x + col as f32 * cell_width;
            let y = content.y + row as f32 * cell_height;
            draw_rectangle(x, y, cell_width, cell_height, alpha(CYAN, 0.28));
            draw_rectangle_lines(x, y, cell_width, cell_height, 1.0, alpha(CYAN, 0.75));
        }
    }

    if window.status.is_running() {
        draw_rectangle(
            content.x,
            content.y + content.h - 1.0,
            content.w,
            1.0,
            alpha(GREEN, 0.08),
        );
    } else {
        draw_rectangle(
            content.x,
            content.y + content.h - 1.0,
            content.w,
            1.0,
            alpha(FAULT, 0.18),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CellStyle {
    fg: Color,
    bg: Color,
    bold: bool,
    underline: bool,
    dim: bool,
}

fn cell_style(cell: &vt100::Cell) -> CellStyle {
    let mut fg = terminal_color(cell.fgcolor(), WM_DEFAULT_FG);
    let mut bg = terminal_color(cell.bgcolor(), WM_DEFAULT_BG);
    if cell.inverse() {
        std::mem::swap(&mut fg, &mut bg);
    }
    CellStyle {
        fg,
        bg,
        bold: cell.bold(),
        underline: cell.underline(),
        dim: cell.dim(),
    }
}

fn terminal_color(color: vt100::Color, default: Color) -> Color {
    match color {
        vt100::Color::Default => default,
        vt100::Color::Idx(index) => ansi_color(index),
        vt100::Color::Rgb(red, green, blue) => Color::from_rgba(red, green, blue, 0xff),
    }
}
