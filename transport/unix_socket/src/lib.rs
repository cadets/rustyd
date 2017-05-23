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
extern crate rand;
#[macro_use]
extern crate serde_derive;
extern crate serde;

use std::collections::HashMap;
use std::ffi::CStr;
use std::io::prelude::*;
use std::io::BufWriter;
use std::os::unix::net::UnixStream;
use std::sync::Mutex;
use rand::Rng;

static SUCCESS: i32 = 0;
static ERR_INVALID_HANDLE: i32 = -1;
static ERR_INVALID_CONFIG: i32 = -2;

#[derive(Debug, Deserialize)]
struct Config {
    instrumentation: Option<Instrumentation>,
}

#[derive(Debug, Deserialize)]
struct Instrumentation {
    server: Option<ServerConfig>,
}

#[derive(Debug, Deserialize)]
struct ServerConfig {
    path: Option<String>,
}

struct Context {
    conn_id: i32,
    handle_map: Mutex<HashMap<i32, BufWriter<UnixStream>>>,
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
pub fn dt_transport_open(config_raw: * const std::os::raw::c_char) -> i32
{
    // Read the configuration (a TOML formated string)
    if let Ok(config_str) = unsafe { CStr::from_ptr(config_raw).to_str() } {
        trace!("Unix socket configuration {:?}", config_str);

        if let Ok(config) = toml::from_str::<Config>(config_str) {
            if let Some(handle) = config.instrumentation
                .and_then(|instrumentation| { instrumentation.server } )
                .and_then(|server| {
                if server.path.is_some() {
                    let path = server.path.unwrap();
                    info!("Connecting to Unix socket {}", path);
                    match UnixStream::connect(path) {
                        Ok(unix_stream) => {
                            info!("Opened new Unix socket {:?}", unix_stream);
                            let buffer = BufWriter::new(unix_stream);

                            // Generate a random handle for the tcp stream
                            //let handle = CONTEXT.conn_id.wrapping_add(1);
                            let handle = 100;
                            trace!("Storing new connection handle {}", handle);
                            // TODO test if handle is already present
                            // if CONTEXT.handle_map.lock().unwrap().contains_key(handle) {
                            CONTEXT.handle_map.lock().unwrap().insert(
                                handle, buffer);
                            Some(handle) 
                        },
                        Err(e) => {
                            error!("Failed opening Unix socket {:?}", e);
                            None
                        }
                    }
                } else {
                    None
                }
            }) {
               handle 
            } else {
                ERR_INVALID_CONFIG
            }
        } else {
            ERR_INVALID_CONFIG
        }
    } else {
       ERR_INVALID_CONFIG
    }
}

#[no_mangle]
pub fn dt_transport_close(handle: i32) -> i32
{
    // Remove the stream from the CONTEXT handle_map.
    // This will close the underlying TCP connection.
    if let Some(stream) = CONTEXT.handle_map.lock().unwrap().remove(&handle) {
        // The stream is closed here (once removed from the map)
        trace!("Closing connection to {:?}", stream);
        SUCCESS
    } else {
        error!("Connection handle invalid");
        ERR_INVALID_HANDLE
    }
}

#[no_mangle]
pub fn dt_transport_write(handle: i32, data: &[u8]) -> i32
{
    // Lookup the stream corresponding to the handle
    if let Some(mut stream) =
        CONTEXT.handle_map.lock().unwrap().get_mut(&handle) {
        // Write DTrace records to the stream 
        match stream.write(data) {
            Ok(_) => {
                trace!("Successfully wrote {:?} to {:?}", data, stream);
                SUCCESS
            },
            Err(err) => {
                error!("Error writing to {:?}: {:?}", stream, err);
                -1
            }
        }
    } else {
        error!("Connection handle invalid");
        ERR_INVALID_HANDLE
    }
}

#[no_mangle]
pub fn dt_transport_flush(handle: i32) -> i32
{
    if let Some(mut stream) =
        CONTEXT.handle_map.lock().unwrap().get_mut(&handle) {
        match stream.flush() {
            Ok(_) => {
                trace!("Successfully flushed {:?}", stream);
                SUCCESS
            },
            Err(err) => {
                error!("Failed flushing to {:?}: {:?}", stream, err);
                -1
            }
        }
    } else {
        error!("Connection handle invalid");
        ERR_INVALID_HANDLE
    }
}

#[no_mangle]
pub fn dt_transport_writeall(handle: i32, data: &[u8]) -> i32
{
    if let Some(mut stream) =
        CONTEXT.handle_map.lock().unwrap().get_mut(&handle) {
        match stream.write_all(data) {
            Ok(_) => {
                trace!("Successfully wrote {:?} to {:?}", data, stream);
                SUCCESS
            },
            Err(err) => {
                error!("Error writing to {:?}: {:?}", stream, err);
                // get raw_os_error
                -1
            }
        }
    } else {
        error!("Connection handle invalid");
        ERR_INVALID_HANDLE
    }
}
