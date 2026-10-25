use std::collections::HashMap;
use std::path::{Path, PathBuf};

use macroquad::prelude::{vec3, Vec3};

use crate::fs::{Entry, EntryKind};
use crate::theme::{
    AUTO_ENTER_FACTOR, GALAXY_RADIUS_SCALE, NODE_DIR_RADIUS, NODE_EXEC_RADIUS, NODE_FILE_RADIUS,
    NODE_LINK_RADIUS, NODE_OTHER_RADIUS,
};
use crate::util::{normalize_path, path_hash};

const GOLDEN_ANGLE: f32 = 2.399_963_2;
const RING_CAPACITY: f32 = 24.0;

#[derive(Debug, Clone)]
pub struct Node {
    pub path: PathBuf,
    pub entry_index: usize,
    pub position: Vec3,
    pub radius: f32,
    pub kind: EntryKind,
    pub executable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkArc {
    pub source: usize,
    pub target: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SectorLayout {
    pub nodes: Vec<Node>,
    pub links: Vec<LinkArc>,
    index: HashMap<PathBuf, usize>,
}

impl SectorLayout {
    pub fn build(entries: &[Entry], visible: &[usize]) -> Self {
        let mut nodes = Vec::with_capacity(visible.len());
        let mut index = HashMap::with_capacity(visible.len());
        let mut group_counts = [0usize; 5];

        let sector_rotation = visible
            .first()
            .map(|entry_index| {
                let hash = path_hash(&entries[*entry_index].path);
                (hash % 360) as f32 * std::f32::consts::TAU / 360.0
            })
            .unwrap_or(0.0);

        for entry_index in visible {
            let Some(entry) = entries.get(*entry_index) else {
                continue;
            };
            let group = group_of(entry);
            let group_index = group_counts[group];
            group_counts[group] += 1;

            let base_radius = match group {
                0 => 2.4,
                1 => 3.6,
                2 => 4.8,
                3 => 6.0,
                _ => 7.0,
            };
            let base_height = match group {
                0 => 0.55,
                1 => 1.25,
                2 => 0.0,
                3 => 1.8,
                _ => 0.9,
            };
            let ring = (group_index as f32 / RING_CAPACITY).floor();
            let radius = base_radius + ring.sqrt() * 0.85;
            let angle = sector_rotation + group as f32 * 0.37 + group_index as f32 * GOLDEN_ANGLE;
            let position = vec3(
                angle.cos() * radius,
                base_height + ring * 0.12,
                angle.sin() * radius,
            );
            let radius = match entry.kind {
                EntryKind::Directory => NODE_DIR_RADIUS,
                EntryKind::File if entry.executable => NODE_EXEC_RADIUS,
                EntryKind::File => file_radius(entry.size),
                EntryKind::Symlink => NODE_LINK_RADIUS,
                EntryKind::Other => NODE_OTHER_RADIUS,
            };

            let node = Node {
                path: entry.path.clone(),
                entry_index: *entry_index,
                position,
                radius,
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
                    links.push(LinkArc { source, target });
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
    ) -> Option<&'a Entry> {
        let mut nearest: Option<(f32, &Entry)> = None;
        for node in &self.nodes {
            let Some(entry) = entries.get(node.entry_index) else {
                continue;
            };
            if !entry.is_dir_like() || entry.link_broken || !entry.readable {
                continue;
            }
            let distance = position.distance(node.position);
            if distance > auto_enter_radius(node) {
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
    node.radius * GALAXY_RADIUS_SCALE
}

pub fn auto_enter_radius(node: &Node) -> f32 {
    galaxy_radius(node) * AUTO_ENTER_FACTOR
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
                target: 2
            }]
        );
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
                .proximity_directory(&entries, layout.nodes[0].position)
                .map(|entry| entry.name.as_os_str()),
            Some(std::ffi::OsStr::new("dir"))
        );
        assert!(layout
            .proximity_directory(&entries, layout.nodes[0].position + vec3(2.0, 0.0, 0.0))
            .is_none());
        entries[0].readable = false;
        assert!(layout
            .proximity_directory(&entries, layout.nodes[0].position)
            .is_none());
    }

    #[test]
    fn galaxy_entry_radius_is_larger_than_the_core() {
        let entries = sample();
        let layout = SectorLayout::build(&entries, &[0, 1, 2, 3, 4]);
        let directory = &layout.nodes[0];
        assert!(galaxy_radius(directory) > directory.radius);
        assert!(auto_enter_radius(directory) < galaxy_radius(directory));
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
