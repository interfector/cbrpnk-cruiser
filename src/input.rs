use std::sync::atomic::{AtomicBool, Ordering};

use macroquad::prelude::{
    get_char_pressed, get_last_key_pressed, is_key_down, is_key_pressed, is_mouse_button_down,
    is_mouse_button_pressed, mouse_delta_position, mouse_position, screen_height, screen_width,
    KeyCode, MouseButton,
};

use crate::camera::CameraInput;
use crate::theme::{POINTER_DEADZONE, POINTER_STEER_BOUND};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Activate,
    Parent,
    Home,
    Root,
    Rescan,
    ToggleHidden,
    ToggleLock,
    NextNode,
    PrevNode,
    Open,
    TerminalWindow,
    RunInWindow,
    CycleWindow,
    Switcher,
    Console,
    Filter,
    Recenter,
    Freeze,
    Help,
    Fullscreen,
    Quit,
}

pub fn ctrl_down() -> bool {
    is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl)
}

pub fn shift_down() -> bool {
    is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)
}

pub fn any_input_pressed() -> bool {
    get_last_key_pressed().is_some()
        || is_mouse_button_pressed(MouseButton::Left)
        || is_mouse_button_pressed(MouseButton::Right)
}

pub fn pointer_visible(auto_hide_mouse: bool, window_focused: bool) -> bool {
    !auto_hide_mouse || window_focused
}

pub fn text_shortcut(character: char) -> Option<Intent> {
    match character {
        ':' => Some(Intent::Console),
        '?' => Some(Intent::Help),
        '/' => Some(Intent::Filter),
        '~' => Some(Intent::Home),
        _ => None,
    }
}

pub fn next_text_shortcut() -> Option<Intent> {
    while let Some(character) = get_char_pressed() {
        if let Some(intent) = text_shortcut(character) {
            return Some(intent);
        }
    }
    None
}

pub fn consume_help_close() -> bool {
    while let Some(character) = get_char_pressed() {
        if character == '?' {
            return true;
        }
    }
    false
}

pub fn cruise_intent(locked: bool) -> Option<Intent> {
    if ctrl_down() && is_key_pressed(KeyCode::Q) {
        return Some(Intent::Quit);
    }
    if is_key_pressed(KeyCode::Escape) {
        return Some(if locked {
            Intent::ToggleLock
        } else {
            Intent::Quit
        });
    }
    if is_key_pressed(KeyCode::F1) || (shift_down() && is_key_pressed(KeyCode::Slash)) {
        return Some(Intent::Help);
    }
    if is_key_pressed(KeyCode::F11)
        || (is_key_down(KeyCode::LeftAlt) && is_key_pressed(KeyCode::Enter))
        || (is_key_down(KeyCode::RightAlt) && is_key_pressed(KeyCode::Enter))
    {
        return Some(Intent::Fullscreen);
    }
    if ctrl_down() && is_key_pressed(KeyCode::H) {
        return Some(Intent::ToggleHidden);
    }
    if ctrl_down() && is_key_pressed(KeyCode::T) {
        return Some(Intent::TerminalWindow);
    }
    if ctrl_down() && is_key_pressed(KeyCode::Enter) {
        return Some(Intent::RunInWindow);
    }
    if is_key_pressed(KeyCode::F6) {
        return Some(Intent::CycleWindow);
    }
    if is_key_pressed(KeyCode::F5) {
        return Some(Intent::Rescan);
    }
    if is_key_pressed(KeyCode::Enter) {
        return Some(Intent::Activate);
    }
    if is_key_pressed(KeyCode::Backspace) || is_key_pressed(KeyCode::H) {
        return Some(Intent::Parent);
    }
    if is_key_pressed(KeyCode::Home) || (shift_down() && is_key_pressed(KeyCode::GraveAccent)) {
        return Some(Intent::Home);
    }
    if is_key_pressed(KeyCode::Tab) {
        return Some(Intent::ToggleLock);
    }
    if is_key_pressed(KeyCode::J) {
        return Some(Intent::PrevNode);
    }
    if is_key_pressed(KeyCode::K) {
        return Some(Intent::NextNode);
    }
    if is_key_pressed(KeyCode::O) {
        return Some(Intent::Open);
    }
    if shift_down() && is_key_pressed(KeyCode::Semicolon) {
        return Some(Intent::Console);
    }
    if is_key_pressed(KeyCode::Slash) && !shift_down() {
        return Some(Intent::Filter);
    }
    if ctrl_down() && is_key_pressed(KeyCode::P) {
        return Some(Intent::Switcher);
    }
    if is_key_pressed(KeyCode::R) {
        return Some(Intent::Recenter);
    }
    if is_key_pressed(KeyCode::F) {
        return Some(Intent::Freeze);
    }
    None
}

