use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::theme::{
    AUTO_ENTER_FACTOR, CONSOLE_LINES, CONSOLE_MAX_LINES, CONSOLE_MIN_LINES, WM_UNFOCUSED_OPACITY,
};

const CONSOLE_HARD_MIN: usize = 2;
const CONSOLE_HARD_MAX: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub start_path: Option<PathBuf>,
    pub show_hidden: bool,
    pub max_entries: usize,
    pub console_lines: usize,
    pub console_min_lines: usize,
    pub console_max_lines: usize,
    pub font_scale: f32,
    pub unfocused_opacity: f32,
    pub auto_enter: bool,
    pub auto_enter_factor: f32,
    pub node_scale: f32,
    pub galaxy_scale: f32,
    #[serde(alias = "auto-hide-mouse")]
    pub auto_hide_mouse: bool,
    pub open_with_system: bool,
    pub enter_shell: bool,
    pub crt: bool,
    pub reduced_motion: bool,
    pub terminal: Option<String>,
    pub shell: Option<String>,
    pub confirm_exec: bool,
    pub theme: String,
    pub log_level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            start_path: None,
            show_hidden: false,
            max_entries: 5_000,
            console_lines: CONSOLE_LINES,
            console_min_lines: CONSOLE_MIN_LINES,
            console_max_lines: CONSOLE_MAX_LINES,
            font_scale: 1.0,
            unfocused_opacity: WM_UNFOCUSED_OPACITY,
            auto_enter: true,
            auto_enter_factor: AUTO_ENTER_FACTOR,
            node_scale: 1.0,
            galaxy_scale: 1.0,
            auto_hide_mouse: true,
            open_with_system: true,
            enter_shell: false,
            crt: true,
            reduced_motion: false,
            terminal: None,
            shell: None,
            confirm_exec: false,
            theme: "default".to_string(),
            log_level: "info".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigLoad {
    pub config: Config,
    pub path: Option<PathBuf>,
    pub warning: Option<String>,
}

pub fn config_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "cbrpnk-cruiser").map(|dirs| dirs.config_dir().join("config.toml"))
}

pub fn load(override_path: Option<&Path>) -> ConfigLoad {
    let path = override_path.map(Path::to_path_buf).or_else(config_path);
    let Some(path) = path else {
        return ConfigLoad {
            config: Config::default(),
            path: None,
            warning: Some("could not determine config directory".to_string()),
        };
    };

    if !path.exists() {
        let warning = override_path.map(|p| format!("config not found: {}", p.display()));
        return ConfigLoad {
            config: Config::default(),
            path: Some(path),
            warning,
        };
    }

    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            return ConfigLoad {
                config: Config::default(),
                path: Some(path),
                warning: Some(format!("could not read config: {error}")),
            }
        }
    };

    match toml::from_str::<Config>(&text) {
        Ok(config) => {
            let (config, warning) = validate(config);
            ConfigLoad {
                config,
                path: Some(path),
                warning,
            }
        }
        Err(error) => ConfigLoad {
            config: Config::default(),
            path: Some(path),
            warning: Some(format!("invalid config, using defaults: {error}")),
        },
    }
}

pub fn validate(mut config: Config) -> (Config, Option<String>) {
    let mut warnings = Vec::new();
    if config.max_entries == 0 || config.max_entries > 100_000 {
        warnings.push(format!(
            "max_entries {} clamped to 5000",
            config.max_entries
        ));
        config.max_entries = config.max_entries.clamp(1, 100_000);
    }
    if !(0.75..=2.0).contains(&config.font_scale) {
        warnings.push(format!("font_scale {} clamped to 1.0", config.font_scale));
        config.font_scale = config.font_scale.clamp(0.75, 2.0);
    }
    if !config.font_scale.is_finite() {
        warnings.push("font_scale is not finite, using 1.0".to_string());
        config.font_scale = 1.0;
    }
    if !config.node_scale.is_finite() {
        warnings.push("node_scale is not finite, using 1.0".to_string());
        config.node_scale = 1.0;
    } else if !(0.5..=2.0).contains(&config.node_scale) {
        let scale = config.node_scale.clamp(0.5, 2.0);
        warnings.push(format!(
            "node_scale {} clamped to {scale}",
            config.node_scale
        ));
        config.node_scale = scale;
    }
    if !config.galaxy_scale.is_finite() {
        warnings.push("galaxy_scale is not finite, using 1.0".to_string());
        config.galaxy_scale = 1.0;
    } else if !(0.4..=2.0).contains(&config.galaxy_scale) {
        let scale = config.galaxy_scale.clamp(0.4, 2.0);
        warnings.push(format!(
            "galaxy_scale {} clamped to {scale}",
            config.galaxy_scale
        ));
        config.galaxy_scale = scale;
    }
    if !config.unfocused_opacity.is_finite() {
        warnings.push(format!(
            "unfocused_opacity is not finite, using {WM_UNFOCUSED_OPACITY}"
        ));
        config.unfocused_opacity = WM_UNFOCUSED_OPACITY;
    } else if !(0.0..=1.0).contains(&config.unfocused_opacity) {
        let opacity = config.unfocused_opacity.clamp(0.0, 1.0);
        warnings.push(format!(
            "unfocused_opacity {} clamped to {opacity}",
            config.unfocused_opacity
        ));
        config.unfocused_opacity = opacity;
    }
    if !config.auto_enter_factor.is_finite() {
        warnings.push(format!(
            "auto_enter_factor is not finite, using {AUTO_ENTER_FACTOR}"
        ));
        config.auto_enter_factor = AUTO_ENTER_FACTOR;
    } else if !(0.0..=1.0).contains(&config.auto_enter_factor) {
        let factor = config.auto_enter_factor.clamp(0.0, 1.0);
        warnings.push(format!(
            "auto_enter_factor {} clamped to {factor}",
            config.auto_enter_factor
        ));
        config.auto_enter_factor = factor;
    }
    if config.theme != "default" {
        warnings.push(format!("unknown theme {:?}, using default", config.theme));
        config.theme = "default".to_string();
    }
    let console_min = config
        .console_min_lines
        .clamp(CONSOLE_HARD_MIN, CONSOLE_HARD_MAX);
    if console_min != config.console_min_lines {
        warnings.push(format!(
            "console_min_lines {} clamped to {console_min}",
            config.console_min_lines
        ));
        config.console_min_lines = console_min;
    }
    let console_max = config
        .console_max_lines
        .clamp(CONSOLE_HARD_MIN, CONSOLE_HARD_MAX);
    if console_max != config.console_max_lines {
        warnings.push(format!(
            "console_max_lines {} clamped to {console_max}",
            config.console_max_lines
        ));
        config.console_max_lines = console_max;
    }
    if config.console_min_lines > config.console_max_lines {
        warnings.push(format!(
            "console_min_lines {} exceeds console_max_lines {}; using {} for both",
            config.console_min_lines, config.console_max_lines, config.console_min_lines
        ));
        config.console_max_lines = config.console_min_lines;
    }
    let console_lines = config
        .console_lines
        .clamp(config.console_min_lines, config.console_max_lines);
    if console_lines != config.console_lines {
        warnings.push(format!(
            "console_lines {} clamped to {console_lines}",
            config.console_lines
        ));
        config.console_lines = console_lines;
    }
    let warning = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("; "))
    };
    (config, warning)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_config_is_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.toml");
        let loaded = load(Some(&path));
        assert_eq!(loaded.config, Config::default());
        assert!(loaded.warning.is_some());
    }

    #[test]
    fn parses_partial_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "show_hidden = true\nmax_entries = 42\n").unwrap();
        let loaded = load(Some(&path));
        assert!(loaded.config.show_hidden);
        assert_eq!(loaded.config.max_entries, 42);
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn configures_console_height() {
        let config = Config {
            console_lines: 40,
            console_min_lines: 6,
            console_max_lines: 48,
            ..Config::default()
        };
        let (config, warning) = validate(config);
        assert_eq!(config.console_lines, 40);
        assert_eq!(config.console_min_lines, 6);
        assert_eq!(config.console_max_lines, 48);
        assert!(warning.is_none());
    }

    #[test]
    fn clamps_bad_console_values() {
        let config = Config {
            console_lines: 500,
            console_min_lines: 0,
            console_max_lines: 500,
            ..Config::default()
        };
        let (config, warning) = validate(config);
        assert_eq!(config.console_min_lines, 2);
        assert_eq!(config.console_max_lines, 64);
        assert_eq!(config.console_lines, 64);
        assert!(warning.is_some());
    }

    #[test]
    fn repairs_inverted_console_bounds() {
        let config = Config {
            console_lines: 12,
            console_min_lines: 30,
            console_max_lines: 20,
            ..Config::default()
        };
        let (config, warning) = validate(config);
        assert_eq!(config.console_min_lines, 30);
        assert_eq!(config.console_max_lines, 30);
        assert_eq!(config.console_lines, 30);
        assert!(warning.is_some());
    }

    #[test]
    fn clamps_bad_values() {
        let config = Config {
            max_entries: 0,
            font_scale: 99.0,
            unfocused_opacity: 2.0,
            ..Config::default()
        };
        let (config, warning) = validate(config);
        assert_eq!(config.max_entries, 1);
        assert_eq!(config.font_scale, 2.0);
        assert_eq!(config.unfocused_opacity, 1.0);
        assert!(warning.is_some());
    }

    #[test]
    fn parses_terminal_visual_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "unfocused_opacity = 0.42\nauto-hide-mouse = false\nopen_with_system = false\nenter_shell = true\n",
        )
        .unwrap();
        let loaded = load(Some(&path));
        assert_eq!(loaded.config.unfocused_opacity, 0.42);
        assert!(!loaded.config.auto_hide_mouse);
        assert!(!loaded.config.open_with_system);
        assert!(loaded.config.enter_shell);
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn configures_auto_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "auto_enter = false\nauto_enter_factor = 0.25\n").unwrap();
        let loaded = load(Some(&path));
        assert!(!loaded.config.auto_enter);
        assert_eq!(loaded.config.auto_enter_factor, 0.25);
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn clamps_auto_enter_factor() {
        let config = Config {
            auto_enter_factor: 3.0,
            ..Config::default()
        };
        let (config, warning) = validate(config);
        assert_eq!(config.auto_enter_factor, 1.0);
        assert!(warning.is_some());

        let config = Config {
            auto_enter_factor: -1.0,
            ..Config::default()
        };
        let (config, warning) = validate(config);
        assert_eq!(config.auto_enter_factor, 0.0);
        assert!(warning.is_some());
    }

    #[test]
    fn configures_node_and_galaxy_scale() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "node_scale = 0.7\ngalaxy_scale = 0.8\n").unwrap();
        let loaded = load(Some(&path));
        assert_eq!(loaded.config.node_scale, 0.7);
        assert_eq!(loaded.config.galaxy_scale, 0.8);
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn clamps_node_and_galaxy_scale() {
        let config = Config {
            node_scale: 3.0,
            galaxy_scale: 0.1,
            ..Config::default()
        };
        let (config, warning) = validate(config);
        assert_eq!(config.node_scale, 2.0);
        assert_eq!(config.galaxy_scale, 0.4);
        assert!(warning.is_some());
    }

    #[test]
    fn replaces_non_finite_node_and_galaxy_scale() {
        let config = Config {
            node_scale: f32::NAN,
            galaxy_scale: f32::INFINITY,
            ..Config::default()
        };
        let (config, warning) = validate(config);
        assert_eq!(config.node_scale, 1.0);
        assert_eq!(config.galaxy_scale, 1.0);
        assert!(warning.is_some());
    }

    #[test]
    fn replaces_non_finite_auto_enter_factor() {
        let config = Config {
            auto_enter_factor: f32::NAN,
            ..Config::default()
        };
        let (config, warning) = validate(config);
        assert_eq!(config.auto_enter_factor, AUTO_ENTER_FACTOR);
        assert!(warning.is_some());
    }

    #[test]
    fn replaces_non_finite_opacity() {
        let config = Config {
            unfocused_opacity: f32::NAN,
            ..Config::default()
        };
        let (config, warning) = validate(config);
        assert_eq!(config.unfocused_opacity, WM_UNFOCUSED_OPACITY);
        assert!(warning.is_some());
    }

    #[test]
    fn invalid_toml_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "not = [valid").unwrap();
        let loaded = load(Some(&path));
        assert_eq!(loaded.config, Config::default());
        assert!(loaded.warning.is_some());
    }
}
