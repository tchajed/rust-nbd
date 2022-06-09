//! Library for interacting with the kernel component of NBD, using ioctls.

#![deny(missing_docs)]

/// Wrappers for NBD ioctls.
///
/// See <https://github.com/NetworkBlockDevice/nbd/blob/master/nbd.h>.
use std::{
    fs::File,
    io,
    os::unix::prelude::{AsRawFd, RawFd},
};

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
pub fn set_sock(f: &File, sock: RawFd) -> io::Result<()> {
    let fd = f.as_raw_fd();
    unsafe { ioctl::set_sock(fd, sock as i32)? };
    Ok(())
}

/// Send DO_IT (not entirely sure what this does...)
pub fn do_it(f: &File) -> io::Result<()> {
    let fd = f.as_raw_fd();
    unsafe { ioctl::do_it(fd)? };
    Ok(())
}

/// Set desired block size for an NBD device opened at `f`.
pub fn set_blksize(f: &File, blksize: u64) -> io::Result<()> {
    let fd = f.as_raw_fd();
    unsafe { ioctl::set_blksize(fd, blksize as i32)? };
    Ok(())
}

/// Set size in bytes for an NBD device opened at `f`.
pub fn set_size(f: &File, bytes: u64) -> io::Result<()> {
    let fd = f.as_raw_fd();
    unsafe { ioctl::set_size(fd, bytes as i32)? };
    Ok(())
}

/// Set size in blocks for an NBD device opened at `f`.
pub fn set_size_blocks(f: &File, blocks: u64) -> io::Result<()> {
    let fd = f.as_raw_fd();
    unsafe { ioctl::set_size_blocks(fd, blocks as i32)? };
    Ok(())
}

/// Clear the socket previously set for NBD device `f`.
pub fn clear_sock(f: &File) -> io::Result<()> {
    let fd = f.as_raw_fd();
    unsafe { ioctl::clear_sock(fd)? };
    Ok(())
}

/// Disconnect from the remote for NBD device `f`.
pub fn disconnect(f: &File) -> io::Result<()> {
    let fd = f.as_raw_fd();
    unsafe { ioctl::disconnect(fd)? };
    Ok(())
}
