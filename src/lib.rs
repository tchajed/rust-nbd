pub mod client;
mod proto;
pub mod server;

#[cfg(test)]
mod tests {
    use color_eyre::Result;
    use readwrite::ReadWrite;
    use std::io::prelude::*;
    use std::{
        cell::RefCell,
        thread::{self, JoinHandle},
    };

    use crate::{
        client::Client,
        server::{Export, Server},
    };

    fn start_server_client(
        data: Vec<u8>,
    ) -> Result<(JoinHandle<Result<()>>, Client<impl Read + Write>)> {
        let (r1, w1) = pipe::pipe();
        let (r2, w2) = pipe::pipe();
        let s1 = ReadWrite::new(r1, w2);
        let s2 = ReadWrite::new(r2, w1);

        let export = Export {
            name: "default".to_string(),
            file: RefCell::new(data),
        };
        let s_handle = thread::spawn(move || -> Result<()> {
            let server = Server::new(export);
            server.handle_client(s1)?;
            Ok(())
        });

        let client = Client::new(s2)?;

        Ok((s_handle, client))
    }

    #[test]
    fn run_client_server_handshake() -> Result<()> {
        let _ = env_logger::builder().is_test(true).try_init();

        let data = vec![1u8; 1024 * 10];
        let (s_handle, client) = start_server_client(data)?;

        client.disconnect()?;
        s_handle.join().unwrap()?;
        Ok(())
    }

    #[test]
    fn client_hard_disconnect() -> Result<()> {
        let _ = env_logger::builder().is_test(true).try_init();

        let data = vec![1u8; 1024 * 10];
        let (s_handle, client) = start_server_client(data)?;

        // we don't call disconnect on client, but drop it to close the connection
        drop(client);

        s_handle.join().unwrap()?;
        Ok(())
    }

    #[test]
    fn run_client_server_read_write() -> Result<()> {
        let _ = env_logger::builder().is_test(true).try_init();

        let data = vec![1u8; 1024 * 10];
        let (s_handle, mut client) = start_server_client(data)?;

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
