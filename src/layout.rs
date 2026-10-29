use std::collections::HashMap;
use std::path::{Path, PathBuf};

use macroquad::prelude::{vec3, Vec3};

use crate::fs::{Entry, EntryKind};
use crate::theme::{
    AUTO_ENTER_FACTOR, GALAXY_COUNT_REFERENCE, GALAXY_RADIUS_DEFAULT, GALAXY_RADIUS_MAX,
    GALAXY_RADIUS_MIN, NODE_DIR_RADIUS, NODE_EXEC_RADIUS, NODE_FILE_RADIUS, NODE_LINK_RADIUS,
    NODE_OTHER_RADIUS, SECTOR_BANDS,
};
use crate::util::{normalize_path, path_hash};

#[derive(Debug, Clone)]
pub struct Node {
    pub path: PathBuf,
    pub entry_index: usize,
    pub position: Vec3,
    pub radius: f32,
    pub galaxy_radius: f32,
    pub kind: EntryKind,
    pub executable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkArc {
    pub source: usize,
    pub target: usize,
    pub cross_galaxy: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SectorLayout {
    pub nodes: Vec<Node>,
    pub links: Vec<LinkArc>,
    index: HashMap<PathBuf, usize>,
}

impl SectorLayout {
    pub fn build(entries: &[Entry], visible: &[usize]) -> Self {
        Self::build_with_counts(entries, visible, &HashMap::new())
    }

    pub fn build_with_counts(
        entries: &[Entry],
        visible: &[usize],
        counts: &std::collections::HashMap<PathBuf, usize>,
    ) -> Self {
        Self::build_with_scales(entries, visible, counts, 1.0, 1.0)
    }

    pub fn build_with_scales(
        entries: &[Entry],
        visible: &[usize],
        counts: &std::collections::HashMap<PathBuf, usize>,
        node_scale: f32,
        galaxy_scale: f32,
    ) -> Self {
        let mut nodes = Vec::with_capacity(visible.len());
        let mut index = HashMap::with_capacity(visible.len());

        for entry_index in visible {
            let Some(entry) = entries.get(*entry_index) else {
                continue;
            };
            let child_count = counts.get(&entry.path).copied();
            let galaxy_radius = if entry.is_dir_like() {
                directory_galaxy_radius(child_count) * galaxy_scale
            } else {
                0.0
            };
            let radius = ((match entry.kind {
                EntryKind::Directory => directory_core_radius(galaxy_radius),
                EntryKind::File if entry.executable => NODE_EXEC_RADIUS,
                EntryKind::File => file_radius(entry.size),
                EntryKind::Symlink => NODE_LINK_RADIUS,
                EntryKind::Other => NODE_OTHER_RADIUS,
            }) * node_scale)
                .clamp(0.08, 0.6);

            let node = Node {
                path: entry.path.clone(),
                entry_index: *entry_index,
                position: position_for(entry),
                radius,
                galaxy_radius,
                kind: entry.kind,
                executable: entry.executable,
            };
            index.insert(node.path.clone(), nodes.len());
            nodes.push(node);
        }

        let mut links = Vec::new();
        for (source, node) in nodes.iter().enumerate() {
            if node.kind != EntryKind::Symlink {
                continue;
            }
            let Some(entry) = entries.get(node.entry_index) else {
                continue;
            };
            let Some(target_path) = link_target_path(entry) else {
                continue;
            };
            if let Some(target) = index.get(&target_path).copied() {
                if target != source {
                    links.push(LinkArc {
                        source,
                        target,
                        cross_galaxy: false,
                    });
                }
                continue;
            }
            if let Some(target) = containing_galaxy(&nodes, entries, &target_path) {
                if target != source {
                    links.push(LinkArc {
                        source,
                        target,
                        cross_galaxy: true,
                    });
                }
            }
        }

        Self {
            nodes,
            links,
            index,
        }
    }

    pub fn node_for(&self, path: &Path) -> Option<&Node> {
        self.index
            .get(path)
            .and_then(|index| self.nodes.get(*index))
    }

    pub fn node_index_for(&self, path: &Path) -> Option<usize> {
        self.index.get(path).copied()
    }

    pub fn proximity_directory<'a>(
        &self,
        entries: &'a [Entry],
        position: Vec3,
        factor: f32,
    ) -> Option<&'a Entry> {
        if factor <= 0.0 {
            return None;
        }
        let mut nearest: Option<(f32, &Entry)> = None;
        for node in &self.nodes {
            let Some(entry) = entries.get(node.entry_index) else {
                continue;
            };
            if !entry.is_dir_like() || entry.link_broken || !entry.readable {
                continue;
            }
            let distance = position.distance(node.position);
            if distance > auto_enter_radius_with_factor(node, factor) {
                continue;
            }
            if nearest
                .as_ref()
                .map(|(best, _)| distance < *best)
                .unwrap_or(true)
            {
                nearest = Some((distance, entry));
            }
        }
        nearest.map(|(_, entry)| entry)
    }

