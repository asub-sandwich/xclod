#![allow(warnings)]
use anyhow::{Result, anyhow, bail};
use clap::{Parser, Subcommand};
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::{FfmpegEvent, LogLevel};
use indicatif::ProgressBar;

use std::ffi::OsStr;
use std::fs::{create_dir_all, read_dir};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

const ASSIMP_BIN: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/assimp"));
// const EXIFTOOL_BIN: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/exiftool"));

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// extract frames of a video as jpegs
    Extract {
        /// filename of the video
        input: PathBuf,
        /// name of the site folder (i.e. `KS1`)
        output: PathBuf,
        /// name of the sample (i.e. `KS1-H1-3`)
        sample_name: String,
        /// frames to extract per second of video
        #[arg(short, long, default_value_t = 3)]
        fps: u8,
        /// directory containing the input video file (shouldn't have to change)
        #[arg(short, long, default_value = OsStr::new("/mnt/c/Users/65610791/Pictures/BulkDensity_2026"))]
        input_dir: PathBuf,
        /// directory containting the output root directory (shouldn't have to change)
        #[arg(short, long, default_value = OsStr::new("/home/ada/bulkdensity/jpgs"))]
        output_dir: PathBuf,
    },
    /// convert Autodesk FBX files to standard OBJ files
    Convert {
        /// an FBX file, or a directory of FBX files, to convert to OBJ files
        input: PathBuf,
        /// directory containing the input FBX file(s) (shouldn't have to change)
        #[arg(short, long, default_value = OsStr::new("/home/ada/bulkdensity/fbx"))]
        input_dir: PathBuf,
        /// directory for the output OBJ file(s) (shouldn't have to change)
        #[arg(short, long, default_value = OsStr::new("/home/ada/bulkdensity/objs"))]
        output_dir: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Extract {
            input,
            output,
            sample_name,
            fps,
            input_dir,
            output_dir,
        } => {
            let input = input_dir.join(&input);
            let output = output_dir.join(&output);
            extract(&input, &output, sample_name, fps)?;
        }
        Commands::Convert { .. } => (),
    }
    Ok(())
}

fn vendored_binary(name: &str, bytes: &[u8]) -> Result<PathBuf> {
    let cache_root = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .ok_or_else(|| anyhow!("couldn't find a cache dir (set HOME or XDG_CACHE_HOME)"))?;
    let dir = cache_root.join("xclod").join("bin");
    create_dir_all(&dir)?;
    let path = dir.join(name);

    let needs_write = match std::fs::metadata(&path) {
        Ok(meta) => meta.len() != bytes.len() as u64,
        Err(_) => true,
    };

    if needs_write {
        std::fs::write(&path, bytes)?;
    }

    let mut perms = std::fs::metadata(&path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms)?;

    Ok(path)
}

