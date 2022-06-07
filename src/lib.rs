use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::io::prelude::*;
use std::net::TcpListener;
use std::os::unix::prelude::FileExt;
use std::{fs::File, io};

use std::error::Error;

use bitflags::bitflags;
use byteorder::{ReadBytesExt, WriteBytesExt, BE};

const TCP_PORT: u16 = 10809;

const MAGIC: u64 = 0x4e42444d41474943; // b"NBDMAGIC"
const IHAVEOPT: u64 = 0x49484156454F5054; // b"IHAVEOPT"
const REPLY_MAGIC: u64 = 0x3e889045565a9;

// transmission constants
const REQUEST_MAGIC: u32 = 0x25609513;
const SIMPLE_REPLY_MAGIC: u32 = 0x67446698;

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

#[derive(IntoPrimitive, TryFromPrimitive, Debug)]
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

#[derive(IntoPrimitive, TryFromPrimitive, Debug)]
#[repr(u32)]
#[allow(non_camel_case_types)]
enum ReplyType {
    ACK = 1,
    SERVER = 2,
    ERR_UNSUP = (1 << 31) + 1,
    ERR_TOO_BIG = (1 << 31) + 9,
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

impl Option {
    fn get<IO: Read>(mut stream: IO) -> io::Result<Self> {
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
        Ok(Self { typ, data })
    }
}

#[derive(IntoPrimitive, TryFromPrimitive, Debug, PartialEq, Eq)]
#[repr(u16)]
#[allow(non_camel_case_types)]
enum Cmd {
    READ = 0,
    WRITE = 1,
    // NBD_CMD_DISC
    DISCONNECT = 2,
    FLUSH = 3,
    TRIM = 4,
    CACHE = 5,
    WRITE_ZEROES = 6,
    BLOCK_STATUS = 7,
    RESIZE = 8,
}

struct Request {
    flags: u16,
    typ: Cmd,
    handle: u64,
    offset: u64,
    len: u32, // used for READ (redundant for WRITE)
    data: Vec<u8>,
}

impl Request {
    fn get<IO: Read + Write>(mut stream: IO) -> io::Result<Self> {
        let magic = stream.read_u32::<BE>()?;
        if magic != REQUEST_MAGIC {
            return other_error(format!("wrong request magic {}", magic));
        }
        let flags = stream.read_u16::<BE>()?;
        let typ = stream.read_u16::<BE>()?;
        let typ = Cmd::try_from(typ).map_err(|_| {
            io::Error::new(io::ErrorKind::Other, format!("unexpected command {}", typ))
        })?;
        let handle = stream.read_u64::<BE>()?;
        let offset = stream.read_u64::<BE>()?;
        let len = stream.read_u32::<BE>()?;
        let data = {
            if typ == Cmd::WRITE {
                if len > 100_000_000 {
                    SimpleReply {
                        err: ErrorType::EOVERFLOW,
                        handle,
                        data: vec![],
                    }
                    .put(&mut stream)?;
                    // TODO: probably shouldn't terminate in this case?
                    return other_error(format!("large write request"));
                }
                let mut buf = vec![0; len as usize];
                stream.read_exact(&mut buf)?;
                buf
            } else {
                vec![]
            }
        };
        Ok(Self {
            flags,
            typ,
            handle,
            offset,
            len,
            data,
        })
    }
}

#[derive(IntoPrimitive, TryFromPrimitive, Debug)]
#[repr(u32)]
#[allow(non_camel_case_types)]
enum ErrorType {
    OK = 0,
    EINVAL = 22,
    ENOSPC = 28,
    EOVERFLOW = 75,
    ENOTSUP = 95,
}

#[derive(Debug)]
struct SimpleReply {
    err: ErrorType,
    handle: u64,
    // TODO: use reference
    data: Vec<u8>,
}

impl SimpleReply {
    fn data(req: &Request, data: Vec<u8>) -> Self {
        SimpleReply {
            err: ErrorType::OK,
            handle: req.handle,
            data,
        }
    }

    fn ok(req: &Request) -> Self {
        Self::data(req, vec![])
    }

