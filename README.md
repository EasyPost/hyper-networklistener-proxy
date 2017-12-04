This is implementation of the [NetworkListener](https://docs.rs/hyper/0.10.13/hyper/net/trait.NetworkListener.html) trait from the pre-asyncio Hyper for the [PROXY protocol](https://www.haproxy.org/download/1.8/doc/proxy-protocol.txt), commonly used by load balancers. It allows wrapping any other NetworkListener and will read the PROXY protocol bits immediately after calling the wrapped Accept.

This is intended for use with [Iron](http://ironframework.io/).
