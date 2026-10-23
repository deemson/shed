use super::model::*;
use crate::traversal;
use colored::Colorize;

struct Context {
    direction: Option<traversal::Direction>,
    status: ReportStatus,
    verbose: bool,
}

impl Report {
    pub fn render(self, verbose: bool) -> String {
        let context = Context {
            direction: self.direction,
            status: self.status,
            verbose,
        };
        self.manifests
            .into_iter()
            .flat_map(|manifest| manifest.lines(&context))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Manifest {
    fn lines(self, context: &Context) -> Vec<String> {
        let mut lines = Vec::new();

        lines.push(self.name.as_str().blue().to_string());

        if let Some(line) = self.path_line(context) {
            lines.push(line);
        }

        if let Some(error) = self.error {
            lines.push(error.red().to_string());
        }

        lines.extend(tree_prefix_groups_of_lines(
            self.manifests
                .into_iter()
                .map(|manifest| manifest.lines(context))
                .chain(self.items.into_iter().map(|item| item.lines(context)))
                .collect(),
        ));

        lines
    }

    fn path_line(&self, context: &Context) -> Option<String> {
        let path = self.path.as_ref()?;
        let direction = match context.direction {
            None => "↔",
            Some(traversal::Direction::Put) => "←",
            Some(traversal::Direction::Get) => "→",
        };
        Some(match &self.shed {
            Some(shed) => format!("{shed} {direction} {path}").dimmed().to_string(),
            None => path.dimmed().to_string(),
        })
    }
}

impl Item {
    fn is_dir(&self) -> bool {
        !self.items.is_empty() || self.entries.is_some()
    }

    fn lines(self, context: &Context) -> Vec<String> {
        let mut lines = Vec::new();

        lines.push(self.path_line(context));

        if let Some(error) = self.error {
            lines.push(error.red().to_string());
        }

        lines.extend(tree_prefix_groups_of_lines(
            self.items
                .into_iter()
                .map(|item| item.lines(context))
                .collect(),
        ));

        lines
    }

    fn path_line(&self, context: &Context) -> String {
        let path = &self.path;
        let direction = match context.direction {
            None => "↔",
            Some(traversal::Direction::Put) => "←",
            Some(traversal::Direction::Get) => "→",
        };
        let slash = if self.is_dir() { "/" } else { "" };
        match &self.shed {
            Some(shed) => format!("{shed}{slash} {direction} {path}{slash}").to_string(),
            None => format!("{path}{slash}"),
        }
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
