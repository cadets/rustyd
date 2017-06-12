/*-
 * Copyright (c) 2016 (Graeme Jenkinson)
 * All rights reserved.
 *
 * This software was developed by BAE Systems, the University of Cambridge
 * Computer Laboratory, and Memorial University under DARPA/AFRL contract
 * FA8650-15-C-7558 ("CADETS"), as part of the DARPA Transparent Computing
 * (TC) research program.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR AND CONTRIBUTORS ``AS IS'' AND
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS BE LIABLE
 * FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
 * OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
 * LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
 * OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
 * SUCH DAMAGE.
 *
 */

#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;
extern crate toml;
extern crate rustc_serialize;
extern crate rand;

use std::io::prelude::*;
use std::io::BufWriter;
use std::net::TcpStream;
use std::collections::HashMap;
use std::sync::Mutex;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::str::FromStr;
use std::ffi::CStr;
use rand::Rng;

#[derive(Debug, RustcDecodable)]
struct Config {
    instrumentation: Option<Instrumentation>,
}

#[derive(Debug, RustcDecodable)]
struct Instrumentation {
    server: Option<ServerConfig>,
}

#[derive(Debug, RustcDecodable)]
struct ServerConfig {
    ip: Option<String>,
    port: Option<u16>,
}

struct Context {
    conn_id: i32,
    handle_map: Mutex<HashMap<i32, BufWriter<TcpStream>>>,
}

impl Context {
    pub fn new() -> Context {
        Context {
            handle_map: Mutex::new(HashMap::new()),
            conn_id: rand::thread_rng().gen::<i32>(),
        }
    }
}

lazy_static! {
   static ref CONTEXT: Context = Context::new();
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}

#[no_mangle]
pub fn dt_transport_init() -> i32
{
   0
}

#[no_mangle]
pub fn dt_transport_fini() -> i32
{
   0
}

#[no_mangle]
pub fn dt_transport_open(config_raw: * const std::os::raw::c_char) -> i32
{
    if let Ok(config_str) = unsafe { CStr::from_ptr(config_raw).to_str() } {
        // Read the configuration (a TOML formated string)
        trace!("TCP stream configuration {:?}", config_str);
        if let Some(handle) = toml::decode_str::<Config>(config_str)
            .and_then(|config| { config.instrumentation } )
            .and_then(|instrumentation| { instrumentation.server } )
            .and_then(|server| {
                if server.ip.is_some() && server.port.is_some() {
                    let ip = IpAddr::from_str(server.ip.unwrap().as_str()).unwrap();
                    let port = server.port.unwrap();
                    let addr = SocketAddr::new(ip, port);

                    match TcpStream::connect(addr) {
                        Ok(tcp_stream) => {
                            info!("Opened new TCP connection to {}", addr);
                            let buffer = BufWriter::new(tcp_stream);

                            // Generate a random handle for the tcp stream
                            let handle = 100;
                            // handle = CONTEXT.handle_base;
                            // CONTEXT.handle_base += 1;
                            trace!("Storing new connection handle {}", handle);
                            CONTEXT.handle_map.lock().unwrap().insert(
                                handle, buffer);
                            Some(handle) 
                        },
                        Err(e) => {
                            error!("Failed opened new TCP connection to {}: {:?}", addr, e);
                            None
                        }
                    }
                } else { 
                    None
                }
        }) {
            handle 
        } else {
            -1
        }
    } else {
        -1
    }
}

#[no_mangle]
pub fn dt_transport_close(handle: i32) -> i32
{
    if let Some(stream) = CONTEXT.handle_map.lock().unwrap().remove(&handle) {
        // The stream is closed here
        //trace!("Closing TCP connection to {:?}", stream.peer_addr());
        0
    } else {
        error!("error closing TCP connection invalid handle {}", handle);
        -1
    }
}

#[no_mangle]
pub fn dt_transport_write(handle: i32, data: &[u8]) -> i32
{
    if let Some(mut stream) =
        CONTEXT.handle_map.lock().unwrap().get_mut(&handle) {
        match stream.write(data) {
            Ok(_) => {
                info!("OK");
                0
            },
            Err(err) => {
                //error!("Error writting to {:?}: {:?}", stream.peer_addr(), err);
                -1
            }
        }
    } else {
        error!("handle not found {}", handle);
        -1
    }
}

#[no_mangle]
pub fn dt_transport_flush(handle: i32) -> i32
{
    if let Some(mut stream) =
        CONTEXT.handle_map.lock().unwrap().get_mut(&handle) {
        match stream.flush() {
            Ok(_) => {
                trace!("flushing DTrace records");
                0
            },
            Err(err) => {
                //trace!("Failed flushing DTrace records to {:?}: {:?}", stream.peer_addr(), err);
                -1
            }
        }
    } else {
        error!("handle not found {}", handle);
        -1
    }
}

#[no_mangle]
pub fn dt_transport_writeall(handle: i32, data: &[u8]) -> i32
{
    if let Some(mut stream) =
        CONTEXT.handle_map.lock().unwrap().get_mut(&handle) {
        match stream.write_all(data) {
            Ok(_) => {
                info!("OK");
                0
            },
            Err(err) => {
                //error!("Error writting to {:?}: {:?}", stream.peer_addr(), err);
                -1
            }
        }
    } else {
        error!("handle not found {}", handle);
        -1
    }
}
