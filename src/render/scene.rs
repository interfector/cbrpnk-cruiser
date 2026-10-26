use std::path::Path;

use macroquad::prelude::{
    draw_circle, draw_circle_lines, draw_line, draw_rectangle, screen_height, screen_width, Color,
    Vec2, Vec3,
};

use crate::camera::{Camera, Projected};
use crate::fs::{Entry, EntryKind};
use crate::layout::{galaxy_radius, Node, SectorLayout};
use crate::render::text::{Align, Fonts};
use crate::theme::{
    alpha, AMBER, CYAN, CYAN_DIM, FONT_SMALL, FONT_TINY, GALAXY_ORBIT_SEGMENTS, GRID_DIM, ICE,
    MAGENTA, MAX_GALAXIES, MAX_VISIBLE_LABELS, PARENT_GATE_ENTRY_RADIUS, TEXT_MUTED,
};
use crate::util::path_hash;

pub struct Scene<'a> {
    pub camera: &'a Camera,
    pub layout: &'a SectorLayout,
    pub entries: &'a [Entry],
    pub selected: Option<&'a Path>,
    pub locked: bool,
    pub sector_path: &'a Path,
    pub time: f32,
    pub reduced_motion: bool,
}

struct ScreenNode<'a> {
    node: &'a Node,
    projected: Projected,
}

struct Label {
    text: String,
    position: Vec2,
    color: Color,
    selected: bool,
    depth: f32,
}

pub fn parent_gate_position() -> Vec3 {
    macroquad::prelude::vec3(0.0, 4.3, 4.6)
}

pub fn parent_gate_reached(position: Vec3) -> bool {
    position.distance(parent_gate_position()) <= PARENT_GATE_ENTRY_RADIUS
}

