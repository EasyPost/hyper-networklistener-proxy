use std::io;
use std::net::SocketAddr;

use hyper;
use hyper::net::NetworkListener;

use proxy_protocol::ProxyProtocolVersion;
use proxy_stream::ProxyStream;


#[derive(Clone)]
pub struct ProxyListener<T: Clone> {
    inner: T,
    version: ProxyProtocolVersion,
}

impl<T: NetworkListener+Clone> ProxyListener<T> {
    fn new(listener: T, proxy_protocol_version: ProxyProtocolVersion) -> Self {
        ProxyListener {
            inner: listener,
            version: proxy_protocol_version
        }
    }
}


impl<T: NetworkListener+Clone> NetworkListener for ProxyListener<T> {
    type Stream = ProxyStream<T::Stream>;

    fn accept(&mut self) -> hyper::Result<Self::Stream> {
        let stream = self.inner.accept()?;
        ProxyStream::from_stream(stream, self.version)
    }

    fn local_addr(&mut self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}
