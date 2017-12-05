use std::fmt::{self, Display, Debug, Formatter};
use std::error::Error;
use std::io::{self,Read};
use std::net::{SocketAddr,IpAddr,Ipv4Addr,Ipv6Addr,AddrParseError};
use std::str::Utf8Error;
use std::num::ParseIntError;

use hyper;
use byteorder::{NetworkEndian,ByteOrder};


/// Version of the PROXY protocol to look for. The `Any` option will attempt to guess between
/// V1 and V2, but does more I/O in order to do so. `V2` is significantly faster than `V1` or `Any`
/// and should be preferred whenever possible.
#[derive(Debug,Clone,PartialEq,Eq,Copy)]
pub enum ProxyProtocolVersion {
    V1,
    V2,
    Any
}


#[derive(Debug)]
pub(crate) enum ProxyReadError {
    MissingField,
    MissingLiteral,
    InvalidProtocol,
    MissingCrlf,
    MissingFirstByte,
    BadVersion,
    BadSourceAddress(AddrParseError),
    BadSourcePort(ParseIntError),
    BadDestAddress(AddrParseError),
    BadDestPort(ParseIntError),
    Io(io::Error),
    Utf8(Utf8Error),
}


impl Display for ProxyReadError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        <Self as Debug>::fmt(self, f)
    }
}

impl Error for ProxyReadError {
    fn description(&self) -> &'static str {
        "error reading PROXY protocol header on stream"
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            ProxyReadError::Io(ref err) => Some(err),
            ProxyReadError::Utf8(ref err) => Some(err),
            ProxyReadError::BadSourceAddress(ref err) => Some(err),
            ProxyReadError::BadDestAddress(ref err) => Some(err),
            _ => None,
        }
    }
}


pub(crate) type Result<T> = ::std::result::Result<T, ProxyReadError>;


impl From<io::Error> for ProxyReadError {
    fn from(e: io::Error) -> Self {
        ProxyReadError::Io(e)
    }
}


impl From<Utf8Error> for ProxyReadError {
    fn from(e: Utf8Error) -> Self {
        ProxyReadError::Utf8(e)
    }
}


impl Into<hyper::Error> for ProxyReadError {
    fn into(self) -> hyper::Error {
        match self {
            ProxyReadError::Io(e) => hyper::Error::Io(e),
            ProxyReadError::Utf8(e) => hyper::Error::Utf8(e),
            ProxyReadError::BadVersion => hyper::Error::Version,
            _ => hyper::Error::Version,
        }
    }
}


#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Proto {
    Tcp4,
    Tcp6,
    Unix,
    Unknown
}


#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ProxyProtocolHeader {
    version: u8,
    proto: Proto,
    command: Command,
    source_addr: Option<SocketAddr>,
    dest_addr: Option<SocketAddr>,
}


impl ProxyProtocolHeader {
    fn new(version: u8, proto: Proto, source_addr: SocketAddr, dest_addr: SocketAddr) -> Self {
        ProxyProtocolHeader {
            version: version,
            proto: proto,
            source_addr: Some(source_addr),
            dest_addr: Some(dest_addr),
            command: Command::Proxy
        }
    }

    fn new_with_command(version: u8, proto: Proto, command: Command, source_addr: SocketAddr, dest_addr: SocketAddr) -> Self {
        ProxyProtocolHeader {
            version: version,
            proto: proto,
            source_addr: Some(source_addr),
            dest_addr: Some(dest_addr),
            command: command
        }
    }

    fn new_unknown(version: u8) -> Self {
        ProxyProtocolHeader {
            version: version,
            proto: Proto::Unknown,
            source_addr: None,
            dest_addr: None,
            command: Command::Unspec,
        }
    }
}


impl ProxyProtocolHeader {
    pub(crate) fn source_addr(&self) -> Option<SocketAddr> {
        self.source_addr.clone()
    }
}


/// Read from a Reader into the given buffer.
fn read_to_crlf<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<usize> {
    let mut found_crlf_at = None;
    // read until we either exceed the buf or find a CRLF. SO INEFFICIENT
    for i in 0..107 {
        r.read_exact(&mut buf[i..i+1])?;
        if i > 1 {
            if buf[i-1] == 13u8 && buf[i] == 10u8 {
                found_crlf_at = Some(i-1);
                break;
            }
        }
    }
    if let Some(end_idx) = found_crlf_at {
        Ok(end_idx)
    } else {
        Err(ProxyReadError::MissingCrlf)
    }
}

