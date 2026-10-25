use macroquad::prelude::{
    draw_text_ex, load_ttf_font_from_bytes, measure_text, Color, Font, TextParams,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

pub struct Fonts {
    pub regular: Font,
    pub bold: Font,
    pub scale: f32,
}

impl Fonts {
    pub fn load(scale: f32) -> Self {
        let regular = load_ttf_font_from_bytes(include_bytes!(
            "../../assets/fonts/NotoSansMono-Regular.ttf"
        ))
        .expect("bundled regular font must be valid");
        let bold =
            load_ttf_font_from_bytes(include_bytes!("../../assets/fonts/NotoSansMono-Bold.ttf"))
                .expect("bundled bold font must be valid");
        Self {
            regular,
            bold,
            scale: scale.clamp(0.75, 2.0),
        }
    }

    pub fn width(&self, text: &str, size: f32, bold: bool) -> f32 {
        let font = if bold { &self.bold } else { &self.regular };
        let scaled = self.size(size);
        measure_text(text, Some(font), scaled, 1.0).width
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        text: &str,
        x: f32,
        top: f32,
        size: f32,
        color: Color,
        bold: bool,
        align: Align,
    ) -> f32 {
        let font = if bold { &self.bold } else { &self.regular };
        let scaled = self.size(size);
        let width = self.width(text, size, bold);
        let x = match align {
            Align::Left => x,
            Align::Center => x - width * 0.5,
            Align::Right => x - width,
        };
        draw_text_ex(
            text,
            x,
            top + scaled as f32 * 0.82,
            TextParams {
                font: Some(font),
                font_size: scaled,
                font_scale: 1.0,
                color,
                ..Default::default()
            },
        );
        width
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_elided(
        &self,
        text: &str,
        x: f32,
        top: f32,
        size: f32,
        max_width: f32,
        color: Color,
        bold: bool,
        align: Align,
    ) -> f32 {
        let shown = elide_to_width(self, text, size, max_width, bold);
        self.draw(&shown, x, top, size, color, bold, align)
    }

    pub fn line_height(&self, size: f32) -> f32 {
        self.size(size) as f32 * 1.35
    }

    fn size(&self, size: f32) -> u16 {
        (size * self.scale).round().clamp(8.0, 96.0) as u16
    }
}

fn elide_to_width(fonts: &Fonts, text: &str, size: f32, max_width: f32, bold: bool) -> String {
    if fonts.width(text, size, bold) <= max_width {
        return text.to_string();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let candidate: String = chars.iter().collect::<String>() + "…";
        if fonts.width(&candidate, size, bold) <= max_width {
            return candidate;
        }
    }
    "…".to_string()
}
