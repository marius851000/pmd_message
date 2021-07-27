use anyhow::{Context, Result};
use clap::Clap;
use pmd_code_table::CodeTable;
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
    /// path to the code_table.bin file
    code_table: PathBuf,
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
    println!("reading the code table");
    let code_table_file = BufReader::new(File::open(&rp.code_table).context("can't open the code table file")?);
    let mut code_table = CodeTable::new_from_file(code_table_file).context("can't read the code table file")?;
    code_table.add_missing();
    
    let code_to_text = code_table.generate_code_to_text();
    let text_to_code = code_table.generate_text_to_code();

    println!("decoding...");
    let mut input_file =
        BufReader::new(File::open(&rp.input).context("can't open the input file")?);
    let message =
        MessageBin::load_file(&mut input_file, Some(&code_to_text)).context("can't extract the messagebin file")?;

    println!("encoding...");
    let mut output_file =
        BufWriter::new(File::create(&rp.output).context("can't open the result file")?);
    message
        .write(&mut output_file, Some(&text_to_code))
        .context("can't encode/write the messagebin file")?;
    println!("done !");
    Ok(())
}
