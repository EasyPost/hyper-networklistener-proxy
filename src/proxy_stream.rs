use std::net::{SocketAddr,Shutdown};
use std::io::{self,Read,Write};
use std::time::Duration;

use hyper;
use hyper::net::NetworkStream;

use proxy_protocol::ProxyProtocolVersion;
use proxy_protocol::read_proxy_protocol_v1;
use proxy_protocol::read_proxy_protocol_v2;


#[derive(Clone)]
pub struct ProxyStream<T: NetworkStream+Clone> {
    inner: T,
    peer_addr: SocketAddr
}




impl<T: NetworkStream+Read+Write+Clone> ProxyStream<T> {
    pub(crate) fn from_stream(mut stream: T, v: ProxyProtocolVersion) -> hyper::Result<Self> {
        let peer_addr_result = match v {
            ProxyProtocolVersion::V1 => read_proxy_protocol_v1(&mut stream),
            ProxyProtocolVersion::V2 => read_proxy_protocol_v2(&mut stream),
            ProxyProtocolVersion::Any => unimplemented!(),
        };
        unimplemented!();
        /*
        Ok(ProxyStream {
            inner: stream,
            peer_addr: peer_addr_result?.source_addr
        })
        */
    }
}

impl<T: NetworkStream+Read+Write+Clone> NetworkStream for ProxyStream<T> {
    fn peer_addr(&mut self) -> io::Result<SocketAddr> {
        Ok(self.peer_addr)
    }

    fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_read_timeout(dur)
    }

    fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_write_timeout(dur)
    }

    fn close(&mut self, how: Shutdown) -> io::Result<()> {
        self.inner.close(how)
    }
}

impl<T: NetworkStream+Read+Write+Clone> Read for ProxyStream<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<T: NetworkStream+Write+Clone> Write for ProxyStream<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
