use std::cmp::Ordering;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn path_hash(path: &Path) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        fnv1a64(path.as_os_str().as_bytes())
    }
    #[cfg(not(unix))]
    {
        fnv1a64(path.to_string_lossy().as_bytes())
    }
}

pub fn path_display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn expand_tilde(input: &str) -> PathBuf {
    if input == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(input));
    }
    if let Some(rest) = input.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(input)
}

pub fn is_hidden_name(name: &OsStr) -> bool {
    let text = name.to_string_lossy();
    text.starts_with('.') && text != "." && text != ".."
}

pub fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            std::path::Component::RootDir => out.push(component.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if out.file_name().is_some() {
                    out.pop();
                } else if !out.has_root() {
                    out.push(component.as_os_str());
                }
            }
            std::path::Component::Normal(part) => out.push(part),
        }
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

pub fn parse_window_size(value: &str) -> Option<(i32, i32)> {
    let (width, height) = value.split_once('x')?;
    let width = width.parse().ok()?;
    let height = height.parse().ok()?;
    (width > 0 && height > 0).then_some((width, height))
}

pub fn format_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    const TIB: f64 = GIB * 1024.0;
    let value = bytes as f64;
    if value >= TIB {
        format!("{:.1} TiB", value / TIB)
    } else if value >= GIB {
        format!("{:.1} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.1} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.1} KiB", value / KIB)
    } else {
        format!("{bytes} B")
    }
}

pub fn format_time(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return "-".to_string();
    };
    let datetime: chrono::DateTime<chrono::Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M").to_string()
}

pub fn format_mode(mode: Option<u32>) -> String {
    let Some(mode) = mode else {
        return "-".to_string();
    };
    let mut text = String::with_capacity(10);
    text.push(if mode & 0o400 != 0 { 'r' } else { '-' });
    text.push(if mode & 0o200 != 0 { 'w' } else { '-' });
    text.push(if mode & 0o100 != 0 { 'x' } else { '-' });
    text.push(if mode & 0o040 != 0 { 'r' } else { '-' });
    text.push(if mode & 0o020 != 0 { 'w' } else { '-' });
    text.push(if mode & 0o010 != 0 { 'x' } else { '-' });
    text.push(if mode & 0o004 != 0 { 'r' } else { '-' });
    text.push(if mode & 0o002 != 0 { 'w' } else { '-' });
    text.push(if mode & 0o001 != 0 { 'x' } else { '-' });
    text
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NaturalToken {
    Text(String),
    Number(String),
}

fn natural_tokens(text: &str) -> Vec<NaturalToken> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        if chars[index].is_ascii_digit() {
            let start = index;
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
            }
            let raw: String = chars[start..index].iter().collect();
            let trimmed = raw.trim_start_matches('0');
            tokens.push(NaturalToken::Number(if trimmed.is_empty() {
                "0".to_string()
            } else {
                trimmed.to_string()
            }));
        } else {
            let start = index;
            while index < chars.len() && !chars[index].is_ascii_digit() {
                index += 1;
            }
            let raw: String = chars[start..index].iter().collect();
            tokens.push(NaturalToken::Text(raw.to_lowercase()));
        }
    }
    tokens
}

pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let left = natural_tokens(a);
    let right = natural_tokens(b);
    for (lhs, rhs) in left.iter().zip(right.iter()) {
        let ordering = match (lhs, rhs) {
            (NaturalToken::Number(lhs), NaturalToken::Number(rhs)) => lhs
                .len()
                .cmp(&rhs.len())
                .then_with(|| lhs.as_bytes().cmp(rhs.as_bytes())),
            (NaturalToken::Text(lhs), NaturalToken::Text(rhs)) => lhs.cmp(rhs),
            (NaturalToken::Text(_), NaturalToken::Number(_)) => Ordering::Less,
            (NaturalToken::Number(_), NaturalToken::Text(_)) => Ordering::Greater,
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len()).then_with(|| a.cmp(b))
}

pub fn os_cmp(a: &OsStr, b: &OsStr) -> Ordering {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        a.as_bytes().cmp(b.as_bytes())
    }
    #[cfg(not(unix))]
    {
        a.to_string_lossy().cmp(&b.to_string_lossy())
    }
}

pub fn elide(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let mut out: String = text.chars().take(max_chars - 1).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_sort_orders_numbers() {
        assert_eq!(natural_cmp("file2", "file10"), Ordering::Less);
        assert_eq!(natural_cmp("File10", "file10"), Ordering::Less);
    }

    #[test]
    fn parses_window_size() {
        assert_eq!(parse_window_size("1920x1080"), Some((1920, 1080)));
        assert_eq!(parse_window_size("0x1080"), None);
        assert_eq!(parse_window_size("1920x0"), None);
        assert_eq!(parse_window_size("1920"), None);
        assert_eq!(parse_window_size("wide"), None);
    }

    #[test]
    fn size_formatting() {
        assert_eq!(format_size(12), "12 B");
        assert_eq!(format_size(2048), "2.0 KiB");
    }

    #[test]
    fn mode_formatting() {
        assert_eq!(format_mode(Some(0o755)), "rwxr-xr-x");
    }

    #[test]
    fn elide_keeps_short_text() {
        assert_eq!(elide("abc", 5), "abc");
        assert_eq!(elide("abcdef", 4), "abc…");
    }

    #[test]
    fn hash_is_stable() {
        assert_eq!(fnv1a64(b"hello"), 0xa430d84680aabd0b);
    }

    #[test]
    fn normalizes_dot_and_parent_components() {
        assert_eq!(
            normalize_path(Path::new("/sector/./link/../data")),
            PathBuf::from("/sector/data")
        );
        assert_eq!(
            normalize_path(Path::new("/../sector/../../data")),
            PathBuf::from("/data")
        );
        assert_eq!(normalize_path(Path::new("")), PathBuf::from("."));
    }
}
