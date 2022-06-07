use std::fs::OpenOptions;

use nbd::{Export, Server};

fn main() -> std::io::Result<()> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("disk.img")?;
    let export = Export {
        name: "default".to_string(),
        file,
    };
    let server = Server::new(export);
    server.start()?;
    Ok(())
}
