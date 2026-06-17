use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
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
    /// update persistent settings in the config file
    ///
    /// Only the settings you pass are changed; the rest are left as-is.
    /// Pass at least one flag.
    Set {
        /// folder holding the source videos (extract input_dir)
        #[arg(long)]
        extract_input_dir: Option<PathBuf>,
        /// root folder for extracted JPG frames (extract output_dir)
        #[arg(long)]
        extract_output_dir: Option<PathBuf>,
        /// frames extracted per second (extract fps)
        #[arg(long)]
        fps: Option<u8>,
        /// folder holding the source FBX files (convert input_dir)
        #[arg(long)]
        convert_input_dir: Option<PathBuf>,
        /// folder for the converted OBJ files (convert output_dir)
        #[arg(long)]
        convert_output_dir: Option<PathBuf>,
        /// directory where bundled binaries are extracted
        #[arg(long)]
        cache_dir: Option<PathBuf>,
    },
}
