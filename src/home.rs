use log::trace;
use thiserror::Error;

use crate::{
    commands::{mk_temp, run_command, run_command_capture, NHRunnable},
    interface::{FlakeRef, HomeArgs, HomeRebuildArgs, HomeSubcommand},
};

#[derive(Error, Debug)]
enum HomeRebuildError {
    #[error("Configuration \"{0}\" doesn't exist")]
    ConfigName(String),
    #[error("No confirmation")]
    NoConfirm,
}

impl NHRunnable for HomeArgs {
    fn run(&self) -> anyhow::Result<()> {
        // self.subcommand
        match &self.subcommand {
            HomeSubcommand::Switch(args) => args.rebuild()?,
            HomeSubcommand::Info => home_info()?,
        }

        Ok(())
    }
}

impl HomeRebuildArgs {
    fn rebuild(&self) -> anyhow::Result<()> {
        let out_link = mk_temp("/tmp/nh/home-result-");

        let username = std::env::var("USER").expect("Couldn't get username");

        let hm_config = match &self.configuration {
            Some(c) => {
                if configuration_exists(&self.flakeref, c)? {
                    c.to_owned()
                } else {
                    return Err(HomeRebuildError::ConfigName(c.to_owned()).into())
                }
            }
            None => get_home_output(&self.flakeref, &username)?,
        };

        trace!("{}", hm_config);

        {
            let cmd_flakeref = format!(
                "{}#homeConfigurations.{}.config.home.activationPackage",
                &self.flakeref, hm_config
            );
            let cmd = vec!["nix", "build", "--out-link", &out_link, &cmd_flakeref];

            run_command(&cmd, Some("Building configuration"), self.dry)?;
        }

        {
            let previous_gen = format!("/nix/var/nix/profiles/per-user/{}/home-manager", &username);
            let cmd = vec!["nvd", "diff", &previous_gen, &out_link];

            run_command(&cmd, Some("Comparing changes"), self.dry)?;
        }

        if self.ask {
            let confirmation = dialoguer::Confirm::new()
                .with_prompt("Apply the config?")
                .default(false)
                .interact()?;

            if !confirmation {
                return Err(HomeRebuildError::NoConfirm.into());
            }
        }

        {
            let activator = format!("{}/activate", out_link);
            let cmd: Vec<&str> = vec![&activator];
            run_command(&cmd, Some("Activating"), self.dry)?;
        }

        Ok(())
    }
}

fn home_info() -> anyhow::Result<()> {
    Ok(())
}

fn get_home_output<S: AsRef<str> + std::fmt::Display>(
    flakeref: &FlakeRef,
    username: S,
) -> Result<String, subprocess::PopenError> {
    // Replicate these heuristics
    // https://github.com/nix-community/home-manager/blob/433e8de330fd9c157b636f9ccea45e3eeaf69ad2/home-manager/home-manager#L110

    let hostname = hostname::get()
        .expect("Couldn't get hostname")
        .into_string()
        .unwrap();

    let full_flakef = format!("{}@{}", username, &hostname);

    if configuration_exists(flakeref, &full_flakef)? {
        Ok(full_flakef)
    } else {
        Ok(username.to_string())
    }
}

fn configuration_exists(
    flakeref: &FlakeRef,
    configuration: &str,
) -> Result<bool, subprocess::PopenError> {
    let output = format!("{}#homeConfigurations", flakeref);
    let filter = format!(r#" x: x ? "{}" "#, configuration);

    let cmd_check = vec!["nix", "eval", &output, "--apply", &filter];

    run_command_capture(&cmd_check, None).map(|s| match s.trim() {
        "true" => true,
        "false" => false,
        _ => todo!(),
    })
}