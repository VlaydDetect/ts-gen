use std::process::{Command, Stdio};

use color_eyre::Result;

use crate::{args::Args, path};

macro_rules! feature {
    ($cargo_invocation: expr, $args: expr, { $($field: ident => $feature: literal),* $(,)? }) => {
        $(
            if $args.$field {
                $cargo_invocation
                    .arg("--features")
                    .arg(format!("ts-gen/{}", $feature));
            }
        )*
    };
}

pub fn invoke(args: &Args) -> Result<()> {
    let mut cargo_invocation = Command::new("cargo");

    cargo_invocation
        .arg("test")
        .arg("export_bindings_")
        .arg("--features")
        .arg("ts-gen/export")
        .arg("--features")
        .arg("ts-gen/generate-metadata")
        .stdout(if args.no_capture {
            Stdio::inherit()
        } else {
            Stdio::piped()
        })
        .env(
            "TS_GEN_EXPORT_DIR",
            path::absolute(path::export_dir(&args))?,
        );

    feature!(cargo_invocation, args, {
        no_warnings => "no-serde-warnings",
        esm_imports => "import-esm",
        format => "format",
    });

    if args.no_capture {
        cargo_invocation.arg("--").arg("--nocapture");
    } else {
        cargo_invocation.arg("--quiet");
    }

    cargo_invocation.spawn()?.wait()?;

    Ok(())
}