    pub fn nearest_sibling(&self, path: &Path) -> Option<&Node> {
        if self.nodes.is_empty() {
            return None;
        }
        let missing = path_hash(path);
        self.nodes.iter().min_by_key(|node| {
            let hash = path_hash(&node.path);
            hash.abs_diff(missing)
        })
    }
}

pub fn galaxy_radius(node: &Node) -> f32 {
    node.galaxy_radius
}

pub fn auto_enter_radius(node: &Node) -> f32 {
    auto_enter_radius_with_factor(node, AUTO_ENTER_FACTOR)
}

pub fn auto_enter_radius_with_factor(node: &Node, factor: f32) -> f32 {
    galaxy_radius(node) * factor
}

fn position_for(entry: &Entry) -> Vec3 {
    let band = SECTOR_BANDS[group_of(entry)];
    let hash = path_hash(&entry.path);
    let angle = hash_unit(hash, 0) * std::f32::consts::TAU;
    let latitude = hash_unit(hash, 1) * 2.0 - 1.0;
    let radius = band.inner_radius + hash_unit(hash, 2) * (band.outer_radius - band.inner_radius);
    vec3(
        angle.cos() * radius,
        band.height_center + latitude * radius * band.height_scale,
        angle.sin() * radius,
    )
}

fn hash_unit(hash: u64, salt: u64) -> f32 {
    let mut value = hash.wrapping_add(salt.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    (value & 0xffff) as f32 / u16::MAX as f32
}

fn directory_galaxy_radius(child_count: Option<usize>) -> f32 {
    let Some(count) = child_count else {
        return GALAXY_RADIUS_DEFAULT;
    };
    let normalized =
        ((1.0 + count as f32).ln() / (1.0 + GALAXY_COUNT_REFERENCE).ln()).clamp(0.0, 1.0);
    GALAXY_RADIUS_MIN + (GALAXY_RADIUS_MAX - GALAXY_RADIUS_MIN) * normalized
}

fn directory_core_radius(galaxy: f32) -> f32 {
    (NODE_DIR_RADIUS + (galaxy - GALAXY_RADIUS_DEFAULT) * 0.12).clamp(NODE_DIR_RADIUS, 0.45)
}

fn containing_galaxy(nodes: &[Node], entries: &[Entry], target_path: &Path) -> Option<usize> {
    nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| {
            entries
                .get(node.entry_index)
                .is_some_and(|entry| entry.is_dir_like() && target_path.starts_with(&node.path))
        })
        .max_by_key(|(_, node)| node.path.components().count())
        .map(|(index, _)| index)
}

fn link_target_path(entry: &Entry) -> Option<PathBuf> {
    let target = entry.link_target.as_ref()?;
    let base = entry.path.parent()?;
    let resolved = if target.is_absolute() {
        target.clone()
    } else {
        base.join(target)
    };
    Some(normalize_path(&resolved))
}

fn group_of(entry: &Entry) -> usize {
    match entry.kind {
        EntryKind::Directory => 0,
        EntryKind::File if entry.executable => 1,
        EntryKind::File => 2,
        EntryKind::Symlink => 3,
        EntryKind::Other => 4,
    }
}

