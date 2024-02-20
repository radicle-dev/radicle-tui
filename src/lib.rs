use std::fmt::Display;

use anyhow::Result;

use serde::ser::{Serialize, SerializeStruct, Serializer};

use radicle::cob::ObjectId;
use radicle::node::notifications::NotificationId;

pub mod common;

#[cfg(feature = "realm")]
pub mod realm;

#[cfg(feature = "flux")]
pub mod flux;

/// An optional return value.
pub struct Exit<T> {
    pub value: Option<T>,
}

/// Returned ids can be of type `ObjectId` or `NotificationId`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Id {
    Object(ObjectId),
    Notification(NotificationId),
}

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Id::Object(id) => {
                write!(f, "{id}")
            }
            Id::Notification(id) => {
                write!(f, "{id}")
            }
        }
    }
}

/// The output that is returned by all selection interfaces.
#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct SelectionExit {
    operation: Option<String>,
    ids: Vec<Id>,
    args: Vec<String>,
}

impl SelectionExit {
    pub fn with_operation(mut self, operation: String) -> Self {
        self.operation = Some(operation);
        self
    }

    pub fn with_id(mut self, id: Id) -> Self {
        self.ids.push(id);
        self
    }

    pub fn with_args(mut self, arg: String) -> Self {
        self.args.push(arg);
        self
    }
}

impl Serialize for SelectionExit {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("", 3)?;
        state.serialize_field("operation", &self.operation)?;
        state.serialize_field(
            "ids",
            &self
                .ids
                .iter()
                .map(|id| format!("{}", id))
                .collect::<Vec<_>>(),
        )?;
        state.serialize_field("args", &self.args)?;
        state.end()
    }
}
