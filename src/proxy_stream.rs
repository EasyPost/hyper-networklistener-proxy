use std::net::{SocketAddr,Shutdown};
use std::io::{self,Read,Write};
use std::time::Duration;

use hyper;
use hyper::net::NetworkStream;

use proxy_protocol::{ProxyProtocolVersion, ProxyProtocolHeader};
use proxy_protocol::read_proxy_protocol_v1;
use proxy_protocol::read_proxy_protocol_v2;
use proxy_protocol::read_proxy_protocol_any;


/// Wrapper class for holding a `NetworkStream` off of which we have already
/// read a PROXY protocol header
#[derive(Clone, Debug)]
pub struct ProxyStream<T: NetworkStream> {
    inner: T,
    peer_addr: Option<SocketAddr>
}

impl<T: NetworkStream> ProxyStream<T> {
    pub(crate) fn from_stream(mut stream: T, v: ProxyProtocolVersion) -> hyper::Result<Self> {
        // XXX: should we be setting a read timeout here?
        // HttpListener sets the timeout in its `accept`, so it should be fine,
        // but other listeners might not set the timeout until after accept...
        let proxy_header: hyper::Result<ProxyProtocolHeader> = match v {
            ProxyProtocolVersion::V1 => read_proxy_protocol_v1(&mut stream),
            ProxyProtocolVersion::V2 => read_proxy_protocol_v2(&mut stream),
            ProxyProtocolVersion::Any => read_proxy_protocol_any(&mut stream),
        }.map_err(|e| e.into());
        Ok(ProxyStream {
            peer_addr: proxy_header?.source_addr(),
            inner: stream,
        })
    }
}

impl<T: NetworkStream> NetworkStream for ProxyStream<T> {
    fn peer_addr(&mut self) -> io::Result<SocketAddr> {
        if let Some(a) = self.peer_addr {
            Ok(a.clone())
        } else {
            self.inner.peer_addr()
        }
    }

    #[inline]
    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_read_timeout(dur)
    }

    #[inline]
    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_write_timeout(dur)
    }

    #[inline]
    fn close(&mut self, how: Shutdown) -> io::Result<()> {
        self.inner.close(how)
    }
}

impl<T: NetworkStream> Read for ProxyStream<T> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<T: NetworkStream> Write for ProxyStream<T> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(unix)]
impl<T: NetworkStream+::std::os::unix::io::AsRawFd> ::std::os::unix::io::AsRawFd for ProxyStream<T> {
    #[inline]
    fn as_raw_fd(&self) -> ::std::os::unix::io::RawFd {
        self.inner.as_raw_fd()
    }
}
