use anyhow::{Result, anyhow, bail};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// where bundled binaries are extracted to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_dir: Option<PathBuf>,
    pub extract: ExtractCfg,
    pub convert: ConvertCfg,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExtractCfg {
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
    pub fps: u8,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConvertCfg {
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
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

// ========== paths ========== //

fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("", "", "xclod")
}

fn config_path() -> PathBuf {
    project_dirs()
        .map(|p| p.config_dir().join("config.toml"))
        .unwrap_or_else(|| home_dir().join(".config").join("xclod").join("config.toml"))
}

pub fn resolve_cache_dir(config: &Config) -> PathBuf {
    config.cache_dir.clone().unwrap_or_else(|| {
        project_dirs()
            .map(|p| p.cache_dir().join("bin"))
            .unwrap_or_else(|| home_dir().join(".cache").join("xclod").join("bin"))
    })
}

// ========== load/save ========== //

fn read_config(path: &Path) -> Result<Config> {
    match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|e| anyhow!("error in {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(anyhow!("couldn't read {}: {e}", path.display())),
    }
}

pub fn load_config() -> Result<Config> {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).map_err(|e| anyhow!("error in {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let config = Config::default();
            save_config(&config, &path)?;
            eprintln!("no config found! wrote basic config to {}", path.display());
            eprintln!("edit config file to set options (or override with command flags!)\n");
            Ok(config)
        }
        Err(e) => Err(anyhow!("couldn't read {}: {e}", path.display())),
    }
}

fn save_config(config: &Config, path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        create_dir_all(dir)?;
    }
    let body = toml::to_string(config).map_err(|e| anyhow!("couldn't serialize config: {e}"))?;
    let header = "\
# xclod configuration file
#
# Managed by `xclod set` (e.g. `xclod set --fps 5`); you can also edit by hand.
# Per-run --input-dir / --output-dir / --fps flags override these values.
#
# [extract] input_dir  = folder holding the source videos
#           output_dir = root folder for extracted JPG frames
#           fps        = frames extracted per second
# [convert] input_dir  = folder holding the source FBX files
#           output_dir = folder for the converted OBJ files
# cache_dir (optional, top-level) = where bundled binaries are extracted
 
";
    std::fs::write(path, format!("{header}{body}"))?;
    Ok(())
}

// ========== config setter ========== //

#[derive(Debug, Default)]
pub struct ConfigOverrides {
    pub cache_dir: Option<PathBuf>,
    pub extract_input_dir: Option<PathBuf>,
    pub extract_output_dir: Option<PathBuf>,
    pub fps: Option<u8>,
    pub convert_input_dir: Option<PathBuf>,
    pub convert_output_dir: Option<PathBuf>,
}

pub fn set(overrides: ConfigOverrides) -> Result<()> {
    let path = config_path();
    let mut config = read_config(&path)?;
    let mut changed: Vec<String> = vec![];

    if let Some(v) = overrides.cache_dir {
        changed.push(format!("cache_dir = {}", v.display()));
        config.cache_dir = Some(v);
    }
    if let Some(v) = overrides.extract_input_dir {
        changed.push(format!("extract.input_dir = {}", v.display()));
        config.extract.input_dir = v;
    }
    if let Some(v) = overrides.extract_output_dir {
        changed.push(format!("extract.output_dir = {}", v.display()));
        config.extract.output_dir = v;
    }
    if let Some(v) = overrides.fps {
        changed.push(format!("extract.fps = {v}"));
        config.extract.fps = v;
    }
    if let Some(v) = overrides.convert_input_dir {
        changed.push(format!("convert.input_dir = {}", v.display()));
        config.convert.input_dir = v;
    }
    if let Some(v) = overrides.convert_output_dir {
        changed.push(format!("convert.output_dir = {}", v.display()));
        config.convert.output_dir = v;
    }

    if changed.is_empty() {
        bail!("nothing to set — pass at least one setting (see `xclod set --help`)");
    }

    save_config(&config, &path)?;

    println!("updated {}", path.display());
    for c in changed {
        println!("  {c}");
    }

    Ok(())
}
