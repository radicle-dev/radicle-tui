use std::str::FromStr;

use anyhow::Result;

use radicle::cob::Label;
use radicle::prelude::Did;

pub mod issue;
pub mod patch;

pub fn parse_labels(input: String) -> Result<Vec<Label>> {
    let mut labels = vec![];
    if !input.is_empty() {
        for name in input.split(',') {
            match Label::new(name.trim()) {
                Ok(label) => labels.push(label),
                Err(err) => return Err(anyhow::anyhow!(err).context("Can't parse labels.")),
            }
        }
    }

    Ok(labels)
}

pub fn parse_assignees(input: String) -> Result<Vec<Did>> {
    let mut assignees = vec![];
    if !input.is_empty() {
        for did in input.split(',') {
            match Did::from_str(&format!("did:key:{}", did)) {
                Ok(did) => assignees.push(did),
                Err(err) => return Err(anyhow::anyhow!(err).context("Can't parse assignees.")),
            }
        }
    }

    Ok(assignees)
}

pub mod state {
    use anyhow::Result;
    use radicle::cob::issue::{CloseReason, State};

    pub fn from_u16(input: u16) -> Result<State> {
        match input {
            0 => Ok(State::Open),
            1 => Ok(State::Closed {
                reason: CloseReason::Solved,
            }),
            2 => Ok(State::Closed {
                reason: CloseReason::Other,
            }),
            _ => Err(anyhow::anyhow!("Can't parse state.")),
        }
    }

    pub fn to_u16(state: &State) -> u16 {
        match state {
            State::Open => 0,
            State::Closed { reason } => match reason {
                CloseReason::Solved => 1,
                CloseReason::Other => 2,
            },
        }
    }
}