fn extract(input_path: &Path, output_root: &Path, sample_name: &str, fps: &u8) -> Result<()> {
    if !input_path.exists() {
        bail!("{} doesn't exist!", input_path.display());
    }

    let output_dir = output_root.join(sample_name);
    create_dir_all(&output_dir)?;

    println!("\nchecking ffmpeg...");
    ffmpeg_sidecar::download::auto_download().map_err(|e| anyhow!("couldn't set up ffmpeg: {e}"));

    println!("\nextracting frames with ffmpeg...");

    let arg_input = format!("{}", input_path.display());
    let arg_fps = format!("fps={}", fps);
    let arg_output = format!("{}/frame_%03d.jpg", output_dir.display());

    let mut child = FfmpegCommand::new()
        .arg("-y")
        .input(&arg_input)
        .args([
            "-map_metadata",
            "0",
            "-vf",
            &arg_fps,
            "-vsync",
            "0",
            "-start_number",
            "1",
            "-q:v",
            "1",
            "-color_range",
            "mpeg",
            "-colorspace",
            "bt709",
            "-color_primaries",
            "bt709",
            "-color_trc",
            "bt709",
            "-f",
            "image2",
        ])
        .output(&arg_output)
        .spawn()
        .map_err(|e| anyhow!("failed to start ffmpeg: {e}"))?;

    let mut ffmpeg_errors: Vec<String> = vec![];
    for event in child
        .iter()
        .map_err(|e| anyhow!("ffmpeg stream error: {e}"))?
    {
        match event {
            FfmpegEvent::Log(LogLevel::Error | LogLevel::Fatal, msg) => ffmpeg_errors.push(msg),
            FfmpegEvent::Error(msg) => ffmpeg_errors.push(msg),
            _ => {}
        }
    }

    let status = child
        .wait()
        .map_err(|e| anyhow!("ffmpeg wait failed: {e}"))?;
    if !status.success() {
        bail!("ffmpeg error:\n{}", ffmpeg_errors.join("\n"));
    }
    println!("ffmpeg done!\n");

    let jpg_files: Vec<PathBuf> = read_dir(&output_dir)?
        .filter_map(|e| {
            let path = e.expect("failure parsing output dir paths").path();
            match path.extension() {
                Some(ext) if ext.eq_ignore_ascii_case("jpg") => Some(path),
                _ => None,
            }
        })
        .collect();

    println!(
        "copying metadata with exiftool ({} frames)...",
        jpg_files.len()
    );

    let num_jobs = jpg_files.len();

    let bar = ProgressBar::new(num_jobs.try_into()?);

    let mut handles = vec![];

    for jpg in jpg_files {
        let arg_input = arg_input.clone();
        let bar = bar.clone();

        let handle = thread::spawn(move || {
            let jpg = jpg.to_str().expect("couldn't unwrap jpg_path");
            let exif_args = [
                "-quiet",
                "-overwrite_original",
                "-ee",
                "-TagsFromFile",
                arg_input.as_str(),
                "-all:all",
                "-unsafe",
                "-icc_profile",
                jpg,
            ];
            let output = Command::new("exiftool")
                .args(exif_args)
                .output()
                .expect("failed to run exiftool");
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("exiftool error on {}: {}", jpg, stderr);
            }
            bar.inc(1);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("an exiftool thread panicked!")
    }

    bar.finish_with_message("metadata extraction complete!");
    println!("\n\nframes written to {}.\n\ndone!\n", output_dir.display());

    Ok(())
}

fn convert(input: &Path, output_dir: &Path) -> Result<()> {
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

    let assimp = vendored_binary("assimp", ASSIMP_BIN)?;

    let total = fbx_files.len();
    println!("\nconverting {} FBX file(s) to OBJ with assimp...\n", total);

    let bar = ProgressBar::new(total.try_into()?);

    let mut handles = vec![];
    let mut failures = 0u32;

    for fbx in fbx_files.iter() {
        let stem = fbx
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("couldn't read filename of {}", fbx.display()))?;

        let obj = output_dir.join(format!("{stem}.obj"));
        let in_arg = fbx.display().to_string();
        let out_arg = obj.display().to_string();

        let fbx = fbx.clone();
        let assimp = assimp.clone();
        let bar = bar.clone();

        let handle = thread::spawn(move || {
            let assimp_run = Command::new(&assimp)
                .args(["export", &in_arg, &out_arg])
                .output()
                .expect("failed to run assimp!");
            if !assimp_run.status.success() {
                failures += 1;
                let stderr = String::from_utf8_lossy(&assimp_run.stderr);
                eprintln!("  assimp error on {}:\n{}", fbx.display(), stderr);
            }
            bar.inc(1);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("an exiftool thread panicked!")
    }

    if failures > 0 {
        eprintln!("{} of {} conversion(s) failed!", failures, total);
    }

    println!("\ndone! OBJ file(s) written to {}", output_dir.display());

    bar.finish_with_message("metadata extraction complete!");
    Ok(())
}
