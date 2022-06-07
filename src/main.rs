use color_eyre::Result;
use std::fs::OpenOptions;

use nbd::{Export, Server};

fn main() -> Result<()> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("disk.img")?;
    file.set_len(10 * 1024 * 1024)?;
    let export = Export {
        name: "default".to_string(),
        file,
    };
    let server = Server::new(export);
    server.start()?;
    Ok(())
}
