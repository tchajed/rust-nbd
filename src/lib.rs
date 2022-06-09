pub mod client;
mod proto;
pub mod server;

#[cfg(test)]
mod tests {
    use color_eyre::Result;
    use readwrite::ReadWrite;
    use std::{cell::RefCell, thread};

    use crate::{
        client::Client,
        server::{Export, Server},
    };

    #[test]
    fn run_client_server_handshake() -> Result<()> {
        let _ = env_logger::builder().is_test(true).try_init();
        let (r1, w1) = pipe::pipe();
        let (r2, w2) = pipe::pipe();
        let s1 = ReadWrite::new(r1, w2);
        let s2 = ReadWrite::new(r2, w1);

        let data = vec![1u8; 1024 * 10];
        let export = Export {
            name: "default".to_string(),
            file: RefCell::new(data),
        };
        let s_handle = thread::spawn(move || -> Result<()> {
            let server = Server::new(export);
            server.handle_client(s1)?;
            Ok(())
        });

        let mut client = Client::new(s2)?;

        let buf = client.read(3, 5)?;
        assert_eq!(buf, [1u8; 5]);
        client.write(4, &[9u8; 7])?;
        client.flush()?;
        let buf = client.read(2, 4)?;
        assert_eq!(buf, [1, 1, 9, 9]);

        client.disconnect()?;

        s_handle.join().unwrap()?;
        Ok(())
    }
}