static POINTER_STEERING_ARMED: AtomicBool = AtomicBool::new(false);

pub fn camera_input(locked: bool) -> CameraInput {
    let mut input = CameraInput {
        forward: axis(KeyCode::S, KeyCode::W),
        strafe: axis(KeyCode::A, KeyCode::D),
        vertical: axis(KeyCode::Q, KeyCode::E),
        yaw: 0.0,
        pitch: 0.0,
        boost: shift_down(),
        brake: ctrl_down(),
    };
    let pointer = mouse_position();
    let width = screen_width();
    let height = screen_height();
    let pointer_inside =
        pointer.0 >= 0.0 && pointer.1 >= 0.0 && pointer.0 <= width && pointer.1 <= height;
    if !pointer_inside {
        POINTER_STEERING_ARMED.store(false, Ordering::Relaxed);
    } else if mouse_delta_position().length_squared() > 0.000_001
        || is_mouse_button_down(MouseButton::Left)
        || is_mouse_button_down(MouseButton::Right)
    {
        POINTER_STEERING_ARMED.store(true, Ordering::Relaxed);
    }

    if pointer_inside {
        let mouse_thrust = f32::from(is_mouse_button_down(MouseButton::Left))
            - f32::from(is_mouse_button_down(MouseButton::Right));
        input.forward = (input.forward + mouse_thrust).clamp(-1.0, 1.0);
    }
    if !locked && POINTER_STEERING_ARMED.load(Ordering::Relaxed) {
        let pointer = pointer_steer(pointer, width, height);
        input.yaw = (axis(KeyCode::Left, KeyCode::Right) + pointer.0).clamp(-1.0, 1.0);
        input.pitch = (axis(KeyCode::Down, KeyCode::Up) - pointer.1).clamp(-1.0, 1.0);
    }
    if is_key_down(KeyCode::Space) {
        input.forward = (input.forward + 1.0).clamp(-1.0, 1.0);
    }
    input
}

pub fn pointer_steer(pointer: (f32, f32), width: f32, height: f32) -> (f32, f32) {
    let center_x = width * 0.5;
    let center_y = height * 0.5;
    let x = (pointer.0 - center_x) / center_x.max(1.0);
    let y = (pointer.1 - center_y) / center_y.max(1.0);
    (shape_pointer(x), shape_pointer(y))
}

fn shape_pointer(value: f32) -> f32 {
    let magnitude = value.abs().clamp(0.0, POINTER_STEER_BOUND);
    if magnitude <= POINTER_DEADZONE {
        return 0.0;
    }
    let scaled = (magnitude - POINTER_DEADZONE) / (POINTER_STEER_BOUND - POINTER_DEADZONE);
    value.signum() * scaled * scaled
}

fn axis(negative: KeyCode, positive: KeyCode) -> f32 {
    let mut value = 0.0;
    if is_key_down(negative) {
        value -= 1.0;
    }
    if is_key_down(positive) {
        value += 1.0;
    }
    value
}

pub fn mouse_wheel_delta() -> f32 {
    let (_, vertical) = macroquad::input::mouse_wheel();
    vertical
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_center_is_neutral() {
        assert_eq!(pointer_steer((500.0, 300.0), 1000.0, 600.0), (0.0, 0.0));
    }

    #[test]
    fn pointer_steering_is_clamped_and_shaped() {
        let (right, up) = pointer_steer((1000.0, 0.0), 1000.0, 600.0);
        assert!((right - 1.0).abs() < 0.001);
        assert!((up + 1.0).abs() < 0.001);
        let (near, _) = pointer_steer((560.0, 300.0), 1000.0, 600.0);
        assert!(near > 0.0 && near < 0.05);
    }

    #[test]
    fn pointer_shows_for_focused_windows() {
        assert!(!pointer_visible(true, false));
        assert!(pointer_visible(true, true));
        assert!(pointer_visible(false, false));
        assert!(pointer_visible(false, true));
    }

    #[test]
    fn text_shortcuts_are_layout_independent() {
        assert_eq!(text_shortcut(':'), Some(Intent::Console));
        assert_eq!(text_shortcut('?'), Some(Intent::Help));
        assert_eq!(text_shortcut('/'), Some(Intent::Filter));
        assert_eq!(text_shortcut('~'), Some(Intent::Home));
        assert_eq!(text_shortcut('t'), None);
    }
}
