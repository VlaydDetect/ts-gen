use std::path::{Component, Path, PathBuf};
use crate::error::{Error, Result};

const ERROR_MESSAGE: &str = r#"The path provided with `#[ts(export_to = "..")]` is not valid"#;

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
                out.pop().ok_or(Error::CannotBeExported(ERROR_MESSAGE))?;
            }
            comp => out.push(comp),
        }
    }

    Ok(if !out.is_empty() {
        out.iter().collect()
    } else {
        PathBuf::from(".")
    })
}