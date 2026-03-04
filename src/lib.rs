pub mod branding;
pub mod cli;
pub mod execute;
pub mod handoff;
pub mod render;
pub mod script;
pub mod validate;

use std::fs;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use serde::Serialize;

use crate::cli::{Cli, Commands, HandoffCommands};

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Handoff(handoff_args) => match handoff_args.command {
            HandoffCommands::Init(args) => {
                let session = handoff::init_session(args).await?;
                print_output(cli.json, &session)?;
            }
            HandoffCommands::List(args) => {
                let page = handoff::list_refs(args)?;
                print_output(cli.json, &page)?;
            }
            HandoffCommands::Get(args) => {
                let item = handoff::get_ref(args)?;
                print_output(cli.json, &item)?;
            }
        },
        Commands::Validate(args) => {
            let script_raw = fs::read_to_string(&args.script)
                .with_context(|| format!("failed to read script {}", args.script.display()))?;
            let script = script::parse_script(&script_raw)?;
            let result = validate::validate_script(&args.session, &script)?;
            print_output(cli.json, &result)?;
            if !result.ok {
                return Err(anyhow!("validation failed"));
            }
        }
        Commands::Execute(args) => {
            let script_raw = fs::read_to_string(&args.script)
                .with_context(|| format!("failed to read script {}", args.script.display()))?;
            let script = script::parse_script(&script_raw)?;
            let exec_result = execute::execute(*args, script).await?;
            print_output(cli.json, &exec_result)?;
            if !exec_result.ok {
                return Err(anyhow!("execution failed"));
            }
        }
    }

    Ok(())
}

fn print_output<T: Serialize>(_json: bool, value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
