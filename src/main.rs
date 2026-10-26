use cbrpnk_cruiser::app::App;
use cbrpnk_cruiser::cli::{self, HELP};
use cbrpnk_cruiser::config;
use cbrpnk_cruiser::render::Renderer;
use cbrpnk_cruiser::util;

fn main() {
    let cli = match cli::parse_env() {
        Ok(cli) => cli,
        Err(error) => {
            eprintln!("cbrpnk-cruiser: {error}");
            eprintln!("try --help");
            std::process::exit(2);
        }
    };
    if cli.help {
        print!("{HELP}");
        return;
    }
    if cli.version {
        println!("cbrpnk-cruiser {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let loaded = config::load(cli.config.as_deref());
    if let Some(warning) = &loaded.warning {
        eprintln!("cbrpnk-cruiser: {warning}");
    }
    init_logger(&loaded.config.log_level);
    let crt = loaded.config.crt && !cli.no_crt;
    let font_scale = loaded.config.font_scale;

    let session = std::env::var_os("CBRPNK_SESSION").is_some();
    let session_size = std::env::var("CBRPNK_SESSION_SIZE")
        .ok()
        .and_then(|value| util::parse_window_size(&value));

    let mut window = macroquad::conf::Conf::default();
    window.miniquad_conf.window_title = "CBRPNK//CRUISER".to_string();
    let (window_width, window_height) = session_size.unwrap_or((1280, 800));
    window.miniquad_conf.window_width = window_width;
    window.miniquad_conf.window_height = window_height;
    window.miniquad_conf.window_resizable = !session;
    window.miniquad_conf.fullscreen = session;
    window.miniquad_conf.high_dpi = true;
    window.miniquad_conf.sample_count = 4;

    macroquad::Window::from_config(window, async move {
        if session {
            macroquad::miniquad::window::set_window_position(0, 0);
        }
        let renderer = Renderer::new(font_scale, crt);
        let app = App::new(cli, loaded, renderer);
        app.run().await;
    });
}

fn init_logger(level: &str) {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    if std::env::var_os("RUST_LOG").is_none() {
        let filter = match level.to_ascii_lowercase().as_str() {
            "trace" => log::LevelFilter::Trace,
            "debug" => log::LevelFilter::Debug,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            "off" => log::LevelFilter::Off,
            _ => log::LevelFilter::Info,
        };
        builder.filter_level(filter);
    }
    builder.try_init().ok();
}
