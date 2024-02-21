use anyhow::Result;

use serde::ser::{Serialize, SerializeStruct, Serializer};

pub mod common;

#[cfg(feature = "realm")]
pub mod realm;

#[cfg(feature = "flux")]
pub mod flux;

/// An optional return value.
#[derive(Clone, Debug)]
pub struct Exit<T> {
    pub value: Option<T>,
}

/// The output that is returned by all selection interfaces.
#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct Selection<I>
where
    I: ToString,
{
    pub operation: Option<String>,
    pub ids: Vec<I>,
    pub args: Vec<String>,
}

impl<I> Selection<I>
where
    I: ToString,
{
    pub fn with_operation(mut self, operation: String) -> Self {
        self.operation = Some(operation);
        self
    }

    pub fn with_id(mut self, id: I) -> Self {
        self.ids.push(id);
        self
    }

    pub fn with_args(mut self, arg: String) -> Self {
        self.args.push(arg);
        self
    }
}

impl<I> Serialize for Selection<I>
where
    I: ToString,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("", 3)?;
        state.serialize_field("operation", &self.operation)?;
        state.serialize_field(
            "ids",
            &self.ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
        )?;
        state.serialize_field("args", &self.args)?;
        state.end()
    }
}
