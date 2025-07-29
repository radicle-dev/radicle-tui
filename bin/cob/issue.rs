use std::fmt;
use std::fmt::Write as _;

use anyhow::Result;

use radicle::cob::issue::{Issue, IssueId};
use radicle::issue::cache::Issues;
use radicle::issue::State;
use radicle::prelude::Did;
use radicle::storage::git::Repository;
use radicle::Profile;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Filter {
    state: Option<State>,
    assigned: bool,
    assignees: Vec<Did>,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            state: Some(State::default()),
            assigned: false,
            assignees: vec![],
        }
    }
}

impl Filter {
    pub fn with_state(mut self, state: Option<State>) -> Self {
        self.state = state;
        self
    }

    pub fn with_assgined(mut self, assigned: bool) -> Self {
        self.assigned = assigned;
        self
    }

    pub fn with_assginee(mut self, assignee: Did) -> Self {
        self.assignees.push(assignee);
        self
    }
}

impl fmt::Display for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(state) = &self.state {
            write!(f, "is:{}", state)?;
            f.write_char(' ')?;
        }
        if self.assigned {
            f.write_str("is:assigned")?;
            f.write_char(' ')?;
        }
        if !self.assignees.is_empty() {
            f.write_str("assignees:")?;
            f.write_char('[')?;

            let mut assignees = self.assignees.iter().peekable();
            while let Some(assignee) = assignees.next() {
                f.write_str(&assignee.encode())?;

                if assignees.peek().is_some() {
                    f.write_char(',')?;
                }
            }
            f.write_char(']')?;
        }

        Ok(())
    }
}

pub fn all(profile: &Profile, repository: &Repository) -> Result<Vec<(IssueId, Issue)>> {
    let cache = profile.issues(repository)?;
    let issues = cache.list()?;

    Ok(issues.flatten().collect())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use anyhow::Result;
    use radicle::issue;

    use super::*;

    #[test]
    fn issue_filter_display_with_state_should_succeed() -> Result<()> {
        let actual = Filter::default().with_state(Some(issue::State::Open));

        assert_eq!(String::from("is:open "), actual.to_string());

        Ok(())
    }

    #[test]
    fn issue_filter_display_with_state_and_assigned_should_succeed() -> Result<()> {
        let actual = Filter::default()
            .with_state(Some(issue::State::Open))
            .with_assgined(true);

        assert_eq!(String::from("is:open is:assigned "), actual.to_string());

        Ok(())
    }

    #[test]
    fn issue_filter_display_with_status_and_author_should_succeed() -> Result<()> {
        let actual = Filter::default()
            .with_state(Some(issue::State::Open))
            .with_assginee(Did::from_str(
                "did:key:z6MkswQE8gwZw924amKatxnNCXA55BMupMmRg7LvJuim2C1V",
            )?);

        assert_eq!(
            String::from(
                "is:open assignees:[did:key:z6MkswQE8gwZw924amKatxnNCXA55BMupMmRg7LvJuim2C1V]"
            ),
            actual.to_string()
        );

        Ok(())
    }
}
