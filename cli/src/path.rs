use std::{
    borrow::Cow,
    path::{Component, Path, PathBuf},
};

use crate::args::Args;
use color_eyre::{eyre::OptionExt, Result};

pub fn absolute<T: AsRef<Path>>(path: T) -> Result<PathBuf> {
    let path = path.as_ref();

    if path.is_absolute() {
        return Ok(path.to_owned());
    }

    let path = std::env::current_dir()?.join(path);

    let mut out = Vec::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => (),
            Component::ParentDir => {
                out.pop().ok_or_eyre("Invalid path")?;
            }
            comp => out.push(comp),
        }
    }

    Ok(if out.is_empty() {
        PathBuf::from(".")
    } else {
        out.iter().collect()
    })
}

fn env_export_dir() -> Cow<'static, Path> {
    match option_env!("TS_GEN_EXPORT_DIR") {
        None => Cow::Borrowed(Path::new("./bindings")),
        Some(dir) => Cow::Owned(PathBuf::from(dir)),
    }
}

pub fn export_dir(args: &Args) -> PathBuf {
    match &args.output_directory {
        None => env_export_dir().to_path_buf(),
        Some(dir) => dir.clone(),
    }
}
