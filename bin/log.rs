use std::fs;
use std::time::SystemTime;

use anyhow::bail;

use homedir::my_home;

use log::LevelFilter;

const PATH: &str = ".radicle-tui/logs";
const PREFIX: &str = "rad-tui-";

/// Enables logging to `$HOME/.radicle-tui/logs/`. Creates folder if it does not
/// exist.
pub fn enable() -> Result<(), anyhow::Error> {
    match my_home()? {
        Some(home) => {
            let path = format!("{}/{}", home.to_string_lossy(), PATH);
            let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;

            fs::create_dir_all(path.clone())?;

            simple_logging::log_to_file(
                format!("{path}/{PREFIX}{}.log", now.as_millis()),
                LevelFilter::Info,
            )?;

            Ok(())
        }
        None => bail!("Failed to read home directory"),
    }
}
