use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub struct Cli {
    /// Executable source directory
    #[arg(long, short)]
    pub source: PathBuf,

    /// Executable target directory
    #[arg(long, short)]
    pub target: PathBuf,

    /// Output directory
    #[arg(long, short = 'O')]
    pub out_dir: PathBuf,
}
