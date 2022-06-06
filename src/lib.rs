use std::io::prelude::*;
use std::{io, net::TcpStream};

pub fn handle_echo(mut stream: TcpStream) -> io::Result<()> {
    println!("hello");
    stream.set_nodelay(true)?;
    let mut buf: Vec<u8> = vec![Default::default(); 4096];
    loop {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            println!("bye");
            return Ok(());
        }
        stream.write(&buf[..n])?;
    }
}
