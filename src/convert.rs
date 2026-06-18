use anyhow::{Result, anyhow, bail};
use indicatif::ProgressBar;

use std::fs::{create_dir_all, read_dir};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

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
    println!("\nconverting {} FBX file(s) to OBJ with assimp...\n", total);

    let bar = ProgressBar::new(total.try_into()?);
    let failures = Arc::new(AtomicUsize::new(0));

    let mut handles = vec![];
    for fbx in fbx_files.iter() {
        let stem = fbx
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("couldn't read filename of {}", fbx.display()))?;

        let obj = output_dir.join(format!("{stem}.obj"));
        let in_arg = fbx.display().to_string();
        let out_arg = obj.display().to_string();

        let fbx = fbx.clone();
        let assimp = assimp.to_path_buf();
        let bar = bar.clone();
        let failures_clone = Arc::clone(&failures);

        let handle = thread::spawn(move || {
            let assimp_run = Command::new(&assimp)
                .args(["export", &in_arg, &out_arg])
                .output()
                .expect("failed to run assimp!");
            if !assimp_run.status.success() {
                let stderr = String::from_utf8_lossy(&assimp_run.stderr);
                failures_clone.fetch_add(1, Ordering::SeqCst);
                eprintln!("  assimp error on {}:\n{}", fbx.display(), stderr);
            }
            bar.inc(1);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("an exiftool thread panicked!")
    }

    bar.finish_and_clear();

    let failures = failures.load(Ordering::SeqCst);
    if failures > 0 {
        eprintln!("{} of {} conversion(s) failed!", failures, total);
    }

    println!(
        "\n\nOBJ file(s) written to {}\n\ndone!\n",
        output_dir.display()
    );

    Ok(())
}
