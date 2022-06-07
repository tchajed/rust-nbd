use std::{
    fs::{File, OpenOptions},
    io::Write,
};

use nbd::{Export, Server};

fn set_size(f: &mut File, size: usize) -> std::io::Result<()> {
    let meta = f.metadata()?;
    if meta.len() == 0 {
        let write_size = 4096;
        let mut buf = vec![0; write_size];
        for _ in 0..(size / write_size) {
            f.write_all(&mut buf)?;
        }
        let bytes_written = (size / write_size) * write_size;
        f.write_all(&buf[..(size - bytes_written)])?;
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("disk.img")?;
    set_size(&mut file, 10_000_000)?;
    let export = Export {
        name: "default".to_string(),
        file,
    };
    let server = Server::new(export);
    server.start()?;
    Ok(())
}
