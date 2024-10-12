# rust-nbd

[![CI](https://github.com/tchajed/rust-nbd/actions/workflows/build.yml/badge.svg)](https://github.com/tchajed/rust-nbd/actions/workflows/build.yml)

Implementation of a basic Network Block Device (NBD) server and client written
in Rust.  NBD is a Linux kernel feature that exports a block device over the
network, with commands for reading and writing blocks of the device by offset.
The kernel natively supports NBD, but you do need `sudo modprobe nbd` to
initialize the kernel module.

This code implements:
- Rust modules that implement the client and server parts of the NBD protocol.
- A userspace NBD server that is compatible with Linux.
- A Rust re-implementation of the `nbd-client` utility (from the [standard userland tools](https://github.com/NetworkBlockDevice/nbd)). This avoids needing to install anything extra to use NBD.

All of the interactions with the kernel are very Linux-specific.

macOS does not provide an nbd kernel component, but it can run the server.
There is also a Rust library to interact with the server that would work if you
wanted to use nbd from userspace.

Here's a quick demo of running the server and connecting with the client:

```
$ cargo run --release -- --size 1000 disk.img &
$ sudo modprobe nbd
$ cargo run --bin client -- /dev/nbd0
```

The client automatically escalates to root with `sudo` in order to have the
necessary privilege to set up the block device.  Now we can interact with
`/dev/nbd0` as with any other block device, for example with `dd` (more
interestingly, you can use `mkfs.ext` to create a file system there and then
`mount` it):

```
$ sudo chown $USER /dev/nbd0
$ dd if=/dev/zero of=/dev/nbd0 bs=4096
```

Finally, make sure to disconnect before running again:

```
$ cargo run --bin client -- --disconnect /dev/nbd0
```
