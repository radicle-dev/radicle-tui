use log::LevelFilter;

use radicle::profile::Profile;

pub fn enable(profile: &Profile, cmd: &str, op: &str) -> Result<(), anyhow::Error> {
    let logfile = format!(
        "{}/rad-tui-{}-{}.log",
        profile.home().path().to_string_lossy(),
        cmd,
        op,
    );
    simple_logging::log_to_file(logfile, LevelFilter::Info)?;

    Ok(())
}
