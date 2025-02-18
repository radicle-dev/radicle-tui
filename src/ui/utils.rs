use std::collections::HashMap;

pub struct LineMerger;

#[derive(Default, Clone, Debug, Hash, Eq, PartialEq)]
pub enum MergeLocation {
    Start,
    Line(usize),
    End,
    #[default]
    Unknown,
}

impl LineMerger {
    pub fn merge<T: Clone>(
        lines: Vec<T>,
        mixins: HashMap<MergeLocation, Vec<Vec<T>>>,
        start: Option<usize>,
    ) -> Vec<T> {
        let mut merged = vec![];
        for (idx, line) in lines.iter().enumerate() {
            let location = if idx == 0 {
                MergeLocation::Start
            } else if idx == lines.len().saturating_sub(1) {
                MergeLocation::End
            } else {
                MergeLocation::Line(idx.saturating_add(start.unwrap_or_default()))
            };

            if location != MergeLocation::Start {
                merged.push(line.clone());
            }

            if let Some(mixins) = mixins.get(&location) {
                for mixin in mixins {
                    for mixin_line in mixin {
                        merged.push(mixin_line.clone());
                    }
                }
            }

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

        let merged = LineMerger::merge(
            diff.lines().collect(),
            HashMap::from([(MergeLocation::Line(3), vec![comment])]),
            Some(1),
        );
        let actual = build_string(merged);

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

        let merged = LineMerger::merge(
            diff.lines().collect(),
            HashMap::from([(MergeLocation::Line(104), vec![comment])]),
            Some(100),
        );
        let actual = build_string(merged);

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

    fn build_string(lines: Vec<&str>) -> String {
        let mut actual = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if idx == lines.len() - 1 {
                actual.push_str(line);
            } else {
                actual.push_str(&format!("{}\n", line));
            }
        }

        actual
    }
}
