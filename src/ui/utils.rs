use std::collections::HashMap;

pub struct LineMerger;

impl LineMerger {
    pub fn merge<T: Clone>(
        lines: Vec<T>,
        mixins: HashMap<usize, Vec<Vec<T>>>,
        start: usize,
    ) -> Vec<T> {
        let mut merged = vec![];
        for (idx, line) in lines.iter().enumerate() {
            merged.push(line.clone());

            let actual_idx = idx.saturating_add(start);
            if let Some(mixins) = mixins.get(&actual_idx) {
                for mixin in mixins {
                    for mixin_line in mixin {
                        merged.push(mixin_line.clone());
                    }
                }
            }
        }

        merged
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use crate::ui::utils::LineMerger;

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
        let comment = comment.lines().into_iter().collect::<Vec<_>>();

        let merged = LineMerger::merge(
            diff.lines().into_iter().collect(),
            HashMap::from([(3_usize, vec![comment])]),
            1,
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
        let comment = comment.lines().into_iter().collect::<Vec<_>>();

        let merged = LineMerger::merge(
            diff.lines().into_iter().collect(),
            HashMap::from([(104_usize, vec![comment])]),
            100,
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
                actual.push_str(&format!("{}", line));
            } else {
                actual.push_str(&format!("{}\n", line));
            }
        }

        actual
    }
}
