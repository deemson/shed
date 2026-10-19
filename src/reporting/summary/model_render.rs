use super::model::*;
use colored::Colorize;

impl Report {
    pub fn render(self) -> String {
        self.manifests
            .into_iter()
            .flat_map(Manifest::lines)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Manifest {
    fn lines(self) -> Vec<String> {
        let mut lines = Vec::new();

        lines.push(self.name.as_str().blue().to_string());

        if let Some(path) = self.path {
            if let Some(shed) = self.shed {
                lines.push(format!("{shed} ↔ {path}").dimmed().to_string());
            } else {
                lines.push(path.dimmed().to_string());
            }
        }

        if let Some(error) = self.error {
            lines.push(error.red().to_string());
        }

        lines.extend(tree_prefix_groups_of_lines(
            self.manifests
                .into_iter()
                .map(Manifest::lines)
                .chain(self.items.into_iter().map(Item::lines))
                .collect(),
        ));

        lines
    }
}

impl Item {
    fn lines(self) -> Vec<String> {
        let mut lines = Vec::new();

        let path = self.path;
        if let Some(shed) = self.shed {
            lines.push(format!("{shed} ↔ {path}"));
        } else {
            lines.push(path);
        }

        if let Some(error) = self.error {
            lines.push(error.red().to_string());
        }

        lines.extend(tree_prefix_groups_of_lines(
            self.items.into_iter().map(Self::lines).collect(),
        ));

        lines
    }
}

fn tree_prefix_groups_of_lines(groups: Vec<Vec<String>>) -> Vec<String> {
    let mut prefixed_lines = Vec::new();
    let groups_count = groups.len();
    for (index, group) in groups.into_iter().enumerate() {
        let is_last_group = index + 1 == groups_count;
        let (first_line_prefix, other_lines_prefix) = if is_last_group {
            // ("└─", "  ")
            ("└╴", "  ")
        } else {
            // ("├─", "│ ")
            ("├╴", "│ ")
        };

        for (line_index, line) in group.into_iter().enumerate() {
            let prefix = if line_index == 0 {
                first_line_prefix
            } else {
                other_lines_prefix
            }
            .dimmed();
            prefixed_lines.push(format!("{prefix}{line}"));
        }
    }
    prefixed_lines
}
