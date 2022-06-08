//! Network Block Device server, exporting an underlying file.
//!
//! Implements the most basic parts of the protocol: a single export,
//! read/write/flush commands, and no other flags (eg, read-only or TLS
//! support).
//!
//! See <https://github.com/NetworkBlockDevice/nbd/blob/master/doc/proto.md> for
//! the protocol description.

#![deny(missing_docs)]
use color_eyre::eyre::{bail, WrapErr};
use color_eyre::Result;

use std::fs::File;
use std::io::{self, prelude::*};
use std::net::TcpListener;
use std::os::unix::prelude::FileExt;

use byteorder::{ReadBytesExt, WriteBytesExt, BE};
use log::{info, warn};

mod proto;
use proto::*;

/// A file to be exported as a block device.
#[derive(Debug)]
pub struct Export {
    /// name of the export (only used for listing)
    pub name: String,
    /// file to be exported
    pub file: File,
}

impl Export {
    fn read(&self, off: u64, len: u32, buf: &mut [u8]) -> core::result::Result<(), ErrorType> {
        if buf.len() < len as usize {
            return Err(ErrorType::EOVERFLOW);
        }
        match self.file.read_at(buf, off) {
            Ok(_) => Ok(()),
            Err(err) => Err(ErrorType::from_io_kind(err.kind())),
        }
    }

    fn write(&self, off: u64, data: &[u8]) -> core::result::Result<(), ErrorType> {
        self.file
            .write_all_at(data, off)
            .map_err(|err| ErrorType::from_io_kind(err.kind()))
    }

    fn flush(&self) -> io::Result<()> {
        self.file.sync_data()?;
        Ok(())
    }

    fn size(&self) -> io::Result<u64> {
        let meta = self.file.metadata()?;
        Ok(meta.len())
    }
}

/// Server implements the NBD protocol, with a single export.
#[derive(Debug)]
pub struct Server {
    export: Export,
}

impl Server {
    /// Create a Server for export
    pub fn new(export: Export) -> Self {
        Self { export }
    }

    // agree on basic negotiation flags (only fixed newstyle is supported so
    // this returns a unit)
    fn initial_handshake<IO: Read + Write>(mut stream: IO) -> Result<HandshakeFlags> {
        stream.write_u64::<BE>(MAGIC)?;
        stream.write_u64::<BE>(IHAVEOPT)?;
        stream
            .write_u16::<BE>((HandshakeFlags::FIXED_NEWSTYLE | HandshakeFlags::NO_ZEROES).bits())?;
        let client_flags = stream.read_u32::<BE>()?;
        let client_flags = ClientHandshakeFlags::from_bits(client_flags).ok_or_else(|| {
            ProtocolError::new(format!("unexpected client flags {}", client_flags))
        })?;
        if !client_flags.contains(ClientHandshakeFlags::C_FIXED_NEWSTYLE) {
            bail!(ProtocolError::new("client does not support FIXED_NEWSTYLE"));
        }
        let mut flags = HandshakeFlags::FIXED_NEWSTYLE;
        if client_flags.contains(ClientHandshakeFlags::C_NO_ZEROES) {
            flags |= HandshakeFlags::NO_ZEROES;
        }
        Ok(flags)
    }

    /// reply to a OptType::LIST option request
    fn send_export_list<IO: Write>(&self, mut stream: IO) -> Result<()> {
        // Return zero or more NBD_REP_SERVER replies, one for each export,
        // followed by NBD_REP_ACK or an error (such as NBD_REP_ERR_SHUTDOWN).
        // The server MAY omit entries from this list if TLS has not been
        // negotiated, the server is operating in SELECTIVETLS mode, and the
        // entry concerned is a TLS-only export.
        let mut data = vec![];
        data.write_u32::<BE>(self.export.name.len() as u32)?;
        data.write_all(self.export.name.as_bytes())?;
        OptReply::new(OptType::LIST, ReplyType::SERVER, data).put(&mut stream)?;
        OptReply::ack(OptType::LIST).put(&mut stream)?;
        Ok(())
    }

