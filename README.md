This is implementation of the [NetworkListener](https://docs.rs/hyper/0.10.13/hyper/net/trait.NetworkListener.html) trait from the pre-asyncio Hyper for the [PROXY protocol](https://www.haproxy.org/download/1.8/doc/proxy-protocol.txt), commonly used by load balancers. It allows wrapping any other NetworkListener and will read the PROXY protocol bits immediately after calling the wrapped Accept.

This is intended for use with [Iron](https://github.com/iron/iron).

An example can be seen at [`examples/time_server.rs`](examples/time_server.rs); you can build and run it with `cargo run --example time_server -- -B 127.0.0.1:8000`.
