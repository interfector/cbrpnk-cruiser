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

pub const GALAXY_RADIUS_SCALE: f32 = 3.9;
pub const GALAXY_ORBIT_SEGMENTS: usize = 40;
pub const MAX_GALAXIES: usize = 96;
pub const AUTO_ENTER_FACTOR: f32 = 0.78;
pub const AUTO_ENTER_GUARD: f32 = 1.25;

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
pub const CONSOLE_BUFFER: usize = 300;
pub const STATUS_LIFETIME: f32 = 7.0;

pub const CAMERA_MIN: f32 = -1.0;
pub const CAMERA_MAX: f32 = 16.0;
pub const CAMERA_RADIUS: f32 = 28.0;
