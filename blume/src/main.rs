mod config;
mod init;
mod deuni;

use std::path::PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Args {
    #[arg(short, help = "Path to the blume working database file")]
    file: PathBuf,
    #[command(subcommand)]
    command: Command
}

#[derive(Subcommand)]
enum Command {
    Config(config::Args),
    Init(init::Args),
    #[command(name = "deuni")]
    DeUni(deuni::Args)
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    use Command::*;
    match args.command {
        Config(margs) => config::run(args.file, margs),
        Init(margs) => init::run(args.file, margs),
        DeUni(margs) => deuni::run(args.file, margs)
    }
}
