use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "grafyx", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
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
