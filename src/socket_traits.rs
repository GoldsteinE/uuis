use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    os::unix::{
        self,
        net::{UnixListener, UnixStream},
    },
};

pub trait Listener {
    type Stream;
    type SocketAddr;

    fn accept(&self) -> io::Result<(Self::Stream, Self::SocketAddr)>;
}

impl Listener for TcpListener {
    type Stream = TcpStream;
    type SocketAddr = SocketAddr;

    fn accept(&self) -> io::Result<(Self::Stream, Self::SocketAddr)> {
        self.accept()
    }
}

impl Listener for UnixListener {
    type Stream = UnixStream;
    type SocketAddr = unix::net::SocketAddr;

    fn accept(&self) -> io::Result<(Self::Stream, Self::SocketAddr)> {
        self.accept()
    }
}

pub trait NetStream: Read + Write + Sized {
    fn try_clone(&self) -> io::Result<Self>;
}

impl NetStream for TcpStream {
    fn try_clone(&self) -> io::Result<Self> {
        self.try_clone()
    }
}

impl NetStream for UnixStream {
    fn try_clone(&self) -> io::Result<Self> {
        self.try_clone()
    }
}
