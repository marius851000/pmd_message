use anyhow::{Context, Result};
use clap::Clap;
use pmd_message::MessageBin;
use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::PathBuf,
};

#[derive(Clap)]
/// messagetool allow to extract "messagebin" file, used in 3ds pokemon mystery dungeon
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    /// decode, then encode a messagebin file
    Reencode(ReencodeParameter),
}

#[derive(Clap)]
struct ReencodeParameter {
    /// the input messagebin file to read
    input: PathBuf,
    /// the output messagebin file to write
    output: PathBuf,
}

fn main() -> Result<()> {
    let opts = Opts::parse();

    match opts.subcmd {
        SubCommand::Reencode(ep) => reencode(ep)?,
    }

    Ok(())
}

fn reencode(rp: ReencodeParameter) -> Result<()> {
    let mut input_file =
        BufReader::new(File::open(&rp.input).context("can't open the input file")?);
    let message =
        MessageBin::load_file(&mut input_file).context("can't extract the messagebin file")?;

    let mut output_file =
        BufWriter::new(File::create(&rp.output).context("can't open the result file")?);
    message
        .write(&mut output_file)
        .context("can't encode/write the messagebin file")?;
    Ok(())
}
