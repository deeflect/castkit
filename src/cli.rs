use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "castkit",
    version,
    about = "Agent-native CLI demo video generator"
)]
pub struct Cli {
    #[arg(long, global = true, default_value_t = false)]
    pub json: bool,
    #[arg(short, long, global = true, default_value_t = false)]
    pub verbose: bool,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Handoff(HandoffArgs),
    Plan(PlanArgs),
    Validate(ValidateArgs),
    Execute(Box<ExecuteArgs>),
}

#[derive(Debug, Args)]
pub struct PlanArgs {
    #[command(subcommand)]
    pub command: PlanCommands,
}

#[derive(Debug, Subcommand)]
pub enum PlanCommands {
    Scaffold(PlanScaffoldArgs),
}

#[derive(Debug, Args)]
pub struct PlanScaffoldArgs {
    #[arg(long)]
    pub session: String,
    #[arg(long, default_value = "demo-script.json")]
    pub output: PathBuf,
    #[arg(long, default_value_t = 3)]
    pub max_scenes: usize,
}

#[derive(Debug, Args)]
pub struct HandoffArgs {
    #[command(subcommand)]
    pub command: HandoffCommands,
}

#[derive(Debug, Subcommand)]
pub enum HandoffCommands {
    Init(HandoffInitArgs),
    List(HandoffListArgs),
    Get(HandoffGetArgs),
}

#[derive(Debug, Args)]
pub struct HandoffInitArgs {
    pub target: String,
    #[arg(long)]
    pub readme: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub no_readme: bool,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum HandoffSource {
    Help,
    Readme,
    Files,
    Probes,
}

#[derive(Debug, Args)]
pub struct HandoffListArgs {
    #[arg(long)]
    pub session: String,
    #[arg(long, value_enum)]
    pub source: HandoffSource,
    #[arg(long, default_value_t = 1)]
    pub page: usize,
    #[arg(long, default_value_t = 20)]
    pub per_page: usize,
}

#[derive(Debug, Args)]
pub struct HandoffGetArgs {
    #[arg(long)]
    pub session: String,
    #[arg(long)]
    pub r#ref: String,
}

#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[arg(long)]
    pub session: String,
    #[arg(long)]
    pub script: PathBuf,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    Mp4,
    Gif,
    Webm,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ThemePreset {
    Clean,
    Bold,
    Minimal,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RenderSpeed {
    Fast,
    Quality,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum KeystrokeProfile {
    Mechanical,
    Laptop,
    Silent,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ExecutePreset {
    Quick,
    Balanced,
    Polished,
}

#[derive(Debug, Args)]
pub struct ExecuteArgs {
    #[arg(long)]
    pub session: String,
    #[arg(long)]
    pub script: PathBuf,
    #[arg(long, default_value_t = false)]
    pub non_interactive: bool,
    #[arg(long, default_value = "demo.mp4")]
    pub output: PathBuf,
    #[arg(long, value_enum, default_value_t = OutputFormat::Mp4)]
    pub format: OutputFormat,
    #[arg(long)]
    pub fps: Option<u32>,
    #[arg(long, default_value_t = false)]
    pub no_zoom: bool,
    #[arg(long)]
    pub music: Option<PathBuf>,
    #[arg(long, default_value_t = true)]
    pub typing_sound: bool,
    #[arg(long)]
    pub branding: Option<PathBuf>,
    #[arg(long)]
    pub brand_title: Option<String>,
    #[arg(long)]
    pub watermark: Option<String>,
    #[arg(long)]
    pub avatar_x: Option<String>,
    #[arg(long)]
    pub avatar_url: Option<String>,
    #[arg(long)]
    pub avatar_label: Option<String>,
    #[arg(long)]
    pub avatar_cache_dir: Option<PathBuf>,
    #[arg(long, value_enum)]
    pub preset: Option<ExecutePreset>,
    #[arg(long, value_enum)]
    pub theme: Option<ThemePreset>,
    #[arg(long, value_enum)]
    pub speed: Option<RenderSpeed>,
    #[arg(long, value_enum)]
    pub keystroke_profile: Option<KeystrokeProfile>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_handoff_init() {
        let cli = Cli::parse_from(["castkit", "handoff", "init", "./mycli", "--json"]);
        match cli.command {
            Commands::Handoff(h) => match h.command {
                HandoffCommands::Init(args) => assert_eq!(args.target, "./mycli"),
                _ => panic!("expected init"),
            },
            _ => panic!("expected handoff"),
        }
        assert!(cli.json);
    }

    #[test]
    fn parses_handoff_list() {
        let cli = Cli::parse_from([
            "castkit",
            "handoff",
            "list",
            "--session",
            "sess_1",
            "--source",
            "readme",
            "--page",
            "2",
            "--per-page",
            "50",
        ]);
        match cli.command {
            Commands::Handoff(h) => match h.command {
                HandoffCommands::List(args) => {
                    assert_eq!(args.session, "sess_1");
                    assert_eq!(args.page, 2);
                    assert_eq!(args.per_page, 50);
                }
                _ => panic!("expected list"),
            },
            _ => panic!("expected handoff"),
        }
    }

    #[test]
    fn parses_validate_and_execute() {
        let validate = Cli::parse_from([
            "castkit",
            "validate",
            "--session",
            "sess_2",
            "--script",
            "demo.json",
        ]);
        match validate.command {
            Commands::Validate(args) => {
                assert_eq!(args.session, "sess_2");
                assert_eq!(args.script, PathBuf::from("demo.json"));
            }
            _ => panic!("expected validate"),
        }

        let execute = Cli::parse_from([
            "castkit",
            "execute",
            "--session",
            "sess_2",
            "--script",
            "demo.json",
            "--non-interactive",
            "--output",
            "out.mp4",
        ]);
        match execute.command {
            Commands::Execute(args) => {
                assert!(args.non_interactive);
                assert_eq!(args.output, PathBuf::from("out.mp4"));
            }
            _ => panic!("expected execute"),
        }
    }

    #[test]
    fn parses_plan_scaffold() {
        let cli = Cli::parse_from([
            "castkit",
            "plan",
            "scaffold",
            "--session",
            "sess_42",
            "--output",
            "demo-script.json",
            "--max-scenes",
            "5",
        ]);
        match cli.command {
            Commands::Plan(p) => match p.command {
                PlanCommands::Scaffold(args) => {
                    assert_eq!(args.session, "sess_42");
                    assert_eq!(args.output, PathBuf::from("demo-script.json"));
                    assert_eq!(args.max_scenes, 5);
                }
            },
            _ => panic!("expected plan"),
        }
    }
}
