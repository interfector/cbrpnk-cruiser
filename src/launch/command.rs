use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedLine {
    pub raw: String,
    pub shell_mode: bool,
    pub words: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Builtin(Builtin),
    External(Vec<String>),
    Shell(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Builtin {
    Cd(Option<String>),
    Ls(Option<String>),
    Open(PathBuf),
    Run { path: PathBuf, args: Vec<String> },
    Term(Option<String>),
    Reload,
    Home,
    Root,
    Help,
    Quit,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CommandError {
    #[error("empty command")]
    Empty,
    #[error("unbalanced quotes: {0}")]
    Parse(String),
    #[error("{0} requires a path")]
    MissingPath(&'static str),
    #[error("{0} takes at most one argument")]
    TooManyArguments(&'static str),
    #[error("shell mode requires a command")]
    EmptyShell,
}

pub fn parse_line(line: &str) -> Result<Command, CommandError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err(CommandError::Empty);
    }
    if let Some(shell) = trimmed.strip_prefix('!') {
        let shell = shell.trim();
        if shell.is_empty() {
            return Err(CommandError::EmptyShell);
        }
        return Ok(Command::Shell(shell.to_string()));
    }
    let words =
        shell_words::split(trimmed).map_err(|error| CommandError::Parse(error.to_string()))?;
    if words.is_empty() {
        return Err(CommandError::Empty);
    }
    classify(&words)
}

fn classify(words: &[String]) -> Result<Command, CommandError> {
    let name = words[0].as_str();
    let args = &words[1..];
    match name {
        "cd" => {
            if args.len() > 1 {
                return Err(CommandError::TooManyArguments("cd"));
            }
            Ok(Command::Builtin(Builtin::Cd(args.first().cloned())))
        }
        "ls" => {
            if args.len() == 1 && !args[0].starts_with('-') {
                Ok(Command::Builtin(Builtin::Ls(Some(args[0].clone()))))
            } else if args.is_empty() {
                Ok(Command::Builtin(Builtin::Ls(None)))
            } else {
                Ok(Command::External(words.to_vec()))
            }
        }
        "open" => {
            let Some(path) = args.first() else {
                return Err(CommandError::MissingPath("open"));
            };
            if args.len() > 1 {
                return Err(CommandError::TooManyArguments("open"));
            }
            Ok(Command::Builtin(Builtin::Open(PathBuf::from(path))))
        }
        "run" => {
            let Some(path) = args.first() else {
                return Err(CommandError::MissingPath("run"));
            };
            Ok(Command::Builtin(Builtin::Run {
                path: PathBuf::from(path),
                args: args.iter().skip(1).cloned().collect(),
            }))
        }
        "term" => {
            if args.len() > 1 {
                return Err(CommandError::TooManyArguments("term"));
            }
            Ok(Command::Builtin(Builtin::Term(args.first().cloned())))
        }
        "reload" => no_arguments(args, "reload").map(|_| Command::Builtin(Builtin::Reload)),
        "home" => no_arguments(args, "home").map(|_| Command::Builtin(Builtin::Home)),
        "root" => no_arguments(args, "root").map(|_| Command::Builtin(Builtin::Root)),
        "help" => no_arguments(args, "help").map(|_| Command::Builtin(Builtin::Help)),
        "quit" => no_arguments(args, "quit").map(|_| Command::Builtin(Builtin::Quit)),
        _ => Ok(Command::External(words.to_vec())),
    }
}

fn no_arguments(args: &[String], name: &'static str) -> Result<(), CommandError> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(CommandError::TooManyArguments(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_builtins() {
        assert_eq!(
            parse_line("cd /tmp").unwrap(),
            Command::Builtin(Builtin::Cd(Some("/tmp".to_string())))
        );
        assert_eq!(
            parse_line("run ./a b c").unwrap(),
            Command::Builtin(Builtin::Run {
                path: PathBuf::from("./a"),
                args: vec!["b".to_string(), "c".to_string()]
            })
        );
    }

    #[test]
    fn leaves_ls_flags_to_external() {
        assert_eq!(
            parse_line("ls -la").unwrap(),
            Command::External(vec!["ls".to_string(), "-la".to_string()])
        );
        assert_eq!(
            parse_line("ls /tmp").unwrap(),
            Command::Builtin(Builtin::Ls(Some("/tmp".to_string())))
        );
    }

    #[test]
    fn handles_quotes_with_shell_words() {
        let command = parse_line("open \"/tmp/a b\"").unwrap();
        assert_eq!(
            command,
            Command::Builtin(Builtin::Open(PathBuf::from("/tmp/a b")))
        );
    }

    #[test]
    fn shell_mode_is_explicit() {
        assert_eq!(
            parse_line("!ls -la | less").unwrap(),
            Command::Shell("ls -la | less".to_string())
        );
    }

    #[test]
    fn reports_bad_input() {
        assert_eq!(parse_line("  ").unwrap_err(), CommandError::Empty);
        assert_eq!(
            parse_line("open").unwrap_err(),
            CommandError::MissingPath("open")
        );
        assert_eq!(parse_line("!").unwrap_err(), CommandError::EmptyShell);
        assert!(parse_line("open \"unterminated").is_err());
    }
}
