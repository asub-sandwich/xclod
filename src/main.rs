#![allow(warnings)]
use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use which::which;

use std::fs::create_dir;
use std::path::{Path, PathBuf};
use std::process::Command;

const DONE_MSG: &'static str = "\ndone!\n";

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// runs vid2jpg
    Vid2jpg {
        /// path to the input video
        input: PathBuf,
        /// path to the "site" directory (i.e. `KS1`)
        output: PathBuf,
        /// name of the sample (i.e. `KS1-H1-3`)
        sample_name: String,
        /// frames to extract per second of video
        #[arg(default_value_t = 3)]
        fps: u8,
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
        } => {
            vid2jpg(input, output, sample_name, fps);
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
    }

    Ok(())
}

fn vid2jpg(input_path: &Path, output_root: &Path, sample_name: &str, fps: &u8) -> Result<()> {
    if !input_path.exists() {
        bail!("{} doesn't exist!", input_path.display());
    }

    if !output_root.is_dir() {
        create_dir(output_root)?;
    }

    let output_dir = output_root.join(sample_name);

    if !output_dir.is_dir() {
        create_dir(&output_dir)?;
    }

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
    } else {
        let stderr = String::from_utf8_lossy(&ffmpeg.stderr);
        bail!("ffmpeg error:\n{}", stderr);
    }

    let arg_exif_output = format!("{}/frame_*.jpg", output_dir.display());

    let exif_args = [
        "-quiet",
        "-overwrite_original",
        "-ee",
        "-TagsFromFile",
        &arg_input,
        "-all:all",
        "-unsafe",
        "-icc_profile",
        &arg_exif_output,
    ];

    let exiftool = Command::new("exiftool")
        .args(exif_args)
        .output()
        .expect("failed to run exiftool!");

    if exiftool.status.success() {
        let stdout = String::from_utf8_lossy(&exiftool.stdout);
        if !stdout.is_empty() {
            println!("exiftool:\n{}", stdout);
        }
    } else {
        let stderr = String::from_utf8_lossy(&exiftool.stderr);
        bail!("exiftool error:\n{}", stderr);
    }

    println!(
        "\nframes written to {}, additional metadata copied via exiftool. done!\n",
        output_dir.display()
    );

    Ok(())
}
