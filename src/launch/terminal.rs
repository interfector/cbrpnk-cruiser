use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCommand {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
}

const CANDIDATES: [&str; 8] = [
    "wezterm",
    "kitty",
    "alacritty",
    "foot",
    "gnome-terminal",
    "konsole",
    "xfce4-terminal",
    "xterm",
];

pub fn detect(config_terminal: Option<&str>, cwd: &Path) -> Option<TerminalCommand> {
    let path_env = std::env::var_os("PATH");
    let config_terminal = config_terminal
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(name) = config_terminal {
        if let Some(program) = find_program(name, path_env.as_deref()) {
            return Some(build_command(&program, cwd));
        }
    }

    if let Some(name) = std::env::var_os("TERMINAL") {
        let name = name.to_string_lossy();
        if let Some(program) = find_program(&name, path_env.as_deref()) {
            return Some(build_command(&program, cwd));
        }
    }

    for candidate in CANDIDATES {
        if let Some(program) = find_program(candidate, path_env.as_deref()) {
            return Some(build_command(&program, cwd));
        }
    }

    None
}

pub fn find_program(name: &str, path_env: Option<&OsStr>) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.components().count() > 1 {
        return path.is_file().then(|| path.to_path_buf());
    }
    let path_env = path_env?;
    for directory in std::env::split_paths(path_env) {
        let candidate = directory.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn build_command(program: &Path, cwd: &Path) -> TerminalCommand {
    let base = program
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let directory = OsString::from(cwd.as_os_str());
    let args = match base.as_str() {
        "wezterm" => vec![OsString::from("start"), OsString::from("--cwd"), directory],
        "kitty" => vec![OsString::from("--directory"), directory],
        "alacritty" => vec![OsString::from("--working-directory"), directory],
        "foot" => vec![OsString::from("--working-directory"), directory],
        "gnome-terminal" => vec![OsString::from("--working-directory"), directory],
        "konsole" => vec![OsString::from("--workdir"), directory],
        "xfce4-terminal" => vec![OsString::from("--working-directory"), directory],
        _ => Vec::new(),
    };
    TerminalCommand {
        program: program.to_path_buf(),
        args,
        cwd: cwd.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_wezterm_arguments() {
        let command = build_command(Path::new("/usr/bin/wezterm"), Path::new("/tmp/x"));
        assert_eq!(
            command.args,
            vec![
                OsString::from("start"),
                OsString::from("--cwd"),
                OsString::from("/tmp/x")
            ]
        );
    }

    #[test]
    fn builds_xterm_without_arguments() {
        let command = build_command(Path::new("/usr/bin/xterm"), Path::new("/tmp/x"));
        assert!(command.args.is_empty());
        assert_eq!(command.cwd, PathBuf::from("/tmp/x"));
    }

    #[test]
    fn finds_absolute_program() {
        let path = Path::new("/bin/sh");
        assert_eq!(find_program("/bin/sh", None), Some(path.to_path_buf()));
    }
}
