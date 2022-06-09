pub mod client;
mod kernel;
mod proto;
pub mod server;

#[cfg(test)]
mod tests {
    use color_eyre::Result;
    use readwrite::ReadWrite;
    use std::io::prelude::*;
    use std::thread::{self, JoinHandle};

    use crate::server::MemBlocks;
    use crate::{client::Client, server::Server};

    struct ServerClient<IO: Read + Write> {
        server: JoinHandle<Result<()>>,
        client: Client<IO>,
    }

    impl<IO: Read + Write> ServerClient<IO> {
        fn shutdown(self) -> Result<()> {
            self.client.disconnect()?;
            self.server.join().unwrap()?;
            Ok(())
        }
    }

    fn start_server_client(data: Vec<u8>) -> Result<ServerClient<impl Read + Write>> {
        let _ = env_logger::builder().is_test(true).try_init();
        let (r1, w1) = pipe::pipe();
        let (r2, w2) = pipe::pipe();
        let s1 = ReadWrite::new(r1, w2);
        let s2 = ReadWrite::new(r2, w1);

        let s_handle = thread::spawn(move || -> Result<()> {
            let server = Server::new(MemBlocks::new(data));
            server.handle_client(s1)?;
            Ok(())
        });

        let client = Client::new(s2)?;

        Ok(ServerClient {
            server: s_handle,
            client,
        })
    }

    #[test]
    fn run_client_server_handshake() -> Result<()> {
        let data = vec![1u8; 1024 * 10];
        let sc = start_server_client(data)?;

        sc.shutdown()?;
        Ok(())
    }

    #[test]
    fn client_hard_disconnect() -> Result<()> {
        let data = vec![1u8; 1024 * 10];
        let ServerClient { server, client } = start_server_client(data)?;

        // we don't call disconnect on client, but drop it to close the connection
        drop(client);
        // server should not error in this situation
        server.join().unwrap()?;

        Ok(())
    }

    #[test]
    fn client_export_size() -> Result<()> {
        let len = 15341;
        let data = vec![1u8; len];
        let sc = start_server_client(data)?;

        assert_eq!(sc.client.size(), len as u64);

        sc.shutdown()?;
        Ok(())
    }

    #[test]
    fn run_client_server_read_write() -> Result<()> {
        let data = vec![1u8; 1024 * 10];
        let mut sc = start_server_client(data)?;
        let client = &mut sc.client;

        let buf = client.read(3, 5)?;
        assert_eq!(buf, [1u8; 5]);
        client.write(4, &[9u8; 7])?;
        client.flush()?;
        let buf = client.read(2, 4)?;
        assert_eq!(buf, [1, 1, 9, 9]);

        sc.shutdown()?;
        Ok(())
    }
}
