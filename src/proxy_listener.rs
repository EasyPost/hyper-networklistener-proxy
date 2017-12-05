use std::io;
use std::net::SocketAddr;

use hyper;
use hyper::net::NetworkListener;

use proxy_protocol::ProxyProtocolVersion;
use proxy_stream::ProxyStream;


#[derive(Clone)]
/// An implementation of `NetworkListener` which reads the PROXY protocol (version specified
/// by the `version` argument) after calling the `accept()` function from the container
/// sub-listener
pub struct ProxyListener<T: Clone> {
    inner: T,
    version: ProxyProtocolVersion,
}

impl<T: NetworkListener+Clone> ProxyListener<T> {
    /// Construct a new `ProxyListener` from an already-construced listener (e.g.,
    /// `hyper::net::HttpListener`)
    pub fn new(listener: T, proxy_protocol_version: ProxyProtocolVersion) -> Self {
        ProxyListener {
            inner: listener,
            version: proxy_protocol_version
        }
    }
}


impl<T: NetworkListener+Clone> NetworkListener for ProxyListener<T> {
    type Stream = ProxyStream<T::Stream>;

    /// Accept a single connection from this Listener
    fn accept(&mut self) -> hyper::Result<Self::Stream> {
        let stream = self.inner.accept()?;
        ProxyStream::from_stream(stream, self.version)
    }

    /// Find out the local address we are bound to
    fn local_addr(&mut self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}


#[cfg(unix)]
impl<T> ::std::os::unix::io::AsRawFd for ProxyListener<T>
    where T: NetworkListener + Clone + ::std::os::unix::io::AsRawFd {
    fn as_raw_fd(&self) -> ::std::os::unix::io::RawFd {
        self.inner.as_raw_fd()
    }
}


#[cfg(test)]
mod tests {
    use hyper::net::{HttpListener, NetworkListener, NetworkStream};
    use super::{ProxyListener, ProxyProtocolVersion};
    use std::thread;
    use std::sync::{Arc,Barrier,Mutex};
    use std::net::{SocketAddr, TcpStream, Shutdown};
    use std::io::{Write,Read};

    #[derive(Debug, PartialEq, Eq)]
    struct BasicResult {
        addr: SocketAddr,
        body: String,
    }

    #[test]
    fn test_basic() {
        let start_barrier = Arc::new(Barrier::new(2));
        let finished_barrier = Arc::new(Barrier::new(2));
        let port: Arc<Mutex<Option<SocketAddr>>> = Arc::new(Mutex::new(None));
        let result: Arc<Mutex<Option<BasicResult>>> = Arc::new(Mutex::new(None));

        let handle = {
            let start_barrier = Arc::clone(&start_barrier);
            let finished_barrier = Arc::clone(&finished_barrier);
            let port = Arc::clone(&port);
            let result = Arc::clone(&result);
            thread::spawn(move|| {
                let inner = HttpListener::new("127.0.0.1:0").expect("should be able to bind");
                let mut outer = ProxyListener::new(inner, ProxyProtocolVersion::V1);
                let addr = outer.local_addr().expect("should be able to find local addr");
                {
                    let mut pl = port.lock().unwrap();
                    *pl = Some(addr);
                }
                start_barrier.wait();

                let mut conn = outer.accept().expect("should be able to accept a connection");
                let peer_addr = conn.peer_addr().expect("should be able to call .peer_addr()");
                let mut target = String::new();
                conn.read_to_string(&mut target).expect("body read should succeed");

                {
                    let mut pl = result.lock().unwrap();
                    *pl = Some(BasicResult {
                        addr: peer_addr,
                        body: target
                    });
                }

                finished_barrier.wait();
            })
        };

        start_barrier.wait();

        let port = (*port.lock().unwrap()).expect("should be some");

        {
            let mut conn = TcpStream::connect(port).expect("should be able to connect");
            write!(&mut conn, "PROXY TCP4 127.0.0.1 127.0.0.2 2020 3030\r\n").expect("write must succeed");
            write!(&mut conn, "GET / HTTP/1.1\r\n\r\n").expect("write must succeed");
            conn.shutdown(Shutdown::Both).expect("this can't even fail");
        }

        finished_barrier.wait();

        let result = (*result.lock().unwrap()).take().expect("should be some result");

        assert_eq!(result, BasicResult {
            addr: "127.0.0.1:2020".parse().unwrap(),
            body: "GET / HTTP/1.1\r\n\r\n".to_string()
        });

        handle.join().expect("must be able to join thread")
    }
}