    /// send export info at the end of newstyle negotiation, when client sends NBD_OPT_EXPORT_NAME
    fn send_export_info<IO: Write>(&self, mut stream: IO, flags: HandshakeFlags) -> Result<()> {
        // If the value of the option field is `NBD_OPT_EXPORT_NAME` and the
        // server is willing to allow the export, the server replies with
        // information about the used export:
        //
        // S: 64 bits, size of the export in bytes (unsigned)
        // S: 16 bits, transmission flags
        // S: 124 bytes, zeroes (reserved) (unless `NBD_FLAG_C_NO_ZEROES` was negotiated by the client)
        stream.write_u64::<BE>(self.export.size()?)?;
        let transmit = TransmitFlags::HAS_FLAGS | TransmitFlags::SEND_FLUSH;
        stream.write_u16::<BE>(transmit.bits())?;
        if !flags.contains(HandshakeFlags::NO_ZEROES) {
            stream.write_all(&[0u8; 124])?;
        }
        stream.flush()?;
        Ok(())
    }

    /// After the initial handshake, "haggle" to agree on connection parameters.
    //
    /// If this returns Ok(None), then the client wants to disconnect
    fn handshake_haggle<IO: Read + Write>(
        &self,
        mut stream: IO,
        flags: HandshakeFlags,
    ) -> Result<Option<&Export>> {
        loop {
            let opt = Opt::get(&mut stream)?;
            match opt.typ {
                OptType::EXPORT_NAME => {
                    let _export: String = String::from_utf8(opt.data)
                        .wrap_err(ProtocolError::new("non-UTF8 export name"))?;
                    // requested export name is currently ignored since there is
                    // only a single export
                    self.send_export_info(&mut stream, flags)?;
                    return Ok(Some(&self.export));
                }
                OptType::LIST => {
                    self.send_export_list(&mut stream)?;
                }
                OptType::ABORT => {
                    return Ok(None);
                }
                _ => {
                    warn!("got unsupported option {:?}", opt);
                    OptReply::new(opt.typ, ReplyType::ERR_UNSUP, vec![]).put(&mut stream)?;
                }
            }
        }
    }

    fn handle_ops<IO: Read + Write>(export: &Export, mut stream: IO) -> Result<()> {
        let mut req_buf = vec![0u8; 4096 * 32];
        loop {
            let req = Request::get(&mut stream, &mut req_buf)?;
            info!(target: "nbd", "{:?}", req);
            match req.typ {
                Cmd::READ => match export.read(req.offset, req.len, &mut req_buf) {
                    Ok(_) => SimpleReply::data(&req, &req_buf).put(&mut stream)?,
                    Err(err) => {
                        warn!(target: "nbd", "read error {:?}", err);
                        SimpleReply::err(err, &req).put(&mut stream)?;
                    }
                },
                Cmd::WRITE => {
                    let data = &req_buf[..req.data_len];
                    if req.len as usize > data.len() {
                        SimpleReply::err(ErrorType::EOVERFLOW, &req).put(&mut stream)?;
                        return Ok(());
                    }
                    match export.write(req.offset, data) {
                        Ok(_) => SimpleReply::ok(&req).put(&mut stream)?,
                        Err(err) => {
                            warn!(target: "nbd", "write error {:?}", err);
                            SimpleReply::err(err, &req).put(&mut stream)?;
                        }
                    }
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

    fn client<IO: Read + Write>(&self, mut stream: IO) -> Result<()> {
        let flags = Self::initial_handshake(&mut stream).wrap_err("initial handshake failed")?;
        info!("handshake with {:?}", flags);
        if let Some(export) = self
            .handshake_haggle(&mut stream, flags)
            .wrap_err("handshake haggling failed")?
        {
            info!("handshake finished");
            Server::handle_ops(export, &mut stream).wrap_err("handling client operations")?;
        }
        Ok(())
    }

    /// Start accepting connections from clients and processing commands.
    ///
    /// Currently accepts in a single thread, so only one client can be
    /// connected at a time.
    pub fn start(self) -> Result<()> {
        let addr = ("127.0.0.1", TCP_PORT);
        let listener = TcpListener::bind(addr)?;
        for stream in listener.incoming() {
            let stream = stream?;
            stream.set_nodelay(true)?;
            info!(target: "nbd", "client connected");
            // TODO: how to process clients in parallel? self has to be shared among threads
            match self.client(stream) {
                Ok(_) => info!(target: "nbd", "client disconnected"),
                Err(err) => eprintln!("error handling client:\n{}", err),
            }
        }
        Ok(())
    }
}
