use anyhow::{Result, bail};
use indicatif::ProgressBar;
use rayon::prelude::*;

use std::fs::{create_dir_all, read_dir};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// uses assimp to convert an Autodesk FBX file to an OBJ file
pub fn convert(input: &Path, output_dir: &Path, assimp: &Path) -> Result<()> {
    let fbx_files: Vec<PathBuf> = if input.is_dir() {
        let mut files: Vec<PathBuf> = read_dir(input)?
            .filter_map(|e| {
                let path = e.expect("failure parsing input dir paths").path();
                match path.extension() {
                    Some(ext) if ext.eq_ignore_ascii_case("fbx") => Some(path),
                    _ => None,
                }
            })
            .collect();
        files.sort();
        if files.is_empty() {
            bail!("no .fbx files found in {}", input.display());
        }
        files
    } else if input.is_file() {
        vec![input.to_path_buf()]
    } else {
        bail!("{} doesn't exist", input.display());
    };

    create_dir_all(output_dir)?;

    let total = fbx_files.len();
    println!("\nconverting {} FBX file(s) to OBJ with assimp...", total);

    let bar = ProgressBar::new(total.try_into()?);
    let failures = Arc::new(AtomicUsize::new(0));

    fbx_files.into_par_iter().for_each(|fbx| {
        let stem = fbx
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("couldn't read filename of an FBX file.");

        let obj = output_dir.join(format!("{stem}.obj"));
        let in_arg = fbx.display().to_string();
        let out_arg = obj.display().to_string();
        let assimp_run = Command::new(&assimp)
            .args(["export", &in_arg, &out_arg])
            .output()
            .expect("failed to run assimp!");
        if !assimp_run.status.success() {
            let stderr = String::from_utf8_lossy(&assimp_run.stderr);
            failures.fetch_add(1, Ordering::SeqCst);
            eprintln!("  assimp error on {}:\n{}", fbx.display(), stderr);
        }
        bar.inc(1);
    });

    bar.finish_and_clear();

    let failures = failures.load(Ordering::SeqCst);
    if failures > 0 {
        eprintln!("{} of {} conversion(s) failed!", failures, total);
    }

    println!("OBJ file(s) written to {}\ndone!\n", output_dir.display());

    Ok(())
}
