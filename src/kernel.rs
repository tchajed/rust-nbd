//! Library for interacting with the kernel component of NBD, using ioctls.
//!
//! The way the NBD API works is that loading the kernel module initializes the
//! devices `/dev/nbd0`, `/dev/nbd1`, and so on, as special files. Then the
//! kernel has a way of associating these files with remote servers; from then
//! on, block device operations on the NBD devices are translated into network
//! operations interacting with the remote end, following the protocol (the
//! kernel is the _client_ in these interactions).
//!
//! In order to set up a device with a remote server, the kernel uses `ioctl()`,
//! a general-purpose system call for interacting with drivers through special
//! files. NBD exposes two key `ioctl()`s: `NBD_SET_SOCK` passes a socket (as a
//! file descriptor) to the kernel, associating it to the NBD device, and
//! `NBD_CLEAR_SOCK` resets this state. There are a handful of other
//! configuration commands, but these are the most important.
//!
//! Note that the kernel implements only the transmission phase of the NBD
//! protocol and it is the job of the userspace process to create the socket
//! (e.g., create a TCP connection connected to a remote NBD server) and
//! negotiate with the remote end.

#![deny(missing_docs)]

use color_eyre::eyre::WrapErr;
use color_eyre::Result;

use std::io::prelude::*;
use std::{
    fs::File,
    io,
    os::unix::prelude::{AsRawFd, IntoRawFd, RawFd},
};

use crate::{client::Client, proto::TransmitFlags};

/// Wrappers for NBD ioctls.
///
/// See <https://github.com/NetworkBlockDevice/nbd/blob/master/nbd.h>.
mod ioctl {
    use nix::{ioctl_none_bad, ioctl_write_int_bad, request_code_none};
    const NBD_IOCTL: u8 = 0xAB;
    ioctl_write_int_bad!(set_sock, request_code_none!(NBD_IOCTL, 0));
    ioctl_write_int_bad!(set_blksize, request_code_none!(NBD_IOCTL, 1));
    ioctl_write_int_bad!(set_size, request_code_none!(NBD_IOCTL, 2));
    ioctl_none_bad!(do_it, request_code_none!(NBD_IOCTL, 3));
    ioctl_none_bad!(clear_sock, request_code_none!(NBD_IOCTL, 4));
    // deprecated
    // ioctl_none_bad!(clear_que, request_code_none!(NBD_IOCTL, 5));
    // ioctl_none_bad!(print_debug, request_code_none!(NBD_IOCTL, 6));
    ioctl_write_int_bad!(set_size_blocks, request_code_none!(NBD_IOCTL, 7));
    ioctl_none_bad!(disconnect, request_code_none!(NBD_IOCTL, 8));
    ioctl_write_int_bad!(set_timeout, request_code_none!(NBD_IOCTL, 9));
    ioctl_write_int_bad!(set_flags, request_code_none!(NBD_IOCTL, 10));
}

/// Set socket for an NBD device opened at `f`. Should be connected to an NBD server.
fn set_sock(f: &File, sock: RawFd) -> io::Result<()> {
    let fd = f.as_raw_fd();
    unsafe { ioctl::set_sock(fd, sock as i32)? };
    Ok(())
}

/// Send DO_IT (not entirely sure what this does...)
fn do_it(f: &File) -> io::Result<()> {
    let fd = f.as_raw_fd();
    unsafe { ioctl::do_it(fd)? };
    Ok(())
}

/// Set desired block size for an NBD device opened at `f`.
fn set_blksize(f: &File, blksize: u64) -> io::Result<()> {
    let fd = f.as_raw_fd();
    unsafe { ioctl::set_blksize(fd, blksize as i32)? };
    Ok(())
}

/// Set size in bytes for an NBD device opened at `f`.
#[allow(dead_code)]
fn set_size(f: &File, bytes: u64) -> io::Result<()> {
    let fd = f.as_raw_fd();
    unsafe { ioctl::set_size(fd, bytes as i32)? };
    Ok(())
}

/// Set size in blocks for an NBD device opened at `f`.
fn set_size_blocks(f: &File, blocks: u64) -> io::Result<()> {
    let fd = f.as_raw_fd();
    unsafe { ioctl::set_size_blocks(fd, blocks as i32)? };
    Ok(())
}

/// Clear the socket previously set for NBD device `f`.
fn clear_sock(f: &File) -> io::Result<()> {
    let fd = f.as_raw_fd();
    unsafe { ioctl::clear_sock(fd)? };
    Ok(())
}

/// Disconnect from the remote for NBD device `f`.
fn disconnect(f: &File) -> io::Result<()> {
    let fd = f.as_raw_fd();
    unsafe { ioctl::disconnect(fd)? };
    Ok(())
}

/// Set flags for the NBD device `f`, using the client's required flags.
fn set_flags(f: &File, flags: TransmitFlags) -> io::Result<()> {
    let fd = f.as_raw_fd();
    unsafe { ioctl::set_flags(fd, flags.bits() as i32)? };
    Ok(())
}

/// Set up NBD device open at `nbd` to connect to `client`.
///
/// Client must use an underlying connection which is based on a raw file
/// descriptor, since this is what is sent to the kernel. In practice a
/// `TcpStream` is likely to be this connection, but it could also be a socket
/// to an in-process server.
pub fn set_client<IO: Read + Write + IntoRawFd>(nbd: &File, client: Client<IO>) -> Result<()> {
    let size = client.size();
    set_blksize(&nbd, 4096)?;
    set_size_blocks(&nbd, size / 4096)?;

    let flags = TransmitFlags::HAS_FLAGS | TransmitFlags::SEND_FLUSH;
    set_flags(&nbd, flags)?;

    clear_sock(&nbd)?;

    let sock = client.into_raw_fd();
    set_sock(&nbd, sock).wrap_err("could not set nbd sock")?;
    Ok(())
}

/// Wait for an initialized NBD device to be closed.
pub fn wait(nbd: &File) -> Result<()> {
    do_it(nbd).wrap_err("waiting for NBD with DO_IT ioctl")?;
    Ok(())
}

/// Close an initialized NBD device, terminating the connection with the client.
///
/// Does not signal if there was an existing connection or not.
pub fn close(nbd: &File) -> Result<()> {
    clear_sock(&nbd).wrap_err("could not clear socket")?;
    disconnect(&nbd).wrap_err("could not disconnect")?;

    Ok(())
}
