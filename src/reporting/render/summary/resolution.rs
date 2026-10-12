use crate::{manifest, reporting::summary::resolution};
use colored::Colorize;

pub fn render(manifests: &[manifest::Manifest], report: &resolution::Report) -> String {
    "asd".into()
}

fn render_manifest_lines(
    manifest: &manifest::Manifest,
    report: &resolution::Manifest,
) -> Vec<String> {
    let mut lines = Vec::new();
    match &report.error {
        Some(error) => (),
        None => (),
    }
    lines.push(manifest.name.clone());
    lines
}
