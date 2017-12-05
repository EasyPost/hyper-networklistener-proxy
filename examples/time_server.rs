extern crate hyper_networklistener_proxy;
extern crate clap;
extern crate hyper;
extern crate iron;
extern crate router;
#[macro_use] extern crate log;
extern crate env_logger;

use std::time::{SystemTime, UNIX_EPOCH};

use clap::Arg;
use hyper_networklistener_proxy::{ProxyListener, ProxyProtocolVersion};
use iron::prelude::*;
use hyper::net::HttpListener;
use router::Router;
use iron::status;

fn handler(request: &mut Request) -> IronResult<Response> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    debug!("got request from {:?}", request.remote_addr);
    Ok(Response::with((status::Ok, format!("time: {}\nyou: {}", now, request.remote_addr))))
}


fn main() {
    let matches = clap::App::new("time_server")
                            .version("0.1.0")
                            .author("James Brown <jbrown@easypost.com>")
                            .arg(Arg::with_name("bind")
                                     .short("B")
                                     .takes_value(true)
                                     .required(true)
                                     .value_name("LISTEN_ADDRESS")
                                     .help("Address to bind to"))
                            .get_matches();

    env_logger::init().unwrap();

    let inner_listener = HttpListener::new(matches.value_of("bind").unwrap()).unwrap();
    let listener = ProxyListener::new(inner_listener, ProxyProtocolVersion::V1);

    let mut router = Router::new();

    router.get("/", handler, "index");

    Iron::new(router).listen(listener, iron::Protocol::http()).unwrap();
}
