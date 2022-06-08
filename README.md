# rust-nbd

[![CI](https://github.com/tchajed/rust-nbd/actions/workflows/build.yml/badge.svg)](https://github.com/tchajed/rust-nbd/actions/workflows/build.yml)

Implementation of a basic Network Block Device (NBD) server in Rust. NBD exports
a block device over the network, with commands for reading and writing blocks of
the device by offset. The kernel natively supports NBD, but you'll probably need
to install the userspace utilities to use it.

```
$ cargo run --release &
$ sudo modprobe nbd
$ sudo nbd-client localhost -N default /dev/nbd0
```

Now we can interact with /dev/nbd0, for example creating an ext4 file system
backed by the remote server:

```
$ sudo chown $USER /dev/nbd0
$ mkfs -t ext4 /dev/nbd0
$ mkdir /mnt/nbd
$ sudo mount /dev/nbd0 /mnt/nbd
```
