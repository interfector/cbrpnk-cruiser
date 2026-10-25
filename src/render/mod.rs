pub mod hud;
pub mod post;
pub mod scene;
pub mod text;

use post::PostProcessor;
use text::Fonts;

pub struct Renderer {
    pub fonts: Fonts,
    post: PostProcessor,
}

impl Renderer {
    pub fn new(font_scale: f32, crt: bool) -> Self {
        Self {
            fonts: Fonts::load(font_scale),
            post: PostProcessor::load(crt),
        }
    }

    pub fn draw_post(&self, time: f32) {
        self.post.draw(time);
    }

    pub fn post_enabled(&self) -> bool {
        self.post.enabled()
    }
}
