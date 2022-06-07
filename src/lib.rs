use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::io::prelude::*;
use std::net::TcpListener;
use std::thread;
use std::{io, net::TcpStream};

use std::error::Error;

use bitflags::bitflags;
use byteorder::{ReadBytesExt, WriteBytesExt, BE};

fn handle_echo(mut stream: TcpStream) -> io::Result<()> {
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

#[derive(Debug, Default)]
pub struct Server {}

const MAGIC: u64 = 0x4e42444d41474943;
const IHAVEOPT: u64 = 0x49484156454F5054;

bitflags! {
  struct HandshakeFlags: u16 {
    const FIXED_NEWSTYLE = 0b01;
    const NO_ZEROES = 0b10;
  }

  struct ClientHandshakeFlags: u32 {
    const C_FIXED_NEWSTYLE = 0b01;
    const C_NO_ZEROES = 0b10;
  }

  struct TransmitFlags: u16 {
    const HAS_FLAGS = 1 << 0;
    const READ_ONLY = 1 << 1;
    const SEND_FLUSH = 1 << 2;
    const SEND_FUA = 1 << 3;
    const ROTATIONAL = 1 << 4;
    const SEND_TRIM = 1 << 5;
    const SEND_WRITE_ZEROES = 1 << 6;
    const SEND_DF = 1 << 7;
    const CAN_MULTI_CONN = 1 << 8;
    const SEND_RESIZE = 1 << 9;
    const SEND_CACHE = 1 << 10;
    const SEND_FAST_ZERO = 1 << 11;
  }
}

#[derive(IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
#[allow(non_camel_case_types)]
enum OptionType {
    EXPORT_NAME = 1,
    ABORT = 2,
    LIST = 3,
    PEEK_EXPORT = 4,
    STARTTLS = 5,
    INFO = 6,
    GO = 7,
}

pub struct ConnConfig {
    export: String,
}

fn other_error<T, E>(e: E) -> io::Result<T>
where
    E: Into<Box<dyn Error + Send + Sync>>,
{
    Err(io::Error::new(io::ErrorKind::Other, e))
}

struct Option {
    typ: OptionType,
    data: Vec<u8>,
}

impl Server {
    // agree on basic negotiation flags (only fixed newstyle is supported so
    // this returns a unit)
    fn initial_handshake<IO: Read + Write>(&self, mut stream: IO) -> io::Result<()> {
        stream.write_u64::<BE>(MAGIC)?;
        stream.write_u64::<BE>(IHAVEOPT)?;
        stream.write_u16::<BE>(HandshakeFlags::FIXED_NEWSTYLE.bits)?;
        let client_flags = stream.read_u32::<BE>()?;
        let client_flags = match ClientHandshakeFlags::from_bits(client_flags) {
            Some(flags) => flags,
            None => return other_error(format!("unexpected client flags {}", client_flags)),
        };
        if !client_flags
            .contains(ClientHandshakeFlags::C_FIXED_NEWSTYLE | ClientHandshakeFlags::C_NO_ZEROES)
        {
            return other_error(format!(
                "client is missing required flags {:?}",
                client_flags
            ));
        }
        Ok(())
    }

    fn get_option<IO: Read>(&self, mut stream: IO) -> io::Result<Option> {
        let magic = stream.read_u64::<BE>()?;
        if magic != IHAVEOPT {
            return other_error(format!("unexpected option magic {magic}"));
        }
        let option = stream.read_u32::<BE>()?;
        let typ = OptionType::try_from(option)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "unexpected option"))?;
        let option_len = stream.read_u32::<BE>()?;
        if option_len > 100000 {
            return other_error(format!("option length {option_len} is too large"));
        }
        let mut data = vec![0u8; option_len as usize];
        stream.read_exact(&mut data)?;
        Ok(Option { typ, data })
    }

    // after the initial handshake, "haggle" to agree on connection parameters
    fn handshake_haggle<IO: Read + Write>(&self, mut stream: IO) -> io::Result<ConnConfig> {
        // only need to handle OPT_EXPORT_NAME, that will make the server functional
        loop {
            let opt = self.get_option(&mut stream)?;
        }
        todo!("process options")
    }

    fn client<IO: Read + Write>(&self, mut stream: IO) -> io::Result<()> {
        self.initial_handshake(&mut stream)?;
        let config = self.handshake_haggle(&mut stream)?;
        // need to implement actual operations on ConnConfig
        todo!()
    }

    pub fn start(self) -> io::Result<()> {
        let addr = ("127.0.0.1", 10809);
        let listener = TcpListener::bind(addr)?;
        for stream in listener.incoming() {
            let stream = stream?;
            // TODO: how to process clients in parallel? self has to be shared among threads
            self.client(stream).expect("error handling stream");
        }
        Ok(())
    }
}
