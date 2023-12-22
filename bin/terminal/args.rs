use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;

use radicle::cob::{issue, patch};

/// Git revision parameter. Supports extended SHA-1 syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rev(String);

impl From<String> for Rev {
    fn from(value: String) -> Self {
        Rev(value)
    }
}

#[derive(thiserror::Error, Debug)]
#[allow(dead_code)]
pub enum Error {
    /// If this error is returned from argument parsing, help is displayed.
    #[error("help invoked")]
    Help,
    /// If this error is returned from argument parsing, the manual page is displayed.
    #[error("help manual invoked")]
    HelpManual { name: &'static str },
    /// If this error is returned from argument parsing, usage is displayed.
    #[error("usage invoked")]
    Usage,
    /// An error with a hint.
    #[error("{err}")]
    WithHint {
        err: anyhow::Error,
        hint: &'static str,
    },
}

pub struct Help {
    pub name: &'static str,
    pub description: &'static str,
    pub version: &'static str,
    pub usage: &'static str,
}

pub trait Args: Sized {
    fn from_env() -> anyhow::Result<Self> {
        let args: Vec<_> = std::env::args_os().skip(1).collect();

        match Self::from_args(args) {
            Ok((opts, unparsed)) => {
                self::finish(unparsed)?;

                Ok(opts)
            }
            Err(err) => Err(err),
        }
    }

    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)>;
}

#[allow(dead_code)]
pub fn parse_value<T: FromStr>(flag: &str, value: OsString) -> anyhow::Result<T>
where
    <T as FromStr>::Err: std::error::Error,
{
    value
        .into_string()
        .map_err(|_| anyhow!("the value specified for '--{}' is not valid UTF-8", flag))?
        .parse()
        .map_err(|e| anyhow!("invalid value specified for '--{}' ({})", flag, e))
}

#[allow(dead_code)]
pub fn format(arg: lexopt::Arg) -> OsString {
    match arg {
        lexopt::Arg::Long(flag) => format!("--{flag}").into(),
        lexopt::Arg::Short(flag) => format!("-{flag}").into(),
        lexopt::Arg::Value(val) => val,
    }
}

#[allow(dead_code)]
pub fn finish(unparsed: Vec<OsString>) -> anyhow::Result<()> {
    if let Some(arg) = unparsed.first() {
        return Err(anyhow::anyhow!(
            "unexpected argument `{}`",
            arg.to_string_lossy()
        ));
    }
    Ok(())
}

#[allow(dead_code)]
pub fn rev(val: &OsString) -> anyhow::Result<Rev> {
    let s = val.to_str().ok_or(anyhow!("invalid git rev {val:?}"))?;
    Ok(Rev::from(s.to_owned()))
}

#[allow(dead_code)]
pub fn issue(val: &OsString) -> anyhow::Result<issue::IssueId> {
    let val = val.to_string_lossy();
    issue::IssueId::from_str(&val).map_err(|_| anyhow!("invalid Issue ID '{}'", val))
}

#[allow(dead_code)]
pub fn patch(val: &OsString) -> anyhow::Result<patch::PatchId> {
    let val = val.to_string_lossy();
    patch::PatchId::from_str(&val).map_err(|_| anyhow!("invalid Patch ID '{}'", val))
}
