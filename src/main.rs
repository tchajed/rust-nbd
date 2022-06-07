use nbd::Server;

fn main() -> std::io::Result<()> {
    let server = Server::default();
    server.start()?;
    Ok(())
}
