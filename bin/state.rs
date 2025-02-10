use std::path::PathBuf;
use std::{fmt::Display, fs};

use anyhow::Result;

use homedir::my_home;

use serde::{Deserialize, Serialize};

use radicle::cob::ObjectId;
use radicle::identity::RepoId;

const PATH: &str = ".radicle-tui/states";

/// Converts bytes to a deserializable type.
pub fn from_json<'a, D>(bytes: &'a [u8]) -> Result<D>
where
    D: Deserialize<'a>,
{
    Ok(serde_json::from_slice(bytes)?)
}

/// Converts serializable type to bytes.
pub fn to_json<S>(state: S) -> Result<Vec<u8>>
where
    S: Serialize,
{
    Ok(serde_json::to_vec(&state)?)
}

/// Trait for state readers.
pub trait ReadState {
    fn read(&self) -> Result<Vec<u8>>;
}

/// Trait for state writers.
pub trait WriteState: ReadState {
    fn write(&self, bytes: &[u8]) -> Result<()>;
}

/// A state storage that reads from and writes to a file.
pub struct FileStore {
    path: PathBuf,
}

impl FileStore {
    pub fn new(filename: impl ToString) -> Result<Self> {
        let folder = match my_home()? {
            Some(home) => format!("{}/{}", home.to_string_lossy(), PATH),
            _ => anyhow::bail!("Failed to read home directory"),
        };
        let path = format!("{}/{}.json", folder, filename.to_string());

        fs::create_dir_all(folder.clone())?;

        Ok(Self {
            path: PathBuf::from(path),
        })
    }
}

impl ReadState for FileStore {
    fn read(&self) -> Result<Vec<u8>> {
        let path = PathBuf::from(&self.path);

        Ok(fs::read(path)?)
    }
}

impl WriteState for FileStore {
    fn write(&self, contents: &[u8]) -> Result<()> {
        let path = PathBuf::from(&self.path);

        Ok(fs::write(path, contents)?)
    }
}

pub struct FileIdentifier {
    id: String,
}

impl FileIdentifier {
    pub fn new(command: &str, operation: &str, rid: &RepoId, id: Option<&ObjectId>) -> Self {
        let id = match id {
            Some(id) => format!("{}-{}-{}-{}", command, operation, rid, id),
            _ => format!("{}-{}-{}", command, operation, rid),
        };
        let id = format!("{:x}", md5::compute(id));

        Self { id }
    }
}

impl Display for FileIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id)
    }
}
