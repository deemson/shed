use std::fs;
use std::io;
use std::path::Path;
use std::process::ExitCode;

use walkdir::WalkDir;

use crate::item::ResolvedItem;

#[derive(Clone, Copy)]
pub enum Direction {
    Put, // system -> shed
    Get, // shed -> system
}

pub struct Outcome {
    pub warnings: usize,
    pub errors: usize,
}

impl Outcome {
    pub fn exit_code(&self) -> ExitCode {
        if self.errors > 0 {
            ExitCode::from(2)
        } else if self.warnings > 0 {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        }
    }
}

pub fn execute(
    items: &[ResolvedItem],
    direction: Direction,
    dry: bool,
    verbose: bool,
) -> Result<Outcome, crate::error::Error> {
    let mut warnings = 0;
    let mut errors = 0;
    let show_output = dry || verbose;

    for item in items {
        if show_output {
            println!("syncing: {}", item.name);
        }

        for entry in &item.entries {
            let (src, dst) = match direction {
                Direction::Put => (&entry.system, &entry.shed),
                Direction::Get => (&entry.shed, &entry.system),
            };

            if !src.exists() {
                warnings += 1;
                eprintln!("  warning: source not found: {:?}", src);
                continue;
            }

            match sync_path(src, dst, dry, show_output) {
                Ok(()) => {}
                Err(e) => {
                    errors += 1;
                    eprintln!("  error: {:?} -> {:?}: {}", src, dst, e);
                }
            }
        }
    }

    Ok(Outcome { warnings, errors })
}

fn sync_path(src: &Path, dst: &Path, dry: bool, verbose: bool) -> io::Result<()> {
    if src.is_dir() {
        sync_dir(src, dst, dry, verbose)
    } else {
        sync_file(src, dst, dry, verbose)
    }
}

fn sync_file(src: &Path, dst: &Path, dry: bool, verbose: bool) -> io::Result<()> {
    if verbose {
        println!("  copy: {:?} -> {:?}", src, dst);
    }

    if !dry {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src, dst)?;
    }

    Ok(())
}

fn sync_dir(src: &Path, dst: &Path, dry: bool, verbose: bool) -> io::Result<()> {
    for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let src_path = entry.path();
        let relative = src_path.strip_prefix(src).unwrap();
        let dst_path = dst.join(relative);

        if src_path.is_dir() {
            if !dry {
                fs::create_dir_all(&dst_path)?;
            }
        } else {
            if verbose {
                println!("  copy: {:?} -> {:?}", src_path, dst_path);
            }
            if !dry {
                if let Some(parent) = dst_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(src_path, &dst_path)?;
            }
        }
    }

    Ok(())
}
