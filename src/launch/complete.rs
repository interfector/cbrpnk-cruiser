use std::fs;
use std::path::{Path, PathBuf};

use crate::util::{expand_tilde, home_dir, is_hidden_name, natural_cmp};

const BUILTINS: [(&str, bool); 10] = [
    ("cd", true),
    ("help", false),
    ("home", false),
    ("ls", true),
    ("open", true),
    ("quit", false),
    ("reload", false),
    ("root", false),
    ("run", true),
    ("term", true),
];

const MAX_SCAN_ENTRIES: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathMode {
    Directories,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub line: String,
    pub candidates: Vec<String>,
}

pub fn complete_line(line: &str, cwd: &Path, show_hidden: bool) -> Option<Completion> {
    if line.starts_with('!') {
        return None;
    }
    let token_start = current_token_start(line);
    let prefix = &line[..token_start];
    let token = &line[token_start..];
    let words = shell_words::split(prefix.trim()).ok()?;

    if words.is_empty() {
        return Some(complete_builtin(line, token_start, token));
    }
    if words.len() != 1 {
        return None;
    }
    let mode = path_mode(&words[0])?;
    Some(complete_path(
        line,
        token_start,
        token,
        cwd,
        mode,
        show_hidden,
    ))
}

fn complete_builtin(line: &str, token_start: usize, token: &str) -> Completion {
    let fragment = token_value(token);
    let candidates: Vec<String> = BUILTINS
        .iter()
        .filter(|(name, _)| name.starts_with(&fragment))
        .map(|(name, _)| (*name).to_string())
        .collect();

    if candidates.len() == 1 {
        let name = candidates[0].clone();
        let takes_path = BUILTINS
            .iter()
            .find(|(builtin, _)| *builtin == name)
            .is_some_and(|(_, takes_path)| *takes_path);
        let mut replacement = shell_words::quote(&name).into_owned();
        if takes_path {
            replacement.push(' ');
        }
        return Completion {
            line: replace_token(line, token_start, &replacement),
            candidates,
        };
    }

    let line = extend_common_prefix(line, token_start, &fragment, &candidates);
    Completion { line, candidates }
}

fn complete_path(
    line: &str,
    token_start: usize,
    token: &str,
    cwd: &Path,
    mode: PathMode,
    show_hidden: bool,
) -> Completion {
    let fragment = token_value(token);
    let (dir_display, name_prefix, lookup_dir) = split_path(&fragment, cwd);
    let include_hidden = show_hidden || fragment.split('/').any(|part| part.starts_with('.'));

    let mut paths = Vec::new();
    if let Ok(reader) = fs::read_dir(&lookup_dir) {
        for entry in reader.filter_map(Result::ok).take(MAX_SCAN_ENTRIES) {
            let name = entry.file_name();
            let name_text = name.to_string_lossy();
            if !name_text.starts_with(&name_prefix) {
                continue;
            }
            if !include_hidden && is_hidden_name(&name) {
                continue;
            }
            let file_type = entry.file_type().ok();
            let is_dir = file_type.is_some_and(|kind| kind.is_dir())
                || file_type.is_some_and(|kind| kind.is_symlink() && entry.path().is_dir());
            if mode == PathMode::Directories && !is_dir {
                continue;
            }
            let mut display = format!("{dir_display}{name_text}");
            if is_dir {
                display.push('/');
            }
            paths.push(PathCandidate { display, is_dir });
        }
    }
    paths.sort_by(|lhs, rhs| natural_cmp(&lhs.display, &rhs.display));

    let candidates: Vec<String> = paths.iter().map(|path| path.display.clone()).collect();
    if paths.len() == 1 {
        let path = &paths[0];
        let mut replacement = shell_words::quote(&path.display).into_owned();
        if !path.is_dir {
            replacement.push(' ');
        }
        return Completion {
            line: replace_token(line, token_start, &replacement),
            candidates,
        };
    }

    let line = extend_common_prefix(line, token_start, &fragment, &candidates);
    Completion { line, candidates }
}

struct PathCandidate {
    display: String,
    is_dir: bool,
}

fn path_mode(command: &str) -> Option<PathMode> {
    match command {
        "cd" | "term" => Some(PathMode::Directories),
        "ls" | "open" | "run" => Some(PathMode::Any),
        _ => None,
    }
}

fn split_path(fragment: &str, cwd: &Path) -> (String, String, PathBuf) {
    if fragment == "~" {
        let home = home_dir().unwrap_or_else(|| PathBuf::from("~"));
        return ("~/".to_string(), String::new(), home);
    }
    if let Some(slash) = fragment.rfind('/') {
        let dir_display = fragment[..=slash].to_string();
        let name_prefix = fragment[slash + 1..].to_string();
        let lookup_raw = &fragment[..slash];
        let lookup = if dir_display == "/" {
            PathBuf::from("/")
        } else if lookup_raw.is_empty() {
            cwd.to_path_buf()
        } else if lookup_raw == "~" {
            home_dir().unwrap_or_else(|| cwd.to_path_buf())
        } else {
            let expanded = expand_tilde(lookup_raw);
            if expanded.is_absolute() {
                expanded
            } else {
                cwd.join(expanded)
            }
        };
        (dir_display, name_prefix, lookup)
    } else {
        (String::new(), fragment.to_string(), cwd.to_path_buf())
    }
}

