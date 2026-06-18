mod cli;
mod config;
mod convert;
mod extract;
mod tools;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Commands};

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
            let config = config::load_config()?;
            let cache_dir = config::resolve_cache_dir(&config);
            let input_dir = input_dir.as_ref().unwrap_or(&config.extract.input_dir);
            let output_dir = output_dir.as_ref().unwrap_or(&config.extract.output_dir);
            let fps = fps.unwrap_or(config.extract.fps);
            let exiftool = tools::resolve_exiftool(&cache_dir);
            let input = input_dir.join(input);
            let output = output_dir.join(output);
            extract::extract(&input, &output, sample_name, fps, exiftool)?;
        }
        Commands::Convert {
            input,
            input_dir,
            output_dir,
        } => {
            let config = config::load_config()?;
            let cache_dir = config::resolve_cache_dir(&config);
            let input_dir = input_dir.as_ref().unwrap_or(&config.convert.input_dir);
            let output_dir = output_dir.as_ref().unwrap_or(&config.convert.output_dir);
            let assimp = tools::resolve_assimp(&cache_dir)?;
            let input = input_dir.join(input);
            convert::convert(&input, &output_dir, &assimp)?;
        }
        Commands::Set {
            extract_input_dir,
            extract_output_dir,
            fps,
            convert_input_dir,
            convert_output_dir,
            cache_dir,
        } => {
            config::set(config::ConfigOverrides {
                cache_dir: cache_dir.clone(),
                extract_input_dir: extract_input_dir.clone(),
                extract_output_dir: extract_output_dir.clone(),
                fps: *fps,
                convert_input_dir: convert_input_dir.clone(),
                convert_output_dir: convert_output_dir.clone(),
            })?;
        }
    }
    Ok(())
}
