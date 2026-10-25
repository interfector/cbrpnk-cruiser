use std::ffi::OsString;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cli {
    pub path: Option<PathBuf>,
    pub config: Option<PathBuf>,
    pub no_crt: bool,
    pub hidden: bool,
    pub help: bool,
    pub version: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CliError {
    #[error("missing value for {0}")]
    MissingValue(&'static str),
    #[error("unknown option: {0}")]
    UnknownOption(String),
    #[error("more than one path supplied")]
    MultiplePaths,
}

pub const HELP: &str = "\
CBRPNK//CRUISER - filesystem cruiser and launcher

Usage:
  cbrpnk-cruiser [PATH] [OPTIONS]

Options:
  --config <PATH>   Load configuration from PATH
  --no-crt          Disable bloom/scanline post-processing
  --hidden          Show hidden files at startup
  -h, --help        Print this help
  -V, --version     Print version

Controls:
  Arrows / pointer  Aim / steer (pointer is joystick-style in Cruise)
  Left/right mouse  Thrust forward / reverse
  W/S/A/D/Q/E       Move
  Shift             Boost
  Tab               Toggle lock mode
  J/K               Previous / next node
  Enter             Enter directory / launch / open (including PARENT gate)
  Backspace         Parent directory
  ~                 Home directory
  /                 Filter current sector
  :                 Command console
  T                 Terminal at current directory
  O                 Open with system handler
  R                 Recenter
  F                 Freeze motion
  F5                Rescan
  Ctrl+H            Toggle hidden files
  F1 or ?           Help
  Esc               Close overlay / unlock / quit when unlocked
  Ctrl+Q            Quit
";

pub fn parse_args<I>(args: I) -> Result<Cli, CliError>
where
    I: IntoIterator<Item = OsString>,
{
    let mut cli = Cli::default();
    let mut args = args.into_iter();
    let mut positional = false;

    while let Some(arg) = args.next() {
        if !positional {
            let text = arg.to_string_lossy();
            match text.as_ref() {
                "--" => {
                    positional = true;
                    continue;
                }
                "-h" | "--help" => {
                    cli.help = true;
                    continue;
                }
                "-V" | "--version" => {
                    cli.version = true;
                    continue;
                }
                "--no-crt" => {
                    cli.no_crt = true;
                    continue;
                }
                "--hidden" => {
                    cli.hidden = true;
                    continue;
                }
                "--config" => {
                    let value = args.next().ok_or(CliError::MissingValue("--config"))?;
                    cli.config = Some(PathBuf::from(value));
                    continue;
                }
                _ => {}
            }

            if let Some(value) = text.strip_prefix("--config=") {
                if value.is_empty() {
                    return Err(CliError::MissingValue("--config"));
                }
                cli.config = Some(PathBuf::from(value));
                continue;
            }

            if text.starts_with('-') && text != "-" {
                return Err(CliError::UnknownOption(text.into_owned()));
            }
        }

        if cli.path.is_some() {
            return Err(CliError::MultiplePaths);
        }
        cli.path = Some(PathBuf::from(arg));
    }

    Ok(cli)
}

pub fn parse_env() -> Result<Cli, CliError> {
    parse_args(std::env::args_os().skip(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn parses_path_and_flags() {
        let cli = parse_args(args(&["/tmp", "--no-crt", "--hidden"])).unwrap();
        assert_eq!(cli.path, Some(PathBuf::from("/tmp")));
        assert!(cli.no_crt);
        assert!(cli.hidden);
    }

    #[test]
    fn parses_config_both_forms() {
        let cli = parse_args(args(&["--config", "a.toml", "--config=b.toml"])).unwrap();
        assert_eq!(cli.config, Some(PathBuf::from("b.toml")));
    }

    #[test]
    fn rejects_unknown_option() {
        let err = parse_args(args(&["--wat"])).unwrap_err();
        assert_eq!(err, CliError::UnknownOption("--wat".to_string()));
    }

    #[test]
    fn rejects_more_than_one_path() {
        let err = parse_args(args(&["a", "b"])).unwrap_err();
        assert_eq!(err, CliError::MultiplePaths);
    }
}
