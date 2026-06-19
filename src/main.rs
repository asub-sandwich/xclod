mod cli;
mod config;
mod convert;
mod extract;
mod tools;

use anyhow::Result;
use clap::Parser;
use dialoguer::Select;

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

            let mut using_working_directory = false;

            let true_input = match input.is_file() {
                true => {
                    let default_input = input_dir.join(input);
                    match default_input.is_file() {
                        true => {
                            let files = vec![&default_input, input];
                            let items: Vec<_> = files.iter().map(|p| p.display()).collect();
                            let selection = Select::new()
                                .with_prompt("found two files... which do you want?")
                                .items(&items)
                                .interact()?;
                            if selection == 1 {
                                using_working_directory = true;
                                println!(
                                    "warning: ignoring config/flags and using current working directory for input/output paths..."
                                );
                            }
                            files[selection].to_path_buf()
                        }
                        false => input.to_path_buf(),
                    }
                }
                false => input_dir.join(input),
            };

            let true_output = match using_working_directory {
                true => output.to_path_buf(),
                false => output_dir.join(output),
            };

            extract::extract(&true_input, &true_output, sample_name, fps, exiftool)?;
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
            let true_input = match input.is_file() {
                true => {
                    let default_input = input_dir.join(input);
                    match default_input.is_file() {
                        true => {
                            let files = vec![&default_input, input];
                            let items: Vec<_> = files.iter().map(|p| p.display()).collect();
                            let selection = Select::new()
                                .with_prompt("found two files... which do you want?")
                                .items(&items)
                                .interact()?;
                            files[selection].to_path_buf()
                        }
                        false => input.to_path_buf(),
                    }
                }
                false => input_dir.join(input),
            };
            convert::convert(&true_input, &output_dir, &assimp)?;
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