fn parse_proxy_protocol_v1_after_first_byte(buf: &[u8]) -> Result<ProxyProtocolHeader> {
    let mut fields = buf.split(|&f| f == 32u8).map(|f| ::std::str::from_utf8(f));
    if fields.next() != Some(Ok("ROXY")) {
        return Err(ProxyReadError::MissingLiteral);
    }
    let proto = match fields.next().ok_or(ProxyReadError::MissingField)?? {
        "TCP4" => Proto::Tcp4,
        "TCP6" => Proto::Tcp6,
        "UNKNOWN" => Proto::Unknown,
        _ => return Err(ProxyReadError::MissingLiteral),
    };
    if proto == Proto::Unknown {
        return Ok(ProxyProtocolHeader::new_unknown(1));
    }
    let source_address: IpAddr = fields.next().ok_or(ProxyReadError::MissingField)??.parse().map_err(ProxyReadError::BadSourceAddress)?;
    let dest_address: IpAddr = fields.next().ok_or(ProxyReadError::MissingField)??.parse().map_err(ProxyReadError::BadDestAddress)?;
    let source_port: u16 = fields.next().ok_or(ProxyReadError::MissingField)??.parse().map_err(ProxyReadError::BadSourcePort)?;
    let dest_port: u16 = fields.next().ok_or(ProxyReadError::MissingField)??.parse().map_err(ProxyReadError::BadDestPort)?;
    Ok(ProxyProtocolHeader::new(1, proto, SocketAddr::new(source_address, source_port), SocketAddr::new(dest_address, dest_port)))
}

pub(crate) fn read_proxy_protocol_v1<R: Read>(r: &mut R) -> Result<ProxyProtocolHeader> {
    // this is the longest that the PROXY header can be
    let mut buf = [0u8; 107];
    let buf_len = read_to_crlf(r, &mut buf)?;
    if buf[0] != 0x50 { // P as in P-ROXY
        return Err(ProxyReadError::MissingLiteral);
    }
    parse_proxy_protocol_v1_after_first_byte(&buf[1..buf_len])
}


#[derive(Debug,PartialEq,Eq)]
pub(crate) enum Command {
    Local,
    Proxy,
    Unspec,
}

#[derive(Debug,PartialEq,Eq)]
enum AddressFamily {
    Unspec,
    Inet,
    Inet6,
    Unix
}


#[derive(Debug,PartialEq,Eq)]
enum TransportFamily {
    Unspec,
    Stream,
    Dgram
}


fn slice_to_ipv6addr(slice: &[u8]) -> Ipv6Addr {
    let o1 = NetworkEndian::read_u16(&slice[0..2]);
    let o2 = NetworkEndian::read_u16(&slice[2..4]);
    let o3 = NetworkEndian::read_u16(&slice[4..6]);
    let o4 = NetworkEndian::read_u16(&slice[6..8]);
    let o5 = NetworkEndian::read_u16(&slice[8..12]);
    let o6 = NetworkEndian::read_u16(&slice[8..12]);
    let o7 = NetworkEndian::read_u16(&slice[12..14]);
    let o8 = NetworkEndian::read_u16(&slice[14..16]);
    Ipv6Addr::new(o1, o2, o3, o4, o5, o6, o7, o8)
}