    fn put<IO: Write>(self, mut stream: IO) -> io::Result<()> {
        stream.write_u32::<BE>(SIMPLE_REPLY_MAGIC)?;
        stream.write_u32::<BE>(self.err.into())?;
        stream.write_u64::<BE>(self.handle)?;
        stream.write_all(&self.data)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Export {
    pub name: String,
    pub file: File,
}

impl Export {
    fn read(&self, off: u64, len: u32) -> io::Result<Vec<u8>> {
        let mut buf = vec![0; len as usize];
        self.file.read_exact_at(&mut buf, off)?;
        Ok(buf)
    }

    fn write(&self, off: u64, data: &[u8]) -> io::Result<()> {
        self.file.write_all_at(data, off)?;
        Ok(())
    }

    fn flush(&self) -> io::Result<()> {
        self.file.sync_data()?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Server {
    export: Export,
}

impl Server {
    pub fn new(export: Export) -> Self {
        Self { export }
    }

    // agree on basic negotiation flags (only fixed newstyle is supported so
    // this returns a unit)
    fn initial_handshake<IO: Read + Write>(mut stream: IO) -> io::Result<()> {
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

    fn reply<IO: Write>(
        mut stream: IO,
        opt: OptionType,
        reply_type: ReplyType,
        data: &[u8],
    ) -> io::Result<()> {
        stream.write_u64::<BE>(REPLY_MAGIC)?;
        stream.write_u32::<BE>(opt.into())?;
        stream.write_u32::<BE>(reply_type.into())?;
        stream.write_u32::<BE>(data.len() as u32)?;
        stream.write_all(data)?;
        stream.flush()?;
        Ok(())
    }

    // after the initial handshake, "haggle" to agree on connection parameters
    fn handshake_haggle<IO: Read + Write>(&self, mut stream: IO) -> io::Result<&Export> {
        // only need to handle OPT_EXPORT_NAME, that will make the server functional
        loop {
            let opt = Option::get(&mut stream)?;
            match opt.typ {
                OptionType::EXPORT_NAME => {
                    let export: String = String::from_utf8(opt.data).map_err(|_| {
                        io::Error::new(io::ErrorKind::Other, "non-UTF8 export name")
                    })?;
                    if export != self.export.name {
                        // protocol has no way to recover from this (it is
                        // handled by NBD_OPT_GO, but that isn't supported)
                        return other_error(format!("incorrect export name {export}"));
                    }
                    return Ok(&self.export);
                }
                _ => {
                    Self::reply(&mut stream, opt.typ, ReplyType::ERR_UNSUP, &[])?;
                }
            }
        }
    }

    fn handle_ops<IO: Read + Write>(export: &Export, mut stream: IO) -> io::Result<()> {
        loop {
            let req = Request::get(&mut stream)?;
            match req.typ {
                Cmd::READ => {
                    let data = export.read(req.offset, req.len)?;
                    SimpleReply::data(&req, data).put(&mut stream)?;
                }
                Cmd::WRITE => {
                    export.write(req.offset, &req.data)?;
                    SimpleReply::ok(&req).put(&mut stream)?;
                }
                Cmd::DISCONNECT => return Ok(()),
                Cmd::FLUSH => {
                    export.flush()?;
                    SimpleReply::ok(&req).put(&mut stream)?;
                }
                _ => {
                    SimpleReply::ok(&req).put(&mut stream)?;
                    return Ok(());
                }
            }
        }
    }

    fn client<IO: Read + Write>(&self, mut stream: IO) -> io::Result<()> {
        Self::initial_handshake(&mut stream)?;
        let export = self.handshake_haggle(&mut stream)?;
        Server::handle_ops(export, &mut stream)?;
        Ok(())
    }

    pub fn start(self) -> io::Result<()> {
        let addr = ("127.0.0.1", TCP_PORT);
        let listener = TcpListener::bind(addr)?;
        for stream in listener.incoming() {
            let stream = stream?;
            println!("client connected");
            // TODO: how to process clients in parallel? self has to be shared among threads
            self.client(stream).expect("error handling stream");
            println!("disconnect");
        }
        Ok(())
    }
}
