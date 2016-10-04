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

use std::io::prelude::*;
use std::net::TcpStream;
use std::collections::HashMap;
use std::sync::Mutex;

lazy_static! {
    static ref HANDLE_MAP: Mutex<HashMap<i32, TcpStream>> =
        Mutex::new(HashMap::new());
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}

#[no_mangle]
pub fn my_init() -> i32
{
   0
}

#[no_mangle]
pub fn my_fini() -> i32
{
   0
}

#[no_mangle]
pub fn my_open() -> i32
{
    let mut stream = TcpStream::connect("127.0.0.1:34254").unwrap();
    HANDLE_MAP.lock().unwrap().insert(100, stream);
    100
}

#[no_mangle]
pub fn my_close() -> i32
{
    let mut _stream = HANDLE_MAP.lock().unwrap().remove(&100);
    100
}

#[no_mangle]
pub fn my_read() -> i32
{
   100
}

#[no_mangle]
pub fn my_write(data: &[u8]) -> i32
{
    if let Some(mut stream)= HANDLE_MAP.lock().unwrap().get(&100) {
        match stream.write_all(data) {
            Ok(_) => { 0 },
            Err(err) => { -1 }
        }
    } else {
        -1
    }
}

#[no_mangle]
pub fn my_flush() -> i32
{
   100
}

