use std::net::TcpListener;
use std::thread;

use nbd::handle_echo;

fn main() -> std::io::Result<()> {
    let addr = ("127.0.0.1", 10809);
    println!("listening on {}:{}", addr.0, addr.1);
    let listener = TcpListener::bind(addr)?;
    for stream in listener.incoming() {
        let stream = stream?;
        thread::spawn(|| handle_echo(stream).expect("error handling stream"));
    }
    Ok(())
}
