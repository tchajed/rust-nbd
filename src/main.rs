use clap::Parser;
use color_eyre::Result;
use std::fs::OpenOptions;

use nbd::{Export, Server};

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Args {
    #[clap(long)]
    no_create: bool,

    #[clap(long, default_value = "default")]
    export: String,

    #[clap(short, long, default_value_t = 10)]
    size: usize,

    #[clap(default_value = "disk.img")]
    filename: String,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    env_logger::init();

    let args = Args::parse();
    let create = !args.no_create;
    let size_bytes = args.size as u64 * 1024 * 1024;

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(create)
        .open(args.filename)?;

    file.set_len(size_bytes)?;

    let export = Export {
        name: args.export,
        file,
    };
    Server::new(export).start()?;
    Ok(())
}
