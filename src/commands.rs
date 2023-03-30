use anyhow::bail;

use std::{
    ffi::{OsStr, OsString},
    fmt::Display,
    process::ExitStatus,
};
use thiserror::Error;

use log::{debug, info};
use subprocess::{Exec, PopenError, Redirection};

use crate::interface::{self, NHCommand};

pub trait NHRunnable {
    fn run(&self) -> anyhow::Result<()>;
}

impl NHRunnable for interface::NHCommand {
    fn run(&self) -> anyhow::Result<()> {
        match self {
            NHCommand::Os(os_args) => os_args.run(),
            NHCommand::Clean(clean_args) => clean_args.run(),
            NHCommand::Home(home_args) => home_args.run(),
            NHCommand::Completions(args) => args.run(),
            s => bail!("Subcommand {s:?} not yet implemented!"),
        }
    }
}

#[derive(Debug, derive_builder::Builder, Default)]
#[builder(derive(Debug), setter(into), default)]
pub struct Command {
    /// Whether to actually run the command or just log it
    dry: bool,
    /// Human-readable message regarding what the command does
    #[builder(setter(strip_option))]
    message: Option<String>,
    /// Whether to capture the stdout or let it inherit the parent
    capture: bool,
    /// Arguments 0..N
    #[builder(setter(custom), default = "vec![]")]
    args: Vec<OsString>,
}

impl CommandBuilder {
    pub fn args(&mut self, input: &[impl AsRef<OsStr>]) -> &mut Self {
        if let Some(args) = &mut self.args {
            args.extend(input.iter().map(|elem| elem.as_ref().to_owned()));
            self
        } else {
            self.args = Some(Vec::new());
            self.args(input)
        }
    }
}

impl Command {
    pub fn run(&self) -> Result<Option<String>, PopenError> {
        let [head, tail @ ..] = &*self.args else {
            panic!("Args was length 0");
        };

        let cmd = if self.capture {
            Exec::cmd(head)
                .args(tail)
                .stderr(Redirection::None)
                .stdout(Redirection::Pipe)
        } else {
            Exec::cmd(head)
                .args(tail)
                .stderr(Redirection::None)
                .stdout(Redirection::None)
        };

        if let Some(m) = &self.message {
            info!("{}", m);
        }
        debug!("{:?}", cmd);

        let result = if self.capture {
            Some(cmd.capture()?.stdout_str())
        } else {
            cmd.join()?;
            None
        };

        Ok(result)
    }
}

#[derive(Debug, Default, derive_builder::Builder)]
#[builder(setter(into), default)]
pub struct BuildCommand {
    /// Human-readable message regarding what the command does
    #[builder(setter(strip_option))]
    message: Option<String>,
    // Flakeref to build
    flakeref: String,
    // Extra arguments passed to nix build
    #[builder(setter(custom))]
    extra_args: Vec<OsString>,
    /// Use nom for the nix build
    nom: bool,
}

impl BuildCommandBuilder {
    pub fn extra_args(&mut self, input: &[impl AsRef<OsStr>]) -> &mut Self {
        if let Some(args) = &mut self.extra_args {
            args.extend(input.iter().map(|elem| elem.as_ref().to_owned()));
            self
        } else {
            self.extra_args = Some(Vec::new());
            self.extra_args(input)
        }
    }
}

impl BuildCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        if let Some(m) = &self.message {
            info!("{}", m);
        }

        let exit = if self.nom {
            let cmd = {
                Exec::cmd("nix")
                    .args(&[
                        "build",
                        &self.flakeref,
                        "--log-format",
                        "internal-json",
                        "--verbose",
                    ])
                    .args(&self.extra_args)
                    .stdout(Redirection::Pipe)
                    .stderr(Redirection::Merge)
                    | Exec::cmd("nom").args(&["--json"])
            }
            .stdout(Redirection::None);
            debug!("{:?}", cmd);
            cmd.join()
        } else {
            let cmd = Exec::cmd("nix")
                .args(&["build", &self.flakeref])
                .args(&self.extra_args)
                .stdout(Redirection::None)
                .stderr(Redirection::Merge);

            debug!("{:?}", cmd);
            cmd.join()
        }?;

        match exit {
            subprocess::ExitStatus::Exited(0) => (),
            other => bail!(ExitError(other)),
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
#[error("Command exited with status {0:?}")]
pub struct ExitError(subprocess::ExitStatus);
