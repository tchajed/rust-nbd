use std::{fs::File, os::unix::io::IntoRawFd};

use color_eyre::Result;
use nbd::{client::Client, kernel};

fn main() -> Result<()> {
    let nbd = File::open("/dev/nbd0")?;
    let client = Client::connect("localhost")?;
    let size = client.size();
    let sock = client.into_raw_fd();
    kernel::set_blksize(&nbd, 4096)?;
    kernel::set_size_blocks(&nbd, size / 4096)?;
    kernel::set_sock(&nbd, sock)?;
    kernel::do_it(&nbd)?;
    Ok(())
}
