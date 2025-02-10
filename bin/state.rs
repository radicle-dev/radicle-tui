use std::path::PathBuf;
use std::{fmt::Display, fs};

use anyhow::Result;

use serde::{Deserialize, Serialize};

use homedir::my_home;

const PATH: &str = ".radicle-tui/states";

/// Trait for state readers.
pub trait ReadState {
    fn read(&self) -> Result<Vec<u8>>;
}

/// Trait for state writers.
pub trait WriteState: ReadState {
    fn write(&self, bytes: &[u8]) -> Result<()>;
}

/// Trait for types that be decoded from bytes.
pub trait Decode<'a, T> {
    fn from_json(bytes: &'a [u8]) -> Result<T>;
}

/// Trait for types that be encoded into a bytes.
pub trait Encode<T> {
    fn to_json(self) -> Result<Vec<u8>>;
}

/// Blanket implementation that decodes deserializable types.
impl<'a, T> Decode<'a, T> for T
where
    T: Deserialize<'a>,
{
    fn from_json(bytes: &'a [u8]) -> Result<T> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

/// Blanket implementation that encodes serializable types.
impl<T> Encode<T> for T
where
    T: Serialize,
{
    fn to_json(self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&self)?)
    }
}

/// A state storage that reads from and writes to a file.
pub struct FileStorage {
    path: PathBuf,
}

impl FileStorage {
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

impl ReadState for FileStorage {
    fn read(&self) -> Result<Vec<u8>> {
        let path = PathBuf::from(&self.path);

        Ok(fs::read(path)?)
    }
}

impl WriteState for FileStorage {
    fn write(&self, contents: &[u8]) -> Result<()> {
        let path = PathBuf::from(&self.path);

        Ok(fs::write(path, contents)?)
    }
}

pub struct Filename {
    name: String,
}

impl Filename {
    pub fn new(command: &str, operation: &str, id: &str) -> Self {
        Self {
            name: format!("{}-{}-{}", command, operation, id),
        }
    }
}

impl Display for Filename {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
