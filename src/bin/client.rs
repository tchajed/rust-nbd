use clap::Parser;
use color_eyre::eyre::{bail, WrapErr};
use color_eyre::Result;
use fork::{daemon, Fork};

use std::fs::{File, OpenOptions};
use std::os::unix::io::IntoRawFd;

use nbd::{client::Client, kernel};

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Args {
    #[clap(short, long, default_value = "localhost")]
    host: String,

    #[clap(short, long)]
    disconnect: bool,

    #[clap(default_value = "/dev/nbd0")]
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
        kernel::clear_sock(&nbd).wrap_err("could not disconnect")?;
        kernel::disconnect(&nbd).wrap_err("could not disconnect")?;
        return Ok(());
    }

    let client = Client::connect(&args.host).wrap_err("connecting to nbd server")?;
    let size = client.size();

    let nbd = match open_nbd(&args) {
        Ok(nbd) => nbd,
        Err(err) => {
            eprintln!("could not open nbd device - do you need to run sudo modprobe nbd?");
            return Err(err);
        }
    };
    kernel::set_blksize(&nbd, 4096)?;
    kernel::set_size_blocks(&nbd, size / 4096)?;
    kernel::set_flags(&nbd)?;
    kernel::clear_sock(&nbd)?;

    let sock = client.into_raw_fd();
    kernel::set_sock(&nbd, sock).wrap_err("could not set nbd sock")?;

    if let Ok(Fork::Child) = daemon(false, false) {
        kernel::do_it(&nbd)
            .wrap_err("error waiting for nbd")
            .unwrap();
    }

    Ok(())
}
