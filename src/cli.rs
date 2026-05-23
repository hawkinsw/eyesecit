use chrono::prelude::*;
use clap::{value_parser, Parser};
use clio::ClioPath;

#[derive(Parser, Debug)]
#[command(author, about, long_about = None)]
pub struct Args {
    /// Skip updates before this date.
    #[arg(short, long, value_parser = parse_cli_start_date)]
    pub start_date: Option<DateTime<FixedOffset>>,

    /// Skip updates before this date.
    #[arg(short, long, value_parser = value_parser!(ClioPath).exists())]
    pub config: ClioPath,

    /// Enable debug output; specify repeatedly for increasingly detailed output.
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub debug: u8,

    /// Send a hello tweet when the bot starts.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub hello: bool,

    /// Execute in dry-run mode.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub dry: bool,
}

fn parse_cli_start_date(
    s: &str,
) -> Result<DateTime<FixedOffset>, Box<dyn std::error::Error + Send + Sync + 'static>> {
    s.parse()
        .or(Err("Could not parse your input to a valid date.".into()))
}