impl Scene<'_> {
    pub fn draw(&self, fonts: &Fonts) {
        let width = screen_width();
        let height = screen_height();
        self.draw_grid(width, height);
        self.draw_rings(width, height);
        self.draw_links(width, height);
        self.draw_parent_gate(fonts, width, height);
        self.draw_nodes(fonts, width, height);
        self.draw_reticle(fonts, width, height);
    }

    fn draw_grid(&self, width: f32, height: f32) {
        let extent: f32 = 26.0;
        let step: f32 = 2.0;
        let floor: f32 = -0.8;
        let mut value: f32 = -extent;
        while value <= extent {
            let major = (value / (step * 4.0)).round() * step * 4.0;
            let is_major = (value - major).abs() < 0.01;
            let color = alpha(GRID_DIM, if is_major { 0.5 } else { 0.24 });
            let a = macroquad::prelude::vec3(value, floor, -extent);
            let b = macroquad::prelude::vec3(value, floor, extent);
            if let (Some(a), Some(b)) = (
                self.camera.project(a, width, height),
                self.camera.project(b, width, height),
            ) {
                draw_line(a.screen.x, a.screen.y, b.screen.x, b.screen.y, 1.0, color);
            }
            let a = macroquad::prelude::vec3(-extent, floor, value);
            let b = macroquad::prelude::vec3(extent, floor, value);
            if let (Some(a), Some(b)) = (
                self.camera.project(a, width, height),
                self.camera.project(b, width, height),
            ) {
                draw_line(a.screen.x, a.screen.y, b.screen.x, b.screen.y, 1.0, color);
            }
            value += step;
        }

        let seed = path_hash(self.sector_path);
        let towers = 44;
        for index in 0..towers {
            let angle = index as f32 / towers as f32 * std::f32::consts::TAU;
            let jitter = ((seed.rotate_left(index as u32 % 63) % 1000) as f32) / 1000.0;
            let radius = 27.0 + jitter * 4.0;
            let height_value = 1.0 + ((seed >> (index % 32)) & 7) as f32 * 0.6;
            let base = macroquad::prelude::vec3(angle.cos() * radius, floor, angle.sin() * radius);
            let top = base + macroquad::prelude::vec3(0.0, height_value, 0.0);
            let Some(base_projected) = self.camera.project(base, width, height) else {
                continue;
            };
            let Some(top_projected) = self.camera.project(top, width, height) else {
                continue;
            };
            let fade = (1.0 - base_projected.depth / 90.0).clamp(0.12, 0.55);
            draw_line(
                base_projected.screen.x,
                base_projected.screen.y,
                top_projected.screen.x,
                top_projected.screen.y,
                1.0,
                alpha(CYAN_DIM, 0.24 * fade),
            );
            if index % 3 == 0 {
                let mid = base + macroquad::prelude::vec3(0.0, height_value * 0.55, 0.0);
                if let Some(mid_projected) = self.camera.project(mid, width, height) {
                    draw_line(
                        top_projected.screen.x,
                        top_projected.screen.y,
                        mid_projected.screen.x,
                        mid_projected.screen.y,
                        1.0,
                        alpha(CYAN_DIM, 0.13 * fade),
                    );
                }
            }
        }
    }

    fn draw_rings(&self, width: f32, height: f32) {
        let rings: [(f32, f32); 4] = [(2.4, 0.1), (3.6, 0.9), (4.8, 0.0), (6.0, 1.4)];
        for (index, (radius, y)) in rings.iter().enumerate() {
            let segments = 72;
            let color = alpha(CYAN_DIM, if index == 0 { 0.28 } else { 0.13 });
            let mut previous: Option<Projected> = None;
            for segment in 0..=segments {
                let angle = segment as f32 / segments as f32 * std::f32::consts::TAU;
                let point =
                    macroquad::prelude::vec3(angle.cos() * radius, *y, angle.sin() * radius);
                let projected = self.camera.project(point, width, height);
                if let (Some(from), Some(to)) = (previous, projected) {
                    let alpha = (1.0 - from.depth / 80.0).clamp(0.03, 1.0);
                    draw_line(
                        from.screen.x,
                        from.screen.y,
                        to.screen.x,
                        to.screen.y,
                        1.0,
                        alpha_multiply(color, alpha),
                    );
                }
                previous = projected;
            }
        }
    }

    fn draw_links(&self, width: f32, height: f32) {
        for link in &self.layout.links {
            let (Some(source), Some(target)) = (
                self.layout.nodes.get(link.source),
                self.layout.nodes.get(link.target),
            ) else {
                continue;
            };
            let from = source.position;
            let to = target.position;
            let delta = to - from;
            let seed = path_hash(&source.path) ^ path_hash(&target.path);
            let perpendicular = Vec3::new(-delta.z, 0.0, delta.x).normalize_or_zero();
            let bow = ((seed.rotate_left(13) % 1000) as f32 / 1000.0 - 0.5) * 0.6;
            let control = from
                + delta * 0.5
                + Vec3::new(0.0, 0.9 + delta.length() * 0.18, 0.0)
                + perpendicular * bow;
            let highlighted = self
                .selected
                .is_some_and(|path| path == source.path || path == target.path);
            let base_alpha = if highlighted {
                0.7
            } else if link.cross_galaxy {
                0.2
            } else {
                0.24
            };
            let mut previous: Option<Projected> = None;
            for step in 0..=28 {
                let t = step as f32 / 28.0;
                let inverse = 1.0 - t;
                let point =
                    from * (inverse * inverse) + control * (2.0 * inverse * t) + to * (t * t);
                let projected = self.camera.project(point, width, height);
                if let (Some(start), Some(end)) = (previous, projected) {
                    let depth = (start.depth + end.depth) * 0.5;
                    let fade = (1.0 - depth / 70.0).clamp(0.12, 1.0);
                    draw_line(
                        start.screen.x,
                        start.screen.y,
                        end.screen.x,
                        end.screen.y,
                        1.2,
                        alpha(MAGENTA, base_alpha * fade),
                    );
                }
                previous = projected;
            }
        }
    }

    fn draw_parent_gate(&self, fonts: &Fonts, width: f32, height: f32) {
        let Some(parent) = self.sector_path.parent() else {
            return;
        };
        let selected = self.selected == Some(parent);
        let Some(projected) = self.camera.project(parent_gate_position(), width, height) else {
            return;
        };
        let pulse = 0.5 + 0.5 * (self.time * 1.3).sin();
        let radius = (projected.scale * 0.55).clamp(18.0, 54.0);
        let x = projected.screen.x;
        let y = projected.screen.y;
        for arc in 0..6 {
            let rotation = arc as f32 * 60.0 + self.time * 6.0;
            macroquad::prelude::draw_arc(
                x,
                y,
                18,
                radius,
                rotation,
                1.0,
                32.0,
                if selected {
                    alpha(ICE, 0.5 + pulse * 0.18)
                } else {
                    alpha(CYAN_DIM, 0.24 + pulse * 0.12)
                },
            );
        }
        draw_circle_lines(
            x,
            y,
            radius * 0.56,
            if selected { 1.4 } else { 1.0 },
            alpha(
                if selected { CYAN } else { CYAN_DIM },
                if selected { 0.9 } else { 0.34 },
            ),
        );
        if selected {
            draw_circle_lines(x, y, radius + 8.0, 1.0, alpha(CYAN, 0.7));
        }
        fonts.draw(
            "PARENT",
            x,
            y + radius + 5.0,
            FONT_TINY,
            alpha(
                if selected { ICE } else { CYAN },
                if selected { 0.95 } else { 0.55 },
            ),
            true,
            Align::Center,
        );
    }

    fn draw_nodes(&self, fonts: &Fonts, width: f32, height: f32) {
        let mut nodes = Vec::with_capacity(self.layout.nodes.len());
        for node in &self.layout.nodes {
            if let Some(projected) = self.camera.project(node.position, width, height) {
                if projected.screen.x > -180.0
                    && projected.screen.x < width + 180.0
                    && projected.screen.y > -180.0
                    && projected.screen.y < height + 180.0
                {
                    nodes.push(ScreenNode { node, projected });
                }
            }
        }
        nodes.sort_by(|a, b| {
            b.projected
                .depth
                .partial_cmp(&a.projected.depth)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut labels = Vec::new();
        let mut galaxy_budget = MAX_GALAXIES;
        for item in &nodes {
            let screen_radius = (item.node.radius * item.projected.scale).clamp(3.0, 46.0);
            let fade = (1.0 - item.projected.depth / 70.0).clamp(0.16, 1.0);
            let selected = self
                .selected
                .map(|path| path == item.node.path.as_path())
                .unwrap_or(false);
            let color = node_color(item.node, selected, fade);
            let galaxy_detail =
                item.node.kind == EntryKind::Directory && screen_radius >= 4.0 && galaxy_budget > 0;
            if galaxy_detail {
                galaxy_budget -= 1;
            }
            self.draw_node_shape(
                item.node,
                item.projected.screen,
                screen_radius,
                color,
                selected,
                galaxy_detail,
                width,
                height,
            );
            labels.push(Label {
                text: self
                    .entries
                    .get(item.node.entry_index)
                    .map(Entry::display_name)
                    .unwrap_or_else(|| item.node.path.display().to_string()),
                position: item.projected.screen,
                color: if selected { ICE } else { alpha(color, 0.78) },
                selected,
                depth: item.projected.depth,
            });
        }

        labels.sort_by(|a, b| {
            b.selected.cmp(&a.selected).then_with(|| {
                b.depth
                    .partial_cmp(&a.depth)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        labels.truncate(MAX_VISIBLE_LABELS);

        for label in labels {
            let depth_fade = (1.0 - label.depth / 75.0).clamp(0.2, 1.0);
            let text = crate::util::elide(&label.text, 34);
            let y = label.position.y + if label.selected { 30.0 } else { 22.0 };
            let width_estimate = fonts.width(&text, FONT_SMALL, label.selected);
            let x = label.position.x.clamp(
                width_estimate * 0.5 + 8.0,
                width - width_estimate * 0.5 - 8.0,
            );
            if label.selected {
                draw_rectangle(
                    x - width_estimate * 0.5 - 4.0,
                    y - 2.0,
                    width_estimate + 8.0,
                    fonts.line_height(FONT_SMALL),
                    alpha(macroquad::prelude::BLACK, 0.55),
                );
            }
            fonts.draw_elided(
                &text,
                x,
                y,
                FONT_SMALL,
                width_estimate + 1.0,
                alpha(label.color, depth_fade),
                label.selected,
                Align::Center,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_node_shape(
        &self,
        node: &Node,
        center: Vec2,
        radius: f32,
        color: Color,
        selected: bool,
        galaxy_detail: bool,
        width: f32,
        height: f32,
    ) {
        let x = center.x;
        let y = center.y;
        draw_circle(x, y, radius * 1.65, alpha(color, 0.035));
        match node.kind {
            EntryKind::Directory => {
                if galaxy_detail {
                    self.draw_galaxy(node, color, width, height);
                }
                let step = if self.reduced_motion {
                    0.0
                } else {
                    self.time * 13.0
                };
                for segment in 0..3 {
                    macroquad::prelude::draw_arc(
                        x,
                        y,
                        16,
                        radius,
                        step + segment as f32 * 120.0,
                        1.0,
                        84.0,
                        color,
                    );
                }
                draw_circle_lines(x, y, radius * 0.42, 1.0, alpha(color, 0.55));
            }
            EntryKind::File if node.executable => {
                let points = [
                    Vec2::new(x, y - radius),
                    Vec2::new(x + radius, y),
                    Vec2::new(x, y + radius),
                    Vec2::new(x - radius, y),
                ];
                for index in 0..4 {
                    let a = points[index];
                    let b = points[(index + 1) % 4];
                    draw_line(a.x, a.y, b.x, b.y, 1.5, color);
                }
                draw_circle(x, y, radius * 0.22, alpha(color, 0.8));
            }
            EntryKind::File => {
                let rotation = if self.reduced_motion {
                    0.35
                } else {
                    0.35 + self.time * 0.12
                };
                let points: Vec<Vec2> = (0..4)
                    .map(|index| {
                        let angle = rotation + index as f32 * std::f32::consts::FRAC_PI_2;
                        Vec2::new(x + angle.cos() * radius, y + angle.sin() * radius)
                    })
                    .collect();
                for index in 0..4 {
                    let a = points[index];
                    let b = points[(index + 1) % 4];
                    draw_line(a.x, a.y, b.x, b.y, 1.2, color);
                }
                draw_line(
                    x - radius * 0.3,
                    y,
                    x + radius * 0.3,
                    y,
                    1.0,
                    alpha(color, 0.5),
                );
            }
            EntryKind::Symlink => {
                let step = if self.reduced_motion {
                    0.0
                } else {
                    self.time * 22.0
                };
                macroquad::prelude::draw_arc(x, y, 20, radius, step, 1.3, 245.0, color);
                macroquad::prelude::draw_arc(
                    x,
                    y,
                    20,
                    radius * 0.55,
                    step + 180.0,
                    1.0,
                    220.0,
                    alpha(color, 0.65),
                );
            }
            EntryKind::Other => {
                macroquad::prelude::draw_arc(x, y, 14, radius, 30.0, 1.2, 120.0, color);
                macroquad::prelude::draw_arc(x, y, 14, radius, 210.0, 1.2, 100.0, color);
                draw_line(
                    x + radius * 0.2,
                    y + radius * 0.2,
                    x + radius * 1.0,
                    y + radius * 1.0,
                    1.0,
                    alpha(color, 0.5),
                );
            }
        }

        if selected {
            let outer = radius + 9.0;
            let bracket = (radius * 0.4).clamp(4.0, 11.0);
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let corner = Vec2::new(x + sx * outer, y + sy * outer);
                draw_line(
                    corner.x,
                    corner.y,
                    corner.x - sx * bracket,
                    corner.y,
                    1.4,
                    ICE,
                );
                draw_line(
                    corner.x,
                    corner.y,
                    corner.x,
                    corner.y - sy * bracket,
                    1.4,
                    ICE,
                );
            }
            let step = if self.reduced_motion {
                0.0
            } else {
                self.time * 26.0
            };
            for index in 0..4 {
                macroquad::prelude::draw_arc(
                    x,
                    y,
                    20,
                    outer + 5.0,
                    step + index as f32 * 90.0,
                    1.0,
                    46.0,
                    alpha(CYAN, 0.75),
                );
            }
        }
    }

    fn draw_galaxy(&self, node: &Node, color: Color, width: f32, height: f32) {
        let orbit = galaxy_radius(node);
        let seed = path_hash(&node.path);
        let base_phase = (seed % 360) as f32 * std::f32::consts::TAU / 360.0;
        let tilts = [0.16_f32, 0.52, -0.34];
        let scales = [1.0_f32, 0.79, 0.56];
        for ring in 0..tilts.len() {
            let phase = base_phase + ring as f32 * 1.37;
            let tilt = tilts[ring];
            let radius = orbit * scales[ring];
            let u = Vec3::new(phase.cos(), 0.0, -phase.sin());
            let v = Vec3::new(
                phase.sin() * tilt.cos(),
                tilt.sin(),
                phase.cos() * tilt.cos(),
            );
            let spin = if self.reduced_motion {
                0.0
            } else {
                self.time * (0.16 + ring as f32 * 0.07) * if ring % 2 == 0 { 1.0 } else { -1.0 }
            };
            let mut previous: Option<Projected> = None;
            for step in 0..=GALAXY_ORBIT_SEGMENTS {
                let angle =
                    spin + step as f32 / GALAXY_ORBIT_SEGMENTS as f32 * std::f32::consts::TAU;
                let point = node.position + u * (angle.cos() * radius) + v * (angle.sin() * radius);
                let projected = self.camera.project(point, width, height);
                if let (Some(start), Some(end)) = (previous, projected) {
                    let fade = (1.0 - start.depth / 75.0).clamp(0.08, 1.0);
                    draw_line(
                        start.screen.x,
                        start.screen.y,
                        end.screen.x,
                        end.screen.y,
                        1.0,
                        alpha(color, (0.18 + ring as f32 * 0.03) * fade),
                    );
                }
                previous = projected;
            }

            for star in 0..3 {
                let angle = spin * 1.7 + star as f32 * 2.17 + ring as f32 * 0.9 + base_phase;
                let point = node.position + u * (angle.cos() * radius) + v * (angle.sin() * radius);
                if let Some(projected) = self.camera.project(point, width, height) {
                    let fade = (1.0 - projected.depth / 70.0).clamp(0.12, 1.0);
                    draw_circle(
                        projected.screen.x,
                        projected.screen.y,
                        1.2,
                        alpha(if ring == 0 { color } else { CYAN_DIM }, 0.65 * fade),
                    );
                }
            }
        }
    }

    fn draw_reticle(&self, fonts: &Fonts, width: f32, height: f32) {
        let x = width * 0.5;
        let y = height * 0.5;
        let pulse = if self.reduced_motion {
            0.0
        } else {
            (self.time * 2.2).sin() * 1.2
        };
        let color = if self.selected.is_some() {
            alpha(AMBER, 0.95)
        } else {
            alpha(CYAN, 0.85)
        };
        for arc in 0..2 {
            macroquad::prelude::draw_arc(
                x,
                y,
                24,
                14.0 + pulse,
                arc as f32 * 180.0 + 24.0,
                1.2,
                58.0,
                color,
            );
        }
        draw_circle(x, y, 1.8, color);
        draw_line(x - 30.0, y, x - 20.0, y, 1.0, alpha(color, 0.55));
        draw_line(x + 20.0, y, x + 30.0, y, 1.0, alpha(color, 0.55));
        draw_line(x, y - 30.0, x, y - 20.0, 1.0, alpha(color, 0.45));
        draw_line(x, y + 20.0, x, y + 30.0, 1.0, alpha(color, 0.45));

        if self.locked && self.selected.is_some() {
            fonts.draw(
                "LOCK",
                x,
                y + 34.0,
                FONT_TINY,
                alpha(ICE, 0.8),
                true,
                Align::Center,
            );
        }
    }
}

fn alpha_multiply(color: Color, factor: f32) -> Color {
    alpha(color, color.a * factor.clamp(0.0, 1.0))
}

fn node_color(node: &Node, selected: bool, fade: f32) -> Color {
    let base = match node.kind {
        EntryKind::Directory => CYAN,
        EntryKind::File if node.executable => AMBER,
        EntryKind::File => alpha(ICE, 0.86),
        EntryKind::Symlink => MAGENTA,
        EntryKind::Other => TEXT_MUTED,
    };
    if selected {
        alpha(ICE, 1.0)
    } else {
        alpha(base, 0.38 + 0.62 * fade)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_gate_detects_only_nearby_camera_positions() {
        let gate = parent_gate_position();
        assert!(parent_gate_reached(gate));
        assert!(parent_gate_reached(
            gate + macroquad::prelude::vec3(PARENT_GATE_ENTRY_RADIUS * 0.9, 0.0, 0.0)
        ));
        assert!(!parent_gate_reached(
            gate + macroquad::prelude::vec3(PARENT_GATE_ENTRY_RADIUS * 1.1, 0.0, 0.0)
        ));
    }
}