fn current_token_start(line: &str) -> usize {
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && quote != Some('\'') {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if character == active {
                quote = None;
            }
            continue;
        }
        if character == '\'' || character == '"' {
            quote = Some(character);
        } else if character.is_whitespace() {
            start = index + character.len_utf8();
        }
    }
    start
}

fn token_value(token: &str) -> String {
    let mut value = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in token.chars() {
        if escaped {
            value.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' && quote != Some('\'') {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if character == active {
                quote = None;
            } else {
                value.push(character);
            }
            continue;
        }
        if character == '\'' || character == '"' {
            quote = Some(character);
        } else {
            value.push(character);
        }
    }
    if escaped {
        value.push('\\');
    }
    value
}

fn replace_token(line: &str, token_start: usize, replacement: &str) -> String {
    format!("{}{replacement}", &line[..token_start])
}

fn extend_common_prefix(
    line: &str,
    token_start: usize,
    fragment: &str,
    candidates: &[String],
) -> String {
    let Some(common) = longest_common_prefix(candidates) else {
        return line.to_string();
    };
    if common.starts_with(fragment) && common.chars().count() > fragment.chars().count() {
        replace_token(line, token_start, &shell_words::quote(&common))
    } else {
        line.to_string()
    }
}

fn longest_common_prefix(candidates: &[String]) -> Option<String> {
    let first = candidates.first()?;
    let mut end = first.len();
    for candidate in &candidates[1..] {
        end = first
            .char_indices()
            .take_while(|(index, character)| {
                *index < end && candidate[*index..].starts_with(*character)
            })
            .map(|(index, character)| index + character.len_utf8())
            .last()
            .unwrap_or(0);
        if end == 0 {
            return Some(String::new());
        }
    }
    Some(first[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("docs")).unwrap();
        fs::create_dir(dir.path().join("my dir")).unwrap();
        fs::create_dir(dir.path().join("alpha")).unwrap();
        fs::create_dir(dir.path().join("alpha/beta")).unwrap();
        fs::create_dir(dir.path().join(".secret")).unwrap();
        fs::write(dir.path().join("data.txt"), b"data").unwrap();
        dir
    }

    #[test]
    fn completes_unique_builtin_and_adds_slash_or_space() {
        let dir = tempfile::tempdir().unwrap();
        let cd = complete_line("c", dir.path(), false).unwrap();
        assert_eq!(cd.line, "cd ");
        assert_eq!(cd.candidates, vec!["cd"]);

        let help = complete_line("hel", dir.path(), false).unwrap();
        assert_eq!(help.line, "help");
        assert_eq!(help.candidates, vec!["help"]);
    }

    #[test]
    fn lists_ambiguous_builtins_without_guessing() {
        let dir = tempfile::tempdir().unwrap();
        let completion = complete_line("r", dir.path(), false).unwrap();
        assert_eq!(completion.line, "r");
        assert_eq!(completion.candidates, vec!["reload", "root", "run"]);
    }

    #[test]
    fn completes_directories_and_files_by_mode() {
        let dir = fixture();
        let cd = complete_line("cd do", dir.path(), false).unwrap();
        assert_eq!(cd.line, "cd docs/");

        let cd_file = complete_line("cd da", dir.path(), false).unwrap();
        assert!(cd_file.candidates.is_empty());
        assert_eq!(cd_file.line, "cd da");

        let open = complete_line("open da", dir.path(), false).unwrap();
        assert_eq!(open.line, "open data.txt ");
    }

    #[test]
    fn quotes_paths_with_spaces() {
        let dir = fixture();
        let completion = complete_line("cd my", dir.path(), false).unwrap();
        assert_eq!(completion.line, "cd 'my dir/'");
    }

    #[test]
    fn completes_nested_paths_without_expanding_the_display_prefix() {
        let dir = fixture();
        let completion = complete_line("cd alpha/be", dir.path(), false).unwrap();
        assert_eq!(completion.line, "cd alpha/beta/");
    }

    #[test]
    fn includes_hidden_entries_only_when_asked_for() {
        let dir = fixture();
        let hidden = complete_line("cd .s", dir.path(), false).unwrap();
        assert_eq!(hidden.line, "cd .secret/");

        let hidden_by_default = complete_line("cd ", dir.path(), false).unwrap();
        assert!(!hidden_by_default
            .candidates
            .iter()
            .any(|candidate| candidate == ".secret/"));

        let shown = complete_line("cd ", dir.path(), true).unwrap();
        assert!(shown
            .candidates
            .iter()
            .any(|candidate| candidate == ".secret/"));
    }

    #[test]
    fn ignores_external_commands_and_shell_mode() {
        let dir = fixture();
        assert!(complete_line("git ch", dir.path(), false).is_none());
        assert!(complete_line("!ls ", dir.path(), false).is_none());
        assert!(complete_line("cd docs extra ", dir.path(), false).is_none());
    }

    #[test]
    fn extends_shared_prefix_when_several_paths_match() {
        let dir = fixture();
        fs::create_dir(dir.path().join("documents")).unwrap();
        let completion = complete_line("cd doc", dir.path(), false).unwrap();
        assert_eq!(completion.line, "cd doc");
        assert_eq!(completion.candidates, vec!["docs/", "documents/"]);
    }

    #[test]
    fn handles_partial_quotes() {
        let dir = fixture();
        let completion = complete_line("cd 'my", dir.path(), false).unwrap();
        assert_eq!(completion.line, "cd 'my dir/'");
    }
}
