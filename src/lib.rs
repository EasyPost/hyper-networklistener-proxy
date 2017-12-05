//! Implementation of the [PROXY protocol](https://www.haproxy.org/download/1.8/doc/proxy-protocol.txt)
//! for the pre-async versions of `hyper` such as those used by `iron`.
//!
//! # Example
//!
//! Wrapping an HTTP listener so that it will expect the PROXY protocol v2
//!
//! ```no_run
//! use hyper_networklistener_proxy::{ProxyListener, ProxyProtocolVersion};
//! use hyper::net::HttpListener;
//!
//! let listener = ProxyListener(
//!     HttpListener::new("127.0.0.1:8080").unwrap(),
//!     ProxyProtocolVersion::V2
//! );
//! ```

extern crate hyper;
extern crate byteorder;

mod proxy_stream;
pub mod proxy_listener;
pub mod proxy_protocol;

pub use proxy_listener::ProxyListener;
pub use proxy_protocol::ProxyProtocolVersion;
