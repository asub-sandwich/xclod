#![allow(warnings)]
use anyhow::{Result, anyhow, bail};
use clap::{Parser, Subcommand};
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::{FfmpegEvent, LogLevel};
use indicatif::ProgressBar;
use serde::Deserialize;

use std::ffi::OsStr;
use std::fs::{create_dir_all, read_dir};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

// === bundled helper binaries === //

const ASSIMP_BIN: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/assimp"));
const EXIFTOOL_BIN: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/exiftool"));

// ============ config =========== //

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Config {
    /// where bundled binaries are extracted to
    cache_dir: Option<PathBuf>,
    extract: ExtractCfg,
    convert: ConvertCfg,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ExtractCfg {
    input_dir: PathBuf,
    output_dir: PathBuf,
    fps: u8,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ConvertCfg {
    input_dir: PathBuf,
    output_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cache_dir: None,
            extract: ExtractCfg::default(),
            convert: ConvertCfg::default(),
        }
    }
}

impl Default for ExtractCfg {
    fn default() -> Self {
        Self {
            input_dir: home_dir().join("xclod/vid"),
            output_dir: home_dir().join("xclod/jpg"),
            fps: 3,
        }
    }
}

impl Default for ConvertCfg {
    fn default() -> Self {
        Self {
            input_dir: home_dir().join("xclod/fbx"),
            output_dir: home_dir().join("xclod/obj"),
        }
    }
}

fn home_dir() -> PathBuf {
    std::env::home_dir()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn config_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config"))
        .join("xclod/config.toml")
}

fn load_config() -> Result<Config> {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).map_err(|e| anyhow!("error in {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            write_default_config(&path)?;
            eprintln!("no config found! wrote basic config to {}", path.display());
            eprintln!("edit config file to set options (or override with command flags!)\n");
            Ok(Config::default())
        }
        Err(e) => Err(anyhow!("couldn't read {}: {e}", path.display())),
    }
}

fn write_default_config(path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        create_dir_all(dir)?;
    }
    let d = Config::default();
    let template = format!(
        "# xclod configuration file\n\
         # Values here replace the built-in defaults. Command-line flags\n\
         # (--input-dir, --output-dir, --fps) override values set here.\n\
         \n\
         # Where the bundled assimp/exiftool are extracted.\n\
         # Defaults to $XDG_CACHE_HOME/xclod/bin or ~/.cache/xclod/bin.\n\
         # cache_dir = \"/path/to/cache\"\n\
         \n\
         [vid2jpg]\n\
         input_dir  = \"{vi}\"   # folder holding the source videos\n\
         output_dir = \"{vo}\"   # root folder for extracted JPG frames\n\
         fps        = {fps}                          # frames extracted per second\n\
         \n\
         [fbx2obj]\n\
         input_dir  = \"{fi}\"      # folder holding the source FBX files\n\
         output_dir = \"{fo}\"      # folder for the converted OBJ files\n",
        vi = d.extract.input_dir.display(),
        vo = d.extract.output_dir.display(),
        fps = d.extract.fps,
        fi = d.convert.input_dir.display(),
        fo = d.convert.output_dir.display(),
    );
    std::fs::write(path, template)?;
    Ok(())
}

fn resolve_cache_dir(config: &Config) -> PathBuf {
    config.cache_dir.clone().unwrap_or_else(|| {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".cache"))
            .join("xclod/bin")
    })
}

// ============= cli ============= //

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
        /// frames to extract per second of video (overrides config)
        #[arg(short, long)]
        fps: Option<u8>,
        /// directory containing the input video file (overrides config)
        #[arg(short, long)]
        input_dir: Option<PathBuf>,
        /// directory containting the output root directory (overrides config)
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
    },
    /// convert Autodesk FBX files to standard OBJ files
    Convert {
        /// an FBX file, or a directory of FBX files, to convert to OBJ files
        input: PathBuf,
        /// directory containing the input FBX file(s) (overrides config)
        #[arg(short, long)]
        input_dir: Option<PathBuf>,
        /// directory for the output OBJ file(s) (overrides config)
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
    },
}

// =========== program =========== //

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = load_config()?;
    let cache_dir = resolve_cache_dir(&config);

    match &cli.command {
        Commands::Extract {
            input,
            output,
            sample_name,
            fps,
            input_dir,
            output_dir,
        } => {
            let input_dir = input_dir.as_ref().unwrap_or(&config.extract.input_dir);
            let output_dir = output_dir.as_ref().unwrap_or(&config.extract.output_dir);
            let fps = fps.unwrap_or(config.extract.fps);
            let exiftool = vendored_binary(&cache_dir, "exiftool", EXIFTOOL_BIN)?;
            let input = input_dir.join(input);
            let output = output_dir.join(output);
            extract(&input, &output, sample_name, fps, &exiftool)?;
        }
        Commands::Convert {
            input,
            input_dir,
            output_dir,
        } => {
            let input_dir = input_dir.as_ref().unwrap_or(&config.convert.input_dir);
            let output_dir = output_dir.as_ref().unwrap_or(&config.convert.output_dir);
            let assimp = vendored_binary(&cache_dir, "assimp", ASSIMP_BIN)?;
            let input = input_dir.join(input);
            convert(&input, &output_dir, &assimp)?;
        }
    }
    Ok(())
}

/// writes or loads bundled binary into cache
fn vendored_binary(cache_dir: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf> {
    create_dir_all(cache_dir)?;
    let path = cache_dir.join(name);
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

/// uses ffmpeg to extract frames as jpg from a video
fn extract(
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

    bar.finish_with_message("metadata extraction complete!");
    println!("\n\nframes written to {}.\n\ndone!\n", output_dir.display());

    Ok(())
}

/// uses assimp to convert an Autodesk FBX file to an OBJ file
fn convert(input: &Path, output_dir: &Path, assimp: &Path) -> Result<()> {
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
        let assimp = assimp.to_path_buf();
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

    println!(
        "\n\nOBJ file(s) written to {}\n\ndone!\n",
        output_dir.display()
    );

    bar.finish_with_message("metadata extraction complete!");
    Ok(())
}
