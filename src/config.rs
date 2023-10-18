use clap::Parser;

#[derive(Debug, Parser)]
pub struct Cli {
    #[clap(subcommand)]
    pub subcommand: Subcommand,
}

#[derive(Debug, Parser)]
pub enum Subcommand {
    RunOnce(RunOnce),
    Serve(Server),
}

#[derive(Debug, Parser)]
pub struct RunOnce {
    #[arg(long)]
    pub instance: String,
    #[arg(long)]
    pub token: String,
}

#[derive(Debug, Parser)]
pub struct Server {
    #[arg(long)]
    pub database: String,
    #[arg(long, default_value = "0.0.0.0")]
    pub addr: String,
    #[arg(long, default_value = "3001")]
    pub port: u16,
}
