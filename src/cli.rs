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
        #[arg(long, value_delimiter = ',')]
        dirs: Vec<String>,

        #[arg(long, default_value_t = false)]
        merge: bool,

        #[arg(long, value_delimiter = ',')]
        ignore: Option<Vec<String>>,

        #[arg(long, default_value = "both")]
        format: String,

        #[arg(long)]
        output: String,
    },
}
