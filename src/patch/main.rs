mod app;

use std::process;

use anyhow::anyhow;

use radicle::profile;

use log::info;
use log::LevelFilter;

use radicle_term as term;
use radicle_tui as tui;

use tui::context;
use tui::Window;

pub const NAME: &str = "rad-patch-tui";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GIT_HEAD: &str = env!("GIT_HEAD");
pub const FPS: u64 = 60;

pub const HELP: &str = r#"
Usage

    rad-patch-tui [<option>...]

Options

    --version       Print version
    --help          Print help

"#;

struct Options;

impl Options {
    fn from_env() -> Result<Self, anyhow::Error> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_env();

        while let Some(arg) = parser.next()? {
            match arg {
                Long("version") => {
                    println!("{NAME} {VERSION}+{GIT_HEAD}");
                    process::exit(0);
                }
                Long("help") | Short('h') => {
                    println!("{HELP}");
                    process::exit(0);
                }
                _ => anyhow::bail!(arg.unexpected()),
            }
        }

        Ok(Self {})
    }
}

fn execute() -> anyhow::Result<()> {
    let _ = Options::from_env()?;

    let (_, id) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;
    let context = context::Context::new(id)?;

    let logfile = format!(
        "{}/rad-patch-tui.log",
        profile::home()?.path().to_string_lossy()
    );
    simple_logging::log_to_file(logfile, LevelFilter::Info)?;
    info!("Launching window...");

    let mut window = Window::default();
    window.run(&mut app::App::new(context), 1000 / FPS)?;

    Ok(())
}

fn main() {
    if let Err(err) = execute() {
        term::error(format!("Error: rad-patch-tui: {err}"));
        process::exit(1);
    }
}