fn file_radius(size: u64) -> f32 {
    let size = size.max(1) as f32;
    (NODE_FILE_RADIUS + size.log10() * 0.032).clamp(NODE_FILE_RADIUS, 0.34)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn entry(name: &str, kind: EntryKind, executable: bool, size: u64) -> Entry {
        Entry {
            name: OsString::from(name),
            path: PathBuf::from(name),
            kind,
            size,
            modified: None,
            mode: None,
            executable,
            readable: true,
            link_target: None,
            link_target_is_dir: None,
            link_broken: false,
            metadata_error: None,
            hidden: false,
        }
    }

    fn sample() -> Vec<Entry> {
        vec![
            entry("dir", EntryKind::Directory, false, 0),
            entry("run", EntryKind::File, true, 10),
            entry("data", EntryKind::File, false, 4096),
            entry("link", EntryKind::Symlink, false, 0),
            entry("special", EntryKind::Other, false, 0),
        ]
    }

    #[test]
    fn layout_is_deterministic() {
        let entries = sample();
        let visible: Vec<usize> = (0..entries.len()).collect();
        let a = SectorLayout::build(&entries, &visible);
        let b = SectorLayout::build(&entries, &visible);
        assert_eq!(a.nodes.len(), b.nodes.len());
        for (lhs, rhs) in a.nodes.iter().zip(b.nodes.iter()) {
            assert_eq!(lhs.position, rhs.position);
            assert_eq!(lhs.radius, rhs.radius);
        }
    }

    #[test]
    fn layout_is_finite_and_bounded() {
        let entries = sample();
        let visible: Vec<usize> = (0..entries.len()).collect();
        let layout = SectorLayout::build(&entries, &visible);
        for node in &layout.nodes {
            assert!(node.position.x.is_finite());
            assert!(node.position.y.is_finite());
            assert!(node.position.z.is_finite());
            assert!(node.position.length() < 30.0);
        }
    }

    #[test]
    fn handles_empty_directory() {
        let layout = SectorLayout::build(&[], &[]);
        assert!(layout.nodes.is_empty());
        assert!(layout.nearest_sibling(Path::new("x")).is_none());
    }

    #[test]
    fn lookup_uses_paths() {
        let entries = sample();
        let visible: Vec<usize> = (0..entries.len()).collect();
        let layout = SectorLayout::build(&entries, &visible);
        let node = layout.node_for(Path::new("data")).unwrap();
        assert_eq!(node.entry_index, 2);
    }

    #[test]
    fn links_connect_visible_sibling_targets() {
        let mut entries = sample();
        entries[2].path = PathBuf::from("/sector/data");
        entries[3].path = PathBuf::from("/sector/link");
        entries[3].link_target = Some(PathBuf::from("./nested/../data"));
        let layout = SectorLayout::build(&entries, &[0, 1, 2, 3, 4]);
        assert_eq!(
            layout.links,
            vec![LinkArc {
                source: 3,
                target: 2,
                cross_galaxy: false,
            }]
        );
    }

    #[test]
    fn links_connect_nested_targets_to_their_galaxy() {
        let mut entries = sample();
        entries[0].path = PathBuf::from("/sector/alpha");
        entries[0].name = OsString::from("alpha");
        entries[3].path = PathBuf::from("/sector/link");
        entries[3].link_target = Some(PathBuf::from("alpha/nested/data.txt"));
        let layout = SectorLayout::build(&entries, &[0, 1, 2, 3, 4]);
        assert_eq!(
            layout.links,
            vec![LinkArc {
                source: 3,
                target: 0,
                cross_galaxy: true,
            }]
        );
    }

    #[test]
    fn links_prefer_the_deepest_visible_galaxy() {
        let mut entries = sample();
        entries[0].path = PathBuf::from("/sector/alpha");
        entries[1].kind = EntryKind::Directory;
        entries[1].path = PathBuf::from("/sector/alpha/beta");
        entries[3].path = PathBuf::from("/sector/link");
        entries[3].link_target = Some(PathBuf::from("alpha/beta/data.txt"));
        let layout = SectorLayout::build(&entries, &[0, 1, 2, 3, 4]);
        assert_eq!(layout.links.len(), 1);
        assert_eq!(layout.links[0].source, 3);
        assert_eq!(layout.links[0].target, 1);
        assert!(layout.links[0].cross_galaxy);
    }

    #[test]
    fn links_ignore_targets_outside_the_sector() {
        let mut entries = sample();
        entries[3].path = PathBuf::from("/sector/link");
        entries[3].link_target = Some(PathBuf::from("../other/data"));
        let layout = SectorLayout::build(&entries, &[0, 1, 2, 3, 4]);
        assert!(layout.links.is_empty());
    }

    #[test]
    fn proximity_enters_only_readable_directories() {
        let mut entries = sample();
        entries[0].path = PathBuf::from("/sector/dir");
        let layout = SectorLayout::build(&entries, &[0, 1, 2, 3, 4]);
        assert_eq!(
            layout
                .proximity_directory(&entries, layout.nodes[0].position, AUTO_ENTER_FACTOR)
                .map(|entry| entry.name.as_os_str()),
            Some(std::ffi::OsStr::new("dir"))
        );
        assert!(layout
            .proximity_directory(
                &entries,
                layout.nodes[0].position + vec3(2.0, 0.0, 0.0),
                AUTO_ENTER_FACTOR,
            )
            .is_none());
        entries[0].readable = false;
        assert!(layout
            .proximity_directory(&entries, layout.nodes[0].position, AUTO_ENTER_FACTOR)
            .is_none());
    }

    #[test]
    fn auto_enter_factor_controls_proximity_radius() {
        let entries = sample();
        let layout = SectorLayout::build(&entries, &[0, 1, 2, 3, 4]);
        let node = &layout.nodes[0];
        let narrow = auto_enter_radius_with_factor(node, 0.25);
        let wide = auto_enter_radius_with_factor(node, 0.5);
        assert!(narrow < wide);
        let between = node.position + vec3((narrow + wide) * 0.5, 0.0, 0.0);
        assert!(layout.proximity_directory(&entries, between, 0.5).is_some());
        assert!(layout
            .proximity_directory(&entries, between, 0.25)
            .is_none());
        assert!(layout
            .proximity_directory(&entries, node.position, 0.0)
            .is_none());
    }

    #[test]
    fn galaxy_entry_radius_is_larger_than_the_core() {
        let entries = sample();
        let layout = SectorLayout::build(&entries, &[0, 1, 2, 3, 4]);
        let directory = &layout.nodes[0];
        assert!(galaxy_radius(directory) > directory.radius);
        assert!(auto_enter_radius(directory) < galaxy_radius(directory));
        assert!(auto_enter_radius(directory) <= galaxy_radius(directory) * 0.55);
    }

    #[test]
    fn directories_fill_a_broad_multi_level_shell() {
        let entries: Vec<Entry> = (0..320)
            .map(|index| {
                entry(
                    &format!("folder-{index:03}"),
                    EntryKind::Directory,
                    false,
                    0,
                )
            })
            .collect();
        let visible: Vec<usize> = (0..entries.len()).collect();
        let layout = SectorLayout::build(&entries, &visible);
        let band = SECTOR_BANDS[0];
        let mut min_radius = f32::MAX;
        let mut max_radius = 0.0_f32;
        let mut min_height = f32::MAX;
        let mut max_height = f32::MIN;

        for node in &layout.nodes {
            let radius = node.position.x.hypot(node.position.z);
            assert!(radius >= band.inner_radius - 0.001);
            assert!(radius <= band.outer_radius + 0.001);
            assert!(node.position.y > -0.75);
            min_radius = min_radius.min(radius);
            max_radius = max_radius.max(radius);
            min_height = min_height.min(node.position.y);
            max_height = max_height.max(node.position.y);
        }

        assert!(max_radius - min_radius > 1.8);
        assert!(max_height - min_height > 4.0);
    }

    #[test]
    fn similar_names_do_not_collapse_into_one_position() {
        let entries: Vec<Entry> = (1..=30)
            .map(|index| {
                entry(
                    &format!("folder-{index:03}"),
                    EntryKind::Directory,
                    false,
                    0,
                )
            })
            .collect();
        let visible: Vec<usize> = (0..entries.len()).collect();
        let layout = SectorLayout::build(&entries, &visible);
        let mut closest = f32::MAX;
        let mut nearest_sum = 0.0;

        for (index, node) in layout.nodes.iter().enumerate() {
            let mut nearest = f32::MAX;
            for other in &layout.nodes[..index] {
                let distance = node.position.distance(other.position);
                closest = closest.min(distance);
                nearest = nearest.min(distance);
            }
            if nearest.is_finite() {
                nearest_sum += nearest;
            }
        }

        assert!(closest > 0.3, "closest={closest}");
        assert!(nearest_sum / layout.nodes.len() as f32 > 1.2);
    }

    #[test]
    fn galaxy_radius_grows_with_child_count_and_stays_bounded() {
        let entries = sample();
        let mut counts = HashMap::new();
        counts.insert(PathBuf::from("dir"), 0);
        let empty = SectorLayout::build_with_counts(&entries, &[0], &counts);
        counts.insert(PathBuf::from("dir"), 500);
        let full = SectorLayout::build_with_counts(&entries, &[0], &counts);
        counts.insert(PathBuf::from("dir"), usize::MAX);
        let huge = SectorLayout::build_with_counts(&entries, &[0], &counts);

        assert!(empty.nodes[0].galaxy_radius < full.nodes[0].galaxy_radius);
        assert!(huge.nodes[0].galaxy_radius <= GALAXY_RADIUS_MAX);
        assert!(empty.nodes[0].galaxy_radius >= GALAXY_RADIUS_MIN);
        assert!(full.nodes[0].radius > empty.nodes[0].radius);
    }

    #[test]
    fn positions_are_path_keyed_and_stable_when_entries_are_added() {
        let entries = sample();
        let first = SectorLayout::build(&entries, &[0, 1, 2]);
        let all = SectorLayout::build(&entries, &[0, 1, 2, 3, 4]);
        for path in [
            PathBuf::from("dir"),
            PathBuf::from("run"),
            PathBuf::from("data"),
        ] {
            let before = first.node_for(&path).unwrap().position;
            let after = all.node_for(&path).unwrap().position;
            assert_eq!(before, after);
        }
    }

    #[test]
    fn large_files_stay_clamped() {
        let entries = vec![
            entry("small", EntryKind::File, false, 1),
            entry("huge", EntryKind::File, false, u64::MAX),
        ];
        let layout = SectorLayout::build(&entries, &[0, 1]);
        assert!(layout.nodes[1].radius <= 0.34);
    }
}