fn read_proxy_protocol_v2_after_first_byte<R: Read>(r: &mut R, header_buf_already_read: &[u8]) -> Result<ProxyProtocolHeader> {
    let mut header_buf = [0u8;16];
    let bytes_read = header_buf_already_read.len();
    if bytes_read < 16 {
        r.read_exact(&mut header_buf[bytes_read..])?;
    }
    header_buf[0..bytes_read].copy_from_slice(header_buf_already_read);
    if &header_buf[0..12] != b"\x0D\x0A\x0D\x0A\x00\x0D\x0A\x51\x55\x49\x54\x0A" {
        return Err(ProxyReadError::MissingLiteral);
    }
    let protocol_version = (header_buf[12] & 0xf0) >> 4;
    if protocol_version != 2 {
        return Err(ProxyReadError::BadVersion);
    }
    let command = match header_buf[12] & 0x0f {
        0x00 => Command::Local,
        0x01 => Command::Proxy,
        _ => return Err(ProxyReadError::InvalidProtocol),
    };
    let af = match (header_buf[13] & 0xf0) >> 4 {
        0x00 => AddressFamily::Unspec,
        0x01 => AddressFamily::Inet,
        0x02 => AddressFamily::Inet6,
        0x03 => AddressFamily::Unix,
        _ => return Err(ProxyReadError::InvalidProtocol),
    };
    let transport = match header_buf[13] & 0x0f {
        0x00 => TransportFamily::Unspec,
        0x01 => TransportFamily::Stream,
        0x02 => TransportFamily::Dgram,
        _ => return Err(ProxyReadError::InvalidProtocol),
    };
    let addrlen = NetworkEndian::read_u16(&header_buf[14..16]) as usize;
    let mut addr_buf = [0u8; 216];
    if addrlen > 216 {
        return Err(ProxyReadError::InvalidProtocol);
    }
    r.read_exact(&mut addr_buf[0..addrlen])?;
    let addr_buf = &addr_buf[0..addrlen];
    let (source, dest) = match af {
        AddressFamily::Inet => {
            let source_addr = IpAddr::from(Ipv4Addr::from(NetworkEndian::read_u32(&addr_buf[0..4])));
            let dest_addr = IpAddr::from(Ipv4Addr::from(NetworkEndian::read_u32(&addr_buf[4..8])));
            let source_port = NetworkEndian::read_u16(&addr_buf[8..10]);
            let dest_port = NetworkEndian::read_u16(&addr_buf[10..12]);
            (SocketAddr::new(source_addr, source_port), SocketAddr::new(dest_addr, dest_port))
        },
        AddressFamily::Inet6 => {
            let source_addr = IpAddr::from(slice_to_ipv6addr(&addr_buf[0..16]));
            let dest_addr = IpAddr::from(slice_to_ipv6addr(&addr_buf[16..32]));
            let source_port = NetworkEndian::read_u16(&addr_buf[32..34]);
            let dest_port = NetworkEndian::read_u16(&addr_buf[34..36]);
            (SocketAddr::new(source_addr, source_port), SocketAddr::new(dest_addr, dest_port))
        },
        AddressFamily::Unix => {
            return Ok(ProxyProtocolHeader::new_unknown(protocol_version))
        },
        AddressFamily::Unspec => {
            return Ok(ProxyProtocolHeader::new_unknown(protocol_version))
        }
    };
    if transport != TransportFamily::Stream {
        return Err(ProxyReadError::InvalidProtocol);
    }
    Ok(ProxyProtocolHeader::new_with_command(
        protocol_version,
        match af {
            AddressFamily::Inet => Proto::Tcp4,
            AddressFamily::Inet6 => Proto::Tcp6,
            AddressFamily::Unix => Proto::Unix,
            AddressFamily::Unspec => unreachable!()
        },
        command,
        source,
        dest
    ))
}

pub(crate) fn read_proxy_protocol_v2<R: Read>(r: &mut R) -> Result<ProxyProtocolHeader> {
    let mut header_buf = [0u8; 16];
    r.read_exact(&mut header_buf)?;
    if header_buf[0] != 0x0d {
        return Err(ProxyReadError::MissingLiteral);
    }
    read_proxy_protocol_v2_after_first_byte(r, &header_buf)
}


pub(crate) fn read_proxy_protocol_any<R: Read>(r: &mut R) -> Result<ProxyProtocolHeader> {
    let mut first_byte = [0u8; 1];
    r.read_exact(&mut first_byte)?;
    if first_byte[0] == 0x0d {
        read_proxy_protocol_v2_after_first_byte(r, &first_byte)
    } else if first_byte[0] == 0x50 {
        let mut buf = [0u8; 107];
        let buf_len = read_to_crlf(r, &mut buf)?;
        parse_proxy_protocol_v1_after_first_byte(&buf[..buf_len])
    } else {
        Err(ProxyReadError::MissingFirstByte)
    }
}

#[cfg(test)]
mod tests {
    use super::read_proxy_protocol_v1;
    use super::read_proxy_protocol_v2;
    use super::read_proxy_protocol_any;
    use super::Proto;
    use super::ProxyProtocolHeader;

