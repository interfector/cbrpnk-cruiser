use std::cmp::Ordering;
use std::ffi::OsString;
use std::fs::DirEntry;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::util::{is_hidden_name, natural_cmp, os_cmp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

impl EntryKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Directory => "SECTOR",
            Self::File => "DATA",
            Self::Symlink => "PORTAL",
            Self::Other => "SPECIAL",
        }
    }

    fn rank(self, executable: bool) -> u8 {
        match self {
            Self::Directory => 0,
            Self::File if executable => 1,
            Self::File => 2,
            Self::Symlink => 3,
            Self::Other => 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: OsString,
    pub path: PathBuf,
    pub kind: EntryKind,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub mode: Option<u32>,
    pub executable: bool,
    pub readable: bool,
    pub link_target: Option<PathBuf>,
    pub link_target_is_dir: Option<bool>,
    pub link_broken: bool,
    pub metadata_error: Option<String>,
    pub hidden: bool,
}

impl Entry {
    pub fn from_dir_entry(entry: &DirEntry) -> Self {
        let name = entry.file_name();
        let path = entry.path();
        let hidden = is_hidden_name(&name);
        let metadata = std::fs::symlink_metadata(&path);

        let mut result = Self {
            name,
            path: path.clone(),
            kind: EntryKind::Other,
            size: 0,
            modified: None,
            mode: None,
            executable: false,
            readable: false,
            link_target: None,
            link_target_is_dir: None,
            link_broken: false,
            metadata_error: None,
            hidden,
        };

        let metadata = match metadata {
            Ok(metadata) => metadata,
            Err(error) => {
                result.metadata_error = Some(error.to_string());
                return result;
            }
        };

        result.size = metadata.len();
        result.modified = metadata.modified().ok();
        let file_type = metadata.file_type();
        result.kind = if file_type.is_symlink() {
            EntryKind::Symlink
        } else if file_type.is_dir() {
            EntryKind::Directory
        } else if file_type.is_file() {
            EntryKind::File
        } else {
            EntryKind::Other
        };

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let mode = metadata.mode();
            result.mode = Some(mode);
            result.executable = result.kind == EntryKind::File && mode & 0o111 != 0;
            result.readable = match result.kind {
                EntryKind::Directory => mode & 0o444 != 0 && mode & 0o111 != 0,
                _ => mode & 0o444 != 0,
            };
        }
        #[cfg(not(unix))]
        {
            result.readable = true;
            result.executable = false;
        }

        if result.kind == EntryKind::Symlink {
            result.link_target = std::fs::read_link(&path).ok();
            match std::fs::metadata(&path) {
                Ok(target) => {
                    result.link_target_is_dir = Some(target.is_dir());
                    result.executable = target.is_file() && is_executable(&target);
                    result.readable = true;
                }
                Err(error) => {
                    result.link_broken = true;
                    result.metadata_error = Some(error.to_string());
                    result.readable = false;
                }
            }
        }

        result
    }

    pub fn display_name(&self) -> String {
        self.name.to_string_lossy().into_owned()
    }

    pub fn kind_label(&self) -> &'static str {
        match (self.kind, self.executable) {
            (EntryKind::File, true) => "PROGRAM",
            (kind, _) => kind.label(),
        }
    }

    pub fn is_dir_like(&self) -> bool {
        self.kind == EntryKind::Directory
            || (self.kind == EntryKind::Symlink && self.link_target_is_dir == Some(true))
    }
}

#[cfg(not(unix))]
fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

pub fn sort_entries(entries: &mut [Entry]) {
    entries.sort_by(|a, b| {
        a.kind
            .rank(a.executable)
            .cmp(&b.kind.rank(b.executable))
            .then_with(|| natural_cmp(&a.name.to_string_lossy(), &b.name.to_string_lossy()))
            .then_with(|| os_cmp(&a.name, &b.name))
    });
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for Entry {}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.path.cmp(&other.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_entry(name: &str, kind: EntryKind, executable: bool) -> Entry {
        Entry {
            name: OsString::from(name),
            path: PathBuf::from(name),
            kind,
            size: 0,
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

    #[test]
    fn sorts_directories_then_executables_then_files() {
        let mut entries = vec![
            test_entry("b.txt", EntryKind::File, false),
            test_entry("run.sh", EntryKind::File, true),
            test_entry("dir2", EntryKind::Directory, false),
            test_entry("dir10", EntryKind::Directory, false),
        ];
        sort_entries(&mut entries);
        let names: Vec<_> = entries.iter().map(Entry::display_name).collect();
        assert_eq!(names, ["dir2", "dir10", "run.sh", "b.txt"]);
    }
}
