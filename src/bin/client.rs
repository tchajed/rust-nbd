use clap::Parser;
use color_eyre::eyre::{bail, WrapErr};
use color_eyre::Result;
use fork::{daemon, Fork};

use std::fs::{File, OpenOptions};

use nbd::{client::Client, kernel};

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Args {
    #[clap(short, long, default_value = "localhost")]
    host: String,

    #[clap(short, long, help = "disconnect from an existing client")]
    disconnect: bool,

    #[clap(short, long, help = "keep running in the foreground (don't daemonize)")]
    foreground: bool,

    #[clap(default_value = "/dev/nbd0", help = "nbd device to set up")]
    device: String,
}

fn open_nbd(args: &Args) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(&args.device)
        .wrap_err("opening nbd device")
}

fn main() -> Result<()> {
    color_eyre::install()?;
    env_logger::init();

    let args = Args::parse();

    if let Err(err) = sudo::escalate_if_needed() {
        bail!("could not get sudo privilege: {}", err);
    }

    if args.disconnect {
        let nbd = open_nbd(&args)?;
        kernel::close(&nbd)?;
        return Ok(());
    }

    let client = Client::connect(&args.host).wrap_err("connecting to nbd server")?;

    let nbd = match open_nbd(&args) {
        Ok(nbd) => nbd,
        Err(err) => {
            eprintln!("could not open nbd device - do you need to run sudo modprobe nbd?");
            return Err(err);
        }
    };
    kernel::set_client(&nbd, client)?;

    if args.foreground {
        kernel::wait(&nbd)?;
        return Ok(());
    }

    if let Ok(Fork::Child) = daemon(false, false) {
        kernel::wait(&nbd)?;
    }

    Ok(())
}
