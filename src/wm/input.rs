use macroquad::prelude::{get_char_pressed, is_key_pressed, KeyCode};

const CONTROL_KEYS: [(KeyCode, u8); 32] = [
    (KeyCode::A, 0x01),
    (KeyCode::B, 0x02),
    (KeyCode::C, 0x03),
    (KeyCode::D, 0x04),
    (KeyCode::E, 0x05),
    (KeyCode::F, 0x06),
    (KeyCode::G, 0x07),
    (KeyCode::H, 0x08),
    (KeyCode::I, 0x09),
    (KeyCode::J, 0x0a),
    (KeyCode::K, 0x0b),
    (KeyCode::L, 0x0c),
    (KeyCode::M, 0x0d),
    (KeyCode::N, 0x0e),
    (KeyCode::O, 0x0f),
    (KeyCode::P, 0x10),
    (KeyCode::Q, 0x11),
    (KeyCode::R, 0x12),
    (KeyCode::S, 0x13),
    (KeyCode::T, 0x14),
    (KeyCode::U, 0x15),
    (KeyCode::V, 0x16),
    (KeyCode::W, 0x17),
    (KeyCode::X, 0x18),
    (KeyCode::Y, 0x19),
    (KeyCode::Z, 0x1a),
    (KeyCode::Space, 0x00),
    (KeyCode::LeftBracket, 0x1b),
    (KeyCode::Backslash, 0x1c),
    (KeyCode::RightBracket, 0x1d),
    (KeyCode::Slash, 0x1f),
    (KeyCode::Enter, 0x0d),
];

pub fn control_byte(key: KeyCode) -> Option<u8> {
    CONTROL_KEYS
        .iter()
        .find_map(|(candidate, byte)| (*candidate == key).then_some(*byte))
}

pub fn pressed_control_byte() -> Option<u8> {
    CONTROL_KEYS
        .iter()
        .find_map(|(key, byte)| is_key_pressed(*key).then_some(*byte))
}

pub fn key_sequence(key: KeyCode, shift: bool, application_cursor: bool) -> Option<&'static [u8]> {
    match key {
        KeyCode::Enter => Some(b"\r"),
        KeyCode::Tab if shift => Some(b"\x1b[Z"),
        KeyCode::Tab => Some(b"\t"),
        KeyCode::Backspace => Some(b"\x7f"),
        KeyCode::Escape => Some(b"\x1b"),
        KeyCode::Insert => Some(b"\x1b[2~"),
        KeyCode::Delete => Some(b"\x1b[3~"),
        KeyCode::Home if application_cursor => Some(b"\x1bOH"),
        KeyCode::End if application_cursor => Some(b"\x1bOF"),
        KeyCode::Home => Some(b"\x1b[H"),
        KeyCode::End => Some(b"\x1b[F"),
        KeyCode::PageUp => Some(b"\x1b[5~"),
        KeyCode::PageDown => Some(b"\x1b[6~"),
        KeyCode::Up if application_cursor => Some(b"\x1bOA"),
        KeyCode::Down if application_cursor => Some(b"\x1bOB"),
        KeyCode::Right if application_cursor => Some(b"\x1bOC"),
        KeyCode::Left if application_cursor => Some(b"\x1bOD"),
        KeyCode::Up => Some(b"\x1b[A"),
        KeyCode::Down => Some(b"\x1b[B"),
        KeyCode::Right => Some(b"\x1b[C"),
        KeyCode::Left => Some(b"\x1b[D"),
        KeyCode::F1 => Some(b"\x1bOP"),
        KeyCode::F2 => Some(b"\x1bOQ"),
        KeyCode::F3 => Some(b"\x1bOR"),
        KeyCode::F4 => Some(b"\x1bOS"),
        KeyCode::F5 => Some(b"\x1b[15~"),
        KeyCode::F6 => Some(b"\x1b[17~"),
        KeyCode::F7 => Some(b"\x1b[18~"),
        KeyCode::F8 => Some(b"\x1b[19~"),
        KeyCode::F9 => Some(b"\x1b[20~"),
        KeyCode::F10 => Some(b"\x1b[21~"),
        KeyCode::F11 => Some(b"\x1b[23~"),
        KeyCode::F12 => Some(b"\x1b[24~"),
        _ => None,
    }
}

const SEQUENCE_KEYS: [KeyCode; 25] = [
    KeyCode::Enter,
    KeyCode::Tab,
    KeyCode::Backspace,
    KeyCode::Escape,
    KeyCode::Insert,
    KeyCode::Delete,
    KeyCode::Home,
    KeyCode::End,
    KeyCode::PageUp,
    KeyCode::PageDown,
    KeyCode::Up,
    KeyCode::Down,
    KeyCode::Left,
    KeyCode::Right,
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
    KeyCode::F11,
];

pub fn pressed_sequence(shift: bool, application_cursor: bool) -> Option<&'static [u8]> {
    SEQUENCE_KEYS
        .iter()
        .find(|key| is_key_pressed(**key))
        .and_then(|key| key_sequence(*key, shift, application_cursor))
}

pub fn take_text(ctrl: bool, alt: bool) -> Vec<u8> {
    let mut output = Vec::new();
    while let Some(character) = get_char_pressed() {
        if ctrl || character.is_control() {
            continue;
        }
        if alt {
            output.push(0x1b);
        }
        let mut buffer = [0_u8; 4];
        output.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_letters_map_to_control_bytes() {
        assert_eq!(control_byte(KeyCode::A), Some(0x01));
        assert_eq!(control_byte(KeyCode::C), Some(0x03));
        assert_eq!(control_byte(KeyCode::D), Some(0x04));
        assert_eq!(control_byte(KeyCode::Z), Some(0x1a));
        assert_eq!(control_byte(KeyCode::Space), Some(0x00));
        assert_eq!(control_byte(KeyCode::Key1), None);
    }

    #[test]
    fn terminal_sequences_cover_navigation_and_shift_tab() {
        assert_eq!(
            key_sequence(KeyCode::Enter, false, false),
            Some(b"\r".as_slice())
        );
        assert_eq!(
            key_sequence(KeyCode::Tab, true, false),
            Some(b"\x1b[Z".as_slice())
        );
        assert_eq!(
            key_sequence(KeyCode::Up, false, false),
            Some(b"\x1b[A".as_slice())
        );
        assert_eq!(
            key_sequence(KeyCode::Up, false, true),
            Some(b"\x1bOA".as_slice())
        );
        assert_eq!(
            key_sequence(KeyCode::Home, false, true),
            Some(b"\x1bOH".as_slice())
        );
        assert_eq!(
            key_sequence(KeyCode::Delete, false, false),
            Some(b"\x1b[3~".as_slice())
        );
        assert_eq!(
            key_sequence(KeyCode::F5, false, false),
            Some(b"\x1b[15~".as_slice())
        );
    }
}
