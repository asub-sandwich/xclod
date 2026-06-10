#![allow(warnings)]
use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use indicatif::ProgressBar;
use which::which;

use std::ffi::OsStr;
use std::fs::{create_dir, create_dir_all, read_dir};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

const DONE_MSG: &'static str = "\ndone!\n";

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// converts a video to several JPGs
    Vid2jpg {
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    check_dependencies()?;

    match &cli.command {
        Commands::Vid2jpg {
            input,
            output,
            sample_name,
            fps,
            input_dir,
            output_dir,
        } => {
            let input = input_dir.join(&input);
            let output = output_dir.join(&output);
            vid2jpg(&input, &output, sample_name, fps)?;
        }
    }
    Ok(())
}

fn check_dependencies() -> Result<()> {
    let required = ["ffmpeg", "assimp", "magick", "exiftool", "parallel"];

    let mut missing: Vec<String> = vec![];

    for tool in required {
        let installed = which(tool);
        if installed.is_err() {
            missing.push(tool.to_string());
        }
    }

    if !missing.is_empty() {
        println!("\ninstall the following dependencies pls:");
        for tool in missing {
            println!("{}", tool);
        }

        bail!("unable to continue without dependencies!");
    }

    Ok(())
}

fn vid2jpg(input_path: &Path, output_root: &Path, sample_name: &str, fps: &u8) -> Result<()> {
    if !input_path.exists() {
        bail!("{} doesn't exist!", input_path.display());
    }

    let output_dir = output_root.join(sample_name);
    create_dir_all(&output_dir)?;

    println!("\nextracting frames with ffmpeg...");

    let arg_input = format!("{}", input_path.display());
    let arg_fps = format!("fps={}", fps);
    let arg_output = format!("{}/frame_%03d.jpg", output_dir.display());

    let ffmpeg_args = [
        "-i",
        &arg_input,
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
        &arg_output,
    ];

    let ffmpeg = Command::new("ffmpeg")
        .args(ffmpeg_args)
        .output()
        .expect("failed to run ffmpeg!");

    if ffmpeg.status.success() {
        let stdout = String::from_utf8_lossy(&ffmpeg.stdout);
        if !stdout.is_empty() {
            println!("ffmpeg:\n{}", stdout);
        }
        println!("ffmpeg done!\n");
    } else {
        let stderr = String::from_utf8_lossy(&ffmpeg.stderr);
        bail!("ffmpeg error:\n{}", stderr);
    }

    let jpg_paths: Vec<PathBuf> = read_dir(&output_dir)?
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
        jpg_paths.len()
    );

    let num_jobs = jpg_paths.len();

    let bar = ProgressBar::new(num_jobs.try_into()?);

    let mut handles = vec![];

    for jpg_path in jpg_paths {
        let arg_input = arg_input.clone();
        let bar_clone = bar.clone();
        let handle = thread::spawn(move || {
            let jpg = jpg_path.to_str().expect("couldn't unwrap jpg_path");
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
            bar_clone.inc(1);
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
