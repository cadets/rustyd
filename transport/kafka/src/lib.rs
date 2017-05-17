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
extern crate kafka;
#[macro_use]
extern crate lazy_static;
extern crate toml;
extern crate rustc_serialize;

use kafka::producer::{Producer, Record};
use std::ffi::CStr;

#[derive(Debug, RustcDecodable)]
struct Config {
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
      let handle = 100;
      handle
   } else {
      -1
   }
}

#[no_mangle]
pub fn dt_transport_close() -> i32
{
   100
}

#[no_mangle]
pub fn dt_transport_write(data: &[u8]) -> i32
{
   let mut producer =
        match Producer::from_hosts(vec!["172.16.100.164:9092".to_owned()])
           .with_ack_timeout(1000)
           .with_required_acks(1)
           .create() {
           Ok(val) => val,
           Err(e) => {error!("creating Kafka producer {}", e); return -1;},
       }; 

    match producer.send(&Record{
        topic: "ddtrace-query-response",
        partition: -1,
        key: (),
        value: data,}) {
           Ok(_) => {},
           Err(e) => {error!("sending to Kafka {}", e); return -1;},
       }; 
   0
}

#[no_mangle]
pub fn dt_transport_flush() -> i32
{
   100
}

#[no_mangle]
pub fn dt_transport_writeall() -> i32
{
   100
}

