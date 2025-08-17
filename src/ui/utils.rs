use std::collections::HashMap;

/// A type that specifies the location of a line merge.
/// The lines to merge will be either added before, within or after
/// the base lines.
#[derive(Default, Clone, Debug, Hash, Eq, PartialEq)]
pub enum MergeLocation {
    Start,
    Line(usize),
    End,
    #[default]
    Unknown,
}

/// A type that can merge lines based on their merge location.
#[derive(Default)]
pub struct LineMerger<T> {
    /// Base lines that other lines will be merged with.
    lines: Vec<T>,
}

impl<T: Clone> LineMerger<T> {
    pub fn new(lines: impl IntoIterator<Item = T>) -> Self {
        Self {
            lines: lines.into_iter().collect::<Vec<_>>(),
        }
    }

    pub fn merge(
        &self,
        merge: HashMap<MergeLocation, Vec<Vec<T>>>,
        start: Option<usize>,
    ) -> Vec<T> {
        let mut merged = vec![];
        for (idx, line) in self.lines.iter().enumerate() {
            let location = if idx == 0 {
                MergeLocation::Start
            } else if idx == self.lines.len().saturating_sub(1) {
                MergeLocation::End
            } else {
                let idx = idx
                    .saturating_add(start.unwrap_or_default())
                    .saturating_sub(1);
                MergeLocation::Line(idx)
            };

            if location != MergeLocation::Start {
                merged.push(line.clone());
            }

            merged.extend(
                merge
                    .get(&location)
                    .into_iter()
                    .flatten()
                    .flat_map(|merge| merge.iter())
                    .cloned(),
            );

            if location == MergeLocation::Start {
                merged.push(line.clone());
            }
        }

        merged
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use crate::ui::utils::{LineMerger, MergeLocation};

    #[test]
    fn lines_should_be_merged_correctly() -> anyhow::Result<()> {
        let diff = r#"
fn main() {
    println!("Hello, world!");

    another_function();
}

fn another_function() {
    println!("Another function.");
}"#;

        let comment = r#"──────────────────────────────────────
Is this needed?
──────────────────────────────────────"#
            .to_string();
        let comment = comment.lines().collect::<Vec<_>>();

        let merged = LineMerger::new(diff.lines()).merge(
            HashMap::from([(MergeLocation::Line(2), vec![comment])]),
            Some(1),
        );
        let actual = merged.join("\n");

        let expected = r#"
fn main() {
    println!("Hello, world!");
──────────────────────────────────────
Is this needed?
──────────────────────────────────────

    another_function();
}

fn another_function() {
    println!("Another function.");
}"#;

        let expected = expected.to_string();

        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn lines_with_start_should_be_merged_correctly() -> anyhow::Result<()> {
        let diff = r#"
fn main() {
    println!("Hello, world!");

    another_function();
}

fn another_function() {
    println!("Another function.");
}"#;

        let comment = r#"──────────────────────────────────────
Is this needed?
──────────────────────────────────────"#
            .to_string();
        let comment = comment.lines().collect::<Vec<_>>();

        let merged = LineMerger::new(diff.lines()).merge(
            HashMap::from([(MergeLocation::Line(103), vec![comment])]),
            Some(100),
        );
        let actual = merged.join("\n");

        let expected = r#"
fn main() {
    println!("Hello, world!");

    another_function();
──────────────────────────────────────
Is this needed?
──────────────────────────────────────
}

fn another_function() {
    println!("Another function.");
}"#;

        let expected = expected.to_string();

        assert_eq!(expected, actual);

        Ok(())
    }
}
