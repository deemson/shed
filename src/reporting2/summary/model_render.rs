use super::model::*;

impl Report {
    pub fn render(&self) -> String {
        self.manifests
            .iter()
            .flat_map(Manifest::lines)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Manifest {
    pub fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(self.name.clone());

        for (index, manifest) in self.manifests.iter().enumerate() {
            let is_last = index + 1 == self.manifests.len();

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

        lines
    }
}
