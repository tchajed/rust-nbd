# rust-nbd

[![CI](https://github.com/tchajed/rust-nbd/actions/workflows/build.yml/badge.svg)](https://github.com/tchajed/rust-nbd/actions/workflows/build.yml)

Implementation of a basic Network Block Device (NBD) server in Rust. NBD exports
a block device over the network, with commands for reading and writing blocks of
the device by offset. The kernel natively supports NBD, but you do need `sudo
modprobe nbd` to initialize the module. This library provides a Rust
implementation of the necessary userspace libraries, so you don't need to
install `nbd-client`.

```
$ cargo run --release -- --size 1000 disk.img &
$ sudo modprobe nbd
$ cargo run --release --bin client -- /dev/nbd0
```

Now we can interact with `/dev/nbd0` as with any other block device, for example
creating an ext4 file system backed by the remote server:

```
$ sudo chown $USER /dev/nbd0
$ mkfs -t ext4 /dev/nbd0
$ mkdir /mnt/nbd
$ sudo mount /dev/nbd0 /mnt/nbd
```

Finally, make sure to disconnect before running again:

```
$ cargo run --release --bin client -- --disconnect /dev/nbd0
```
