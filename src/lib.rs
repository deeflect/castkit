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
    std::env::set_var("CASTKIT_VERBOSE", if cli.verbose { "1" } else { "0" });

    match cli.command {
        Commands::Handoff(handoff_args) => match handoff_args.command {
            HandoffCommands::Init(args) => {
                if cli.verbose {
                    eprintln!("[castkit] handoff init target={}", args.target);
                }
                let session = handoff::init_session(args).await?;
                print_output(cli.json, &session)?;
            }
            HandoffCommands::List(args) => {
                if cli.verbose {
                    eprintln!(
                        "[castkit] handoff list session={} source={:?} page={} per_page={}",
                        args.session, args.source, args.page, args.per_page
                    );
                }
                let page = handoff::list_refs(args)?;
                print_output(cli.json, &page)?;
            }
            HandoffCommands::Get(args) => {
                if cli.verbose {
                    eprintln!(
                        "[castkit] handoff get session={} ref={}",
                        args.session, args.r#ref
                    );
                }
                let item = handoff::get_ref(args)?;
                print_output(cli.json, &item)?;
            }
        },
        Commands::Validate(args) => {
            if cli.verbose {
                eprintln!(
                    "[castkit] validate session={} script={}",
                    args.session,
                    args.script.display()
                );
            }
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
            if cli.verbose {
                eprintln!(
                    "[castkit] execute session={} script={} output={}",
                    args.session,
                    args.script.display(),
                    args.output.display()
                );
            }
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

fn print_output<T: Serialize>(json: bool, value: &T) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{}", serde_json::to_string(value)?);
    }
    Ok(())
}