    #[test]
    fn test_proxy_protocol_v1_spec_vectors() { 
        let vectors = vec![
            (b"PROXY TCP4 255.255.255.255 255.255.255.255 65535 65535\r\n".to_vec(), ProxyProtocolHeader::new(1, Proto::Tcp4, "255.255.255.255:65535".parse().unwrap(), "255.255.255.255:65535".parse().unwrap())),
            (b"PROXY TCP6 ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff 65535 65535\r\n".to_vec(), ProxyProtocolHeader::new(1, Proto::Tcp6, "[ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff]:65535".parse().unwrap(), "[ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff]:65535".parse().unwrap())),
            (b"PROXY UNKNOWN\r\n".to_vec(), ProxyProtocolHeader::new_unknown(1)),
            (b"PROXY TCP4 192.168.0.1 192.168.0.11 56324 443\r\n".to_vec(), ProxyProtocolHeader::new(1, Proto::Tcp4, "192.168.0.1:56324".parse().unwrap(), "192.168.0.11:443".parse().unwrap())),
        ];
        for (bytestr, expected) in vectors {
            let r = read_proxy_protocol_v1(&mut bytestr.as_slice()).expect("Should parse");
            assert_eq!(r, expected);
        }
    }

    #[test]
    fn test_proxy_protocol_v1_failure_cases() {
        read_proxy_protocol_v1(&mut (b"" as &[u8])).expect_err("should not parse");
        read_proxy_protocol_v1(&mut (b"\r\n" as &[u8])).expect_err("should not parse");
        read_proxy_protocol_v1(&mut (b"proxy tcp4 255.255.255.255 255.255.255.255 0 0\r\n" as &[u8])).expect_err("should not parse");
    }

    #[test]
    fn test_proxy_protocol_v2_vectors() {
        let vectors = vec![
            (b"\x0d\x0a\x0d\x0a\x00\x0d\x0a\x51\x55\x49\x54\x0a\x21\x11\x00\x0c\x0a\x0b\x0c\x0d\x7f\x00\x00\x01\x22\xb8\x27\x0f".to_vec(), ProxyProtocolHeader::new(2, Proto::Tcp4, "10.11.12.13:8888".parse().unwrap(), "127.0.0.1:9999".parse().unwrap())),
            (b"\x0d\x0a\x0d\x0a\x00\x0d\x0a\x51\x55\x49\x54\x0a\x21\x21\x00\x24\xfd\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01\x22\xb8\x27\x0f".to_vec(), ProxyProtocolHeader::new(2, Proto::Tcp6, "[fd00::1]:8888".parse().unwrap(), "[::1]:9999".parse().unwrap()))
        ];
        for (bytestr, expected) in vectors {
            let r = read_proxy_protocol_v2(&mut bytestr.as_slice()).expect("Should parse");
            assert_eq!(r, expected);
        }
    }

    #[test]
    fn test_proxy_protocol_v2_failure_cases() {
        read_proxy_protocol_v2(&mut (b"" as &[u8])).expect_err("should not parse");
        read_proxy_protocol_v2(&mut (b"\x0d\x0a\x0d\x0a\x00\x0d\x0a\x51\x55\x49\x54\x0a" as &[u8])).expect_err("should not parse");
    }

    #[test]
    fn test_proxy_protocol_any() {
        let vectors = vec![
            (b"PROXY TCP4 192.168.0.1 192.168.0.11 56324 443\r\n".to_vec(), ProxyProtocolHeader::new(1, Proto::Tcp4, "192.168.0.1:56324".parse().unwrap(), "192.168.0.11:443".parse().unwrap())),
            (b"\x0d\x0a\x0d\x0a\x00\x0d\x0a\x51\x55\x49\x54\x0a\x21\x11\x00\x0c\x0a\x0b\x0c\x0d\x7f\x00\x00\x01\x22\xb8\x27\x0f".to_vec(), ProxyProtocolHeader::new(2, Proto::Tcp4, "10.11.12.13:8888".parse().unwrap(), "127.0.0.1:9999".parse().unwrap())),
        ];
        for (bytestr, expected) in vectors {
            let r = read_proxy_protocol_any(&mut bytestr.as_slice()).expect("should parse");
            assert_eq!(r, expected);
        }
    }
}
