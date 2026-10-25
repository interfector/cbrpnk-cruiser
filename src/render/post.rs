use macroquad::prelude::{
    draw_rectangle, gl_use_default_material, gl_use_material, load_material, screen_height,
    screen_width, Material, MaterialParams, ShaderSource, UniformDesc, UniformType, WHITE,
};

pub struct PostProcessor {
    material: Option<Material>,
    enabled: bool,
}

impl PostProcessor {
    pub fn load(enabled: bool) -> Self {
        let material = load_material(
            ShaderSource::Glsl {
                vertex: include_str!("../../assets/shaders/crt.vert"),
                fragment: include_str!("../../assets/shaders/crt.frag"),
            },
            MaterialParams {
                uniforms: vec![UniformDesc::new("Time", UniformType::Float1)],
                ..Default::default()
            },
        )
        .map_err(|error| log::warn!("post-processing disabled: {error}"))
        .ok();
        Self { material, enabled }
    }

    pub fn enabled(&self) -> bool {
        self.enabled && self.material.is_some()
    }

    pub fn draw(&self, time: f32) {
        if let (true, Some(material)) = (self.enabled, &self.material) {
            material.set_uniform("Time", time);
            gl_use_material(material);
            draw_rectangle(0.0, 0.0, screen_width(), screen_height(), WHITE);
            gl_use_default_material();
        }
    }
}
