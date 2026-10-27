use macroquad::prelude::Color;

const fn rgb(hex: u32) -> Color {
    Color::from_rgba(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
        0xff,
    )
}

pub const VOID: Color = rgb(0x010409);
pub const DEEP_PANEL: Color = rgb(0x061018);
pub const GRID_DIM: Color = rgb(0x0b2b33);
pub const CYAN: Color = rgb(0x00e5ff);
pub const CYAN_DIM: Color = rgb(0x007a8a);
pub const GREEN: Color = rgb(0x39ff14);
pub const AMBER: Color = rgb(0xffb000);
pub const MAGENTA: Color = rgb(0xff2a6d);
pub const ICE: Color = rgb(0xe6ffff);
pub const TEXT_MUTED: Color = rgb(0x7fa6ad);
pub const FAULT: Color = rgb(0xff3b30);

pub fn alpha(color: Color, alpha: f32) -> Color {
    Color::new(color.r, color.g, color.b, alpha.clamp(0.0, 1.0))
}

pub const FONT_BODY: f32 = 16.0;
pub const FONT_SMALL: f32 = 13.0;
pub const FONT_TINY: f32 = 11.0;
pub const FONT_TITLE: f32 = 22.0;
pub const FONT_BOOT: f32 = 30.0;

pub const PAD: f32 = 14.0;
pub const HUD_GAP: f32 = 8.0;
pub const PANEL_ALPHA: f32 = 0.86;

pub const NODE_DIR_RADIUS: f32 = 0.28;
pub const NODE_EXEC_RADIUS: f32 = 0.22;
pub const NODE_FILE_RADIUS: f32 = 0.17;
pub const NODE_LINK_RADIUS: f32 = 0.22;
pub const NODE_OTHER_RADIUS: f32 = 0.15;

pub const GALAXY_RADIUS_MIN: f32 = 0.75;
pub const GALAXY_RADIUS_DEFAULT: f32 = 1.0;
pub const GALAXY_RADIUS_MAX: f32 = 2.15;
pub const GALAXY_COUNT_REFERENCE: f32 = 512.0;
pub const GALAXY_ORBIT_SEGMENTS: usize = 40;
pub const MAX_GALAXIES: usize = 96;
pub const MAX_DIRECTORY_COUNTS: usize = 128;
pub const AUTO_ENTER_FACTOR: f32 = 0.5;
pub const AUTO_ENTER_GUARD: f32 = 1.25;
pub const PARENT_GATE_ENTRY_RADIUS: f32 = 1.1;

#[derive(Clone, Copy)]
pub struct SectorBand {
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub height_center: f32,
    pub height_scale: f32,
}

pub const SECTOR_BANDS: [SectorBand; 5] = [
    SectorBand {
        inner_radius: 2.4,
        outer_radius: 5.6,
        height_center: 4.8,
        height_scale: 0.9,
    },
    SectorBand {
        inner_radius: 4.4,
        outer_radius: 8.4,
        height_center: 5.0,
        height_scale: 0.65,
    },
    SectorBand {
        inner_radius: 6.2,
        outer_radius: 11.8,
        height_center: 5.2,
        height_scale: 0.5,
    },
    SectorBand {
        inner_radius: 9.0,
        outer_radius: 15.2,
        height_center: 5.4,
        height_scale: 0.4,
    },
    SectorBand {
        inner_radius: 12.0,
        outer_radius: 18.0,
        height_center: 5.6,
        height_scale: 0.34,
    },
];

pub const SECTOR_RINGS: [(f32, f32, f32); 5] = [
    (2.8, 4.0, 0.16),
    (5.6, 4.8, -0.12),
    (9.2, 5.6, 0.18),
    (13.2, 6.4, -0.10),
    (17.4, 7.0, 0.12),
];

pub const FOV_Y: f32 = 64.0_f32.to_radians();
pub const NEAR_PLANE: f32 = 0.18;
pub const FAR_PLANE: f32 = 120.0;

pub const FOCUS_TOLERANCE_PX: f32 = 190.0;
pub const POINTER_STEER_BOUND: f32 = 0.8;
pub const POINTER_DEADZONE: f32 = 0.035;
pub const MAX_VISIBLE_LABELS: usize = 150;
pub const SCANLINE_ALPHA: f32 = 0.05;

pub const BOOT_SECONDS: f32 = 1.15;
pub const CONSOLE_LINES: usize = 12;
pub const CONSOLE_MIN_LINES: usize = 4;
pub const CONSOLE_MAX_LINES: usize = 24;
pub const CONSOLE_RESIZE_STEP: usize = 2;
pub const CONSOLE_LINE_HEIGHT: f32 = 18.0;
pub const CONSOLE_TOP_RESERVE: f32 = 48.0;
pub const CONSOLE_BUFFER: usize = 300;
pub const STATUS_LIFETIME: f32 = 7.0;

pub const CAMERA_MIN: f32 = -1.0;
pub const CAMERA_MAX: f32 = 16.0;
pub const CAMERA_RADIUS: f32 = 28.0;

pub const WM_MAX_WINDOWS: usize = 8;
pub const WM_DEFAULT_WIDTH: f32 = 760.0;
pub const WM_DEFAULT_HEIGHT: f32 = 430.0;
pub const WM_MIN_WIDTH: f32 = 340.0;
pub const WM_MIN_HEIGHT: f32 = 180.0;
pub const WM_TITLE_HEIGHT: f32 = 28.0;
pub const WM_PADDING: f32 = 8.0;
pub const WM_TERM_FONT: f32 = 13.0;
pub const WM_SCROLLBACK: usize = 2000;
pub const WM_CASCADE_STEP: f32 = 32.0;
pub const WM_START_TOP: f32 = 100.0;
pub const WM_RESIZE_HANDLE: f32 = 16.0;
pub const WM_CURSOR_BLINK_HZ: f32 = 1.8;
pub const WM_DEFAULT_FG: Color = ICE;
pub const WM_DEFAULT_BG: Color = DEEP_PANEL;
pub const WM_FOCUSED_OPACITY: f32 = 0.98;
pub const WM_UNFOCUSED_OPACITY: f32 = 0.70;

pub fn ansi_color(index: u8) -> Color {
    match index {
        0 => VOID,
        1 => FAULT,
        2 => GREEN,
        3 => AMBER,
        4 => CYAN_DIM,
        5 => MAGENTA,
        6 => CYAN,
        7 => ICE,
        8 => GRID_DIM,
        9 => rgb(0xff7a72),
        10 => rgb(0x7dff6a),
        11 => rgb(0xffd166),
        12 => rgb(0x5ab7ff),
        13 => rgb(0xff7ac8),
        14 => rgb(0x7af5ff),
        15 => rgb(0xffffff),
        16..=231 => {
            let value = index - 16;
            let r = value / 36;
            let g = (value % 36) / 6;
            let b = value % 6;
            let channel = |part: u8| if part == 0 { 0 } else { 55 + part * 40 };
            Color::from_rgba(channel(r), channel(g), channel(b), 0xff)
        }
        _ => {
            let level = 8 + (index - 232) * 10;
            Color::from_rgba(level, level, level, 0xff)
        }
    }
}
