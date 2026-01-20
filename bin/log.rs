use std::fs;

use log::LevelFilter;

use crate::settings;

const FILE_PREFIX: &str = "rad-tui";

pub fn enable() -> Result<(), anyhow::Error> {
    match settings::get_state_path() {
        Ok(path) => {
            fs::create_dir_all(path.clone())?;

            let file = fs::OpenOptions::new()
                .append(true)
                .open(format!("{}/{FILE_PREFIX}.log", path.to_string_lossy()))?;
            simple_logging::log_to(file, LevelFilter::Info);

            Ok(())
        }
        Err(err) => Err(anyhow::Error::from(err)),
    }
}
