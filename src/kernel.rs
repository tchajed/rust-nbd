//! Library for interacting with the kernel component of NBD, using ioctls.
//!
//! The setup implemented here is normally carried out by `nbd-client` on Linux.
//! The
//! [nbd-client.c](https://github.com/NetworkBlockDevice/nbd/blob/master/nbd-client.c)
//! source code was helpful for developing this library.
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

use std::io::{self, prelude::*};
use std::{
    fs::File,
    os::unix::io::{AsRawFd, IntoRawFd, RawFd},
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

/// Set up NBD device file to connect to a connected client.
///
/// `nbd` should be an open NBD device file (eg, /dev/nbd0).
///
/// Client must use an underlying connection which is based on a raw file
/// descriptor, since this is what is sent to the kernel. In practice a
/// `TcpStream` is likely to be this connection, but it could also be a socket
/// to an in-process server.
///
/// The protocol here is probably best reverse-engineered by running an NBD
/// server (`cargo run` will work), then running `strace` over `nbd-client` (run
/// this with `sudo -i` - you don't want to run `strace` on `sudo`, it's more
/// confusing)
///
/// `strace nbd-client -N default -nonetlink localhost /dev/nbd0 1>/dev/null`
///
/// ```txt
/// connect(4, {sa_family=AF_INET, sin_port=htons(10809), sin_addr=inet_addr("127.0.0.1")}, 16) = 0
/// setsockopt(4, SOL_TCP, TCP_NODELAY, [1], 4) = 0
/// write(1, "Negotiation: ", 13)           = 13
/// read(4, "NBDMAGIC", 8)                  = 8
/// write(1, ".", 1)                        = 1
/// read(4, "IHAVEOPT", 8)                  = 8
/// write(1, ".", 1)                        = 1
/// read(4, "\0\3", 2)                      = 2
/// write(4, "\0\0\0\3", 4)                 = 4
/// write(4, "IHAVEOPT\0\0\0\7\0\0\0\r", 16) = 16
/// write(4, "\0\0\0\7", 4)                 = 4
/// write(4, "default", 7)                  = 7
/// write(4, "\0\0", 2)                     = 2
/// read(4, "\0\3\350\211\4Ue\251\0\0\0\7\0\0\0\3\0\0\0\f", 20) = 20
/// read(4, "\0\0\0\0\0\0\0\240\0\0\0\r", 12) = 12
/// write(1, "size = 10MB", 11)             = 11
/// write(1, "\n", 1)                       = 1
/// read(4, "\0\3\350\211\4Ue\251\0\0\0\7\0\0\0\1\0\0\0\0", 20) = 20
/// ioctl(3, NBD_SET_BLKSIZE, 512)          = 0
/// ioctl(3, NBD_SET_SIZE_BLOCKS, 20480)    = 0
/// write(2, "bs=512, sz=10485760 bytes\n", 26bs=512, sz=10485760 bytes
/// ) = 26
/// ioctl(3, NBD_CLEAR_SOCK)                = 0
/// ioctl(3, NBD_SET_FLAGS, NBD_FLAG_HAS_FLAGS|NBD_FLAG_SEND_FLUSH|NBD_FLAG_SEND_FUA) = 0
/// ioctl(3, BLKROSET, [0])                 = 0
/// ioctl(3, NBD_SET_SOCK, 4)               = 0
/// clone(child_stack=NULL, flags=CLONE_CHILD_CLEARTID|CLONE_CHILD_SETTID|SIGCHLD, child_tidptr=0x7f78b1811c10) = 25208
/// ```
///
/// You can see that fd 4 is a connection to the server, there's a bunch of
/// traffic for the initial negotiation, and then the `ioctl(3, ...)` are the
/// interesting part. From earlier in the trace, we can see fd 3 is `/dev/nbd0`.
/// Then we see a handful of configuration ioctl calls followed by `ioctl(3,
/// NBD_SET_SOCK, 4)`, which is the really important part. Then the process
/// calls `clone` to keep running in the background.
pub fn set_client<IO: Read + Write + IntoRawFd>(nbd: &File, client: Client<IO>) -> Result<()> {
    let size = client.size();
    set_blksize(nbd, 4096)?;
    set_size_blocks(nbd, size / 4096)?;

    let flags = TransmitFlags::HAS_FLAGS | TransmitFlags::SEND_FLUSH;
    set_flags(nbd, flags)?;

    clear_sock(nbd)?;

    let sock = client.into_raw_fd();
    set_sock(nbd, sock).wrap_err("could not set nbd sock")?;
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
///
/// Similar to [`set_client`], we can investigate this with strace:
///
/// `strace nbd-client -d -nonetlink /dev/nbd0 1>/dev/null`
///
/// ```txt
/// openat(AT_FDCWD, "/dev/nbd0", O_RDWR)   = 3
/// write(1, "disconnect, ", 12)            = 12
/// ioctl(3, NBD_DISCONNECT)                = 0
/// write(1, "sock, ", 6)                   = 6
/// ioctl(3, NBD_CLEAR_SOCK)                = 0
/// write(1, "done", 4)                     = 4
/// write(1, "\n", 1)                       = 1
/// close(3)                                = 0
/// ```
pub fn close(nbd: &File) -> Result<()> {
    disconnect(nbd).wrap_err("could not disconnect")?;
    clear_sock(nbd).wrap_err("could not clear socket")?;

    Ok(())
}
