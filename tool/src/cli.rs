use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "grafyx", version, about, long_about = None)]
pub struct Cli {
    #[arg(long, num_args = 1.., conflicts_with = "command", requires = "output")]
    pub dirs: Option<Vec<String>>,

    #[arg(long, num_args = 1.., conflicts_with = "command")]
    pub ignore: Option<Vec<String>>,

    #[arg(long, default_value = "both", conflicts_with = "command", requires = "dirs")]
    pub format: OutputFormat,

    #[arg(long, conflicts_with = "command", requires = "dirs")]
    pub output: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Json,
    Sqlite,
    Both,
}

#[derive(Subcommand)]
pub enum Commands {
    Scan {
        #[arg(long, num_args = 1..)]
        dirs: Vec<String>,

        #[arg(long, num_args = 1..)]
        ignore: Option<Vec<String>>,

        #[arg(long, default_value = "both")]
        format: OutputFormat,

        #[arg(long)]
        output: String,
    },
    #[command(alias = "i")]
    Install,
    #[command(alias = "un")]
    Uninstall,
    #[cfg(feature = "self-update")]
    #[command(alias = "u")]
    Upgrade,
}
