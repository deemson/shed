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

        lines.push(self.name.as_str().bold().to_string());
        if let Some(error) = self.error {
            lines.push(error.red().to_string());
        }

        let manifest_count = self.manifests.len();
        for (index, manifest) in self.manifests.into_iter().enumerate() {
            let is_last = index + 1 == manifest_count;

            for (line_index, line) in manifest.lines().into_iter().enumerate() {
                let prefix = match (line_index == 0, is_last) {
                    (true, false) => "├─",
                    (true, true) => "└─",
                    (false, false) => "│ ",
                    (false, true) => "  ",
                };
                lines.push(format!("{prefix}{line}"));
            }
        }

        for item in self.items.into_iter() {
            lines.extend(item.lines());
        }

        lines
    }
}

impl Item {
    fn lines(self) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(error) = self.error {
            lines.push(error.red().to_string());
        }
        for item in self.items.into_iter() {
            lines.extend(item.lines());
        }
        lines
    }
}
