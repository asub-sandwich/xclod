use anyhow::{Result, anyhow, bail};
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::{FfmpegEvent, LogLevel};
use indicatif::ProgressBar;

use std::fs::{create_dir_all, read_dir};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

/// uses ffmpeg to extract frames as jpg from a video
pub fn extract(
    input_path: &Path,
    output_root: &Path,
    sample_name: &str,
    fps: u8,
    exiftool: &Path,
) -> Result<()> {
    if !input_path.exists() {
        bail!("{} doesn't exist!", input_path.display());
    }

    let output_dir = output_root.join(sample_name);
    create_dir_all(&output_dir)?;

    println!("\nchecking ffmpeg...");
    ffmpeg_sidecar::download::auto_download()
        .map_err(|e| anyhow!("couldn't set up ffmpeg: {e}"))?;

    println!("extracting frames with ffmpeg...");

    let arg_input = format!("{}", input_path.display());
    let arg_fps = format!("fps={}", fps);
    let arg_output = output_dir.join("frame_%03d.jpg").display().to_string();

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
        let exiftool = exiftool.to_path_buf();
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
            let output = Command::new(&exiftool)
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

    bar.finish_and_clear();

    println!("frames written to {}.\ndone!", output_dir.display());

    Ok(())
}
