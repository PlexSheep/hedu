//! # A simple hexdumper with a somewhat fancy format
//!
//! Dump data from any readable source, such as the stdin or a file
#![warn(clippy::pedantic)]

use std::{fs::File, io::IsTerminal, path::PathBuf};

use libpt::log::{error, trace, warn, Level, Logger};

use clap::Parser;

mod dumper;
use dumper::{DataSource, Hedu};

#[derive(Debug, Clone, Parser)]
#[command(
    author,
    version,
    about,
    long_about,
    help_template = r#"{about-section}
{usage-heading} {usage}
{all-args}{tab}

{name}: {version}
Author: {author-with-newline}
"#
)]
/// Hexdumper written in Rust
pub struct Cli {
    /// show more details
    #[arg(short, long)]
    pub verbose: bool,

    /// show additional logging meta data
    #[arg(long)]
    pub meta: bool,

    /// show character representation
    #[arg(short, long)]
    pub chars: bool,

    /// skip first N bytes
    #[arg(short, long, default_value_t = 0)]
    pub skip: usize,

    /// only interpret N bytes (end after N)
    #[arg(short, long, default_value_t = 0)]
    pub limit: usize,

    /// show identical lines
    #[arg(short = 'i', long)]
    pub show_identical: bool,

    /// a data source, probably a file.
    ///
    /// If left empty or set as "-", the program will read from stdin.
    ///
    /// Directories cannot be dumped.
    pub data_source: Vec<String>,
}

fn main() {
    let mut cli = cli_parse();
    let mut sources: Vec<Box<dyn DataSource>> = Vec::new();
    if !cli.data_source.is_empty() && !cli.data_source.contains(&"-".to_string()) {
        for data_source in &cli.data_source {
            let data_source: PathBuf = PathBuf::from(data_source);
            if data_source.is_dir() {
                warn!("'{:?}' is a directory and cannot be dumped, skipping", data_source);
                // std::process::exit(1);
                continue;
            }
            trace!("Trying to open '{:?}'", data_source);
            match File::open(&data_source) {
                Ok(file) => sources.push(Box::new(file)),
                Err(err) => {
                    error!("Could not open '{:?}': {err}", data_source);
                    std::process::exit(1);
                }
            };
        }
    } else {
        trace!("Trying to open stdin");
        let stdin = std::io::stdin();
        if stdin.is_terminal() {
            warn!("Refusing to dump from interactive terminal");
            std::process::exit(2)
        }
        // just for the little header
        cli.data_source = Vec::new();
        cli.data_source.push("stdin".to_string());
        sources.push(Box::new(stdin));
    }
    for (i, source) in sources.iter_mut().enumerate() {
        let mut config = Hedu::new(cli.chars, cli.skip, cli.show_identical, cli.limit);
        // FIXME: find a better way to get the file name
        // Currently, skipped sources make an extra newline here.
        match config.chars {
            false => {
                println!("{:─^59}", format!(" {} ", cli.data_source[i]));
            }
            true => {
                println!("{:─^80}", format!(" {} ", cli.data_source[i]));
            }
        }
        match config.dump(&mut **source) {
            Ok(()) => (),
            Err(err) => {
                error!("Could not dump data of file: {err}");
                std::process::exit(3);
            }
        }
        if i < cli.data_source.len() - 1 {
            config.newline();
        }
    }
}

fn cli_parse() -> Cli {
    let cli = Cli::parse();
    let ll: Level = if cli.verbose {
        Level::INFO
    } else {
        Level::DEBUG
    };
    if cli.meta {
        let _ = Logger::builder().max_level(ll).build();
    } else {
        // less verbose version
        let _ = Logger::builder().show_time(false).build();
    }
    cli
}
