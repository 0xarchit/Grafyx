use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "kgraph", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Scan {
        #[arg(long, num_args = 1..)]
        dirs: Vec<String>,

        #[arg(long, default_value_t = false)]
        merge: bool,

        #[arg(long, num_args = 1..)]
        ignore: Option<Vec<String>>,

        #[arg(long, default_value = "both")]
        format: String,

        #[arg(long)]
        output: String,
    },
}
