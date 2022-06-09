use std::fs::OpenOptions;
use std::os::unix::io::IntoRawFd;

use color_eyre::eyre::WrapErr;
use color_eyre::Result;
use nbd::{client::Client, kernel};

fn main() -> Result<()> {
    color_eyre::install()?;
    env_logger::init();

    let client = Client::connect("localhost").wrap_err("connecting to nbd server")?;
    let size = client.size();

    let nbd = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/nbd0")
        .wrap_err("opening nbd device")?;
    _ = kernel::set_blksize(&nbd, 4096);
    _ = kernel::set_size_blocks(&nbd, size / 4096);
    _ = kernel::set_size(&nbd, size);
    kernel::clear_sock(&nbd)?;

    let sock = client.into_raw_fd();
    kernel::set_sock(&nbd, sock).wrap_err("could not set nbd sock")?;
    kernel::do_it(&nbd).wrap_err("could not send do_it")?;

    Ok(())
}
