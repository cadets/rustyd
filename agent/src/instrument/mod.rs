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

extern crate toml;
extern crate libc;
extern crate libloading;
extern crate sysctl;

use std::ffi::{CString, CStr};
use self::libdtrace::dtrace_workstatus_t;
use std::default::Default;
use std::sync::mpsc;
use std::os::raw::c_char;
use std::str::from_utf8;

mod libdtrace;

#[derive(Debug, RustcDecodable)]
struct Config {
    instrumentation: Option<Instrumentation>,
}

#[derive(Debug, RustcDecodable)]
struct Instrumentation {
    comment: Option<String>,
    script: Option<String>,
}

#[derive(PartialEq)]
pub enum InstrumentationThreadMessage {
   Stop,
}

struct TransportBridge {
    handle: i32,
    lib: libloading::Library,
}

// TODO replace fix handle values with those returns by dylib
impl TransportBridge {
		   
    fn new(transport_plugin: &'static str) -> TransportBridge {

       TransportBridge {
          handle: -1,
	      lib: libloading::Library::new(transport_plugin).unwrap(),
       }
    }

    fn close(&self) -> i32 {
      trace!("close()");
      unsafe {
           if let Ok(close_func) = self.lib.get::<libloading::Symbol<unsafe extern fn(i32) -> i32>>(DT_CLOSE_FCN) {
               close_func(self.handle)
           } else {
               -1
           }
       }
    }

    fn open(&mut self, config: &str) -> i32 {
      trace!("open()");
      unsafe {
           if let Ok(open_func) =
               self.lib.get::<libloading::Symbol<unsafe extern fn(* const c_char) -> i32>>(DT_OPEN_FCN) {
               self.handle = open_func(CString::new(config).unwrap().as_ptr());
               0
           } else {
               -1
           }
       }
    }

    fn write(&self, data: &[u8]) -> i32 {
       trace!("write({:?})", from_utf8(data).unwrap());
       unsafe {
           if let Ok(write_func) =
               self.lib.get::<libloading::Symbol<unsafe extern fn(i32, &[u8]) -> i32>>(DT_WRITE_FCN) {
               write_func(self.handle, data)
           } else {
               -1
           }
       }
    }

    fn flush(&self) -> i32 {
       trace!("flush() {}", self.handle);
       unsafe {
           if let Ok(flush_func) =
               self.lib.get::<libloading::Symbol<unsafe extern fn(i32) -> i32>>(b"dt_transport_flush") {
               flush_func(100) //self.handle)
           } else {
               -1
           }
       }
    }
}

const DT_OPEN_FCN: &'static[u8] = b"dt_transport_open";
const DT_CLOSE_FCN: &'static[u8] = b"dt_transport_close";
const DT_WRITE_FCN: &'static[u8] = b"dt_transport_write";
const DT_FLUSH_FCN: &'static[u8] = b"dt_transport_flush";

// libdtrace constants;
const EINTR: i32 = 9959;

const DTRACE_CONSUME_NEXT: i32 = 1;
const DTRACE_CONSUME_THIS: i32 = 0;

fn dtrace_open(version: i32, flags: i32)
   -> (*mut self::libdtrace::dtrace_hdl_t, i32) {

   let mut err: libc::c_int = 0;    
   let handle = unsafe {
      self::libdtrace::dtrace_open(version, flags, &mut err)
   };
   (handle, err)
}

fn dtrace_setopt(handle: *mut self::libdtrace::dtrace_hdl_t,
    opt: &'static str, val: &'static str) {

    unsafe {
        self::libdtrace::dtrace_setopt(handle,
            CString::new(opt).unwrap().as_ptr(),
            CString::new(val).unwrap().as_ptr());
    }
}

fn dtrace_errmsg(handle: *mut self::libdtrace::dtrace_hdl_t, err: i32)
    -> String {

    unsafe {
        CStr::from_ptr(self::libdtrace::dtrace_errmsg(handle, err))
             .to_string_lossy().into_owned()
    }
}

fn dtrace_program_strcompile(handle: *mut self::libdtrace::dtrace_hdl_t,
    script: & str, spec: self::libdtrace::dtrace_probespec_t, cflags: u32)
    -> *mut self::libdtrace::dtrace_prog_t {

    if let sysctl::CtlValue::String(hostuuid) =
        sysctl::value("kern.hostuuid").unwrap() {
        let args = vec!(CString::new(format!("\"{}\"", hostuuid)).unwrap());
        let c_args = args.iter().map(|arg| arg.as_ptr()).collect::<Vec<*const c_char>>();
        unsafe {
            self::libdtrace::dtrace_program_strcompile(handle,
                CString::new(script).unwrap().as_ptr(), spec,
                cflags, c_args.len() as ::std::os::raw::c_int, c_args.as_ptr())
        }
    } else {
unsafe {
            self::libdtrace::dtrace_program_strcompile(handle,
                CString::new(script).unwrap().as_ptr(), spec,
                cflags, 0, ::std::ptr::null())
        }
    }
}
    
fn dtrace_program_exec(handle: *mut self::libdtrace::dtrace_hdl_t,
    prog: *mut self::libdtrace::dtrace_prog_t,
    info: *mut self::libdtrace::dtrace_proginfo_t) -> i32 {

    unsafe {
        self::libdtrace::dtrace_program_exec(handle, prog, info)
    }
}

fn dtrace_go(handle: *mut self::libdtrace::dtrace_hdl_t) -> i32 {
    unsafe {
        self::libdtrace::dtrace_go(handle)
    }
}

fn dtrace_work(handle: *mut self::libdtrace::dtrace_hdl_t,
   fp: *mut self::libdtrace::__sFILE,
   pfunc: self::libdtrace::dtrace_consume_probe_f,
   rfunc: self::libdtrace::dtrace_consume_rec_f,
   arg: *mut ::std::os::raw::c_void) -> self::libdtrace::dtrace_workstatus_t {
    unsafe {
        self::libdtrace::dtrace_work(handle, fp, pfunc, rfunc, arg)
    }
}

fn dtrace_sleep(handle: *mut self::libdtrace::dtrace_hdl_t) {
    unsafe {
        self::libdtrace::dtrace_sleep(handle)
    }
}

fn dtrace_stop(handle: *mut self::libdtrace::dtrace_hdl_t) -> i32 {
    unsafe {
        self::libdtrace::dtrace_stop(handle)
    }
}

fn dtrace_close(handle: *mut self::libdtrace::dtrace_hdl_t) {
    unsafe {
        self::libdtrace::dtrace_close(handle)
    }
}

fn dtrace_errno(handle: *mut self::libdtrace::dtrace_hdl_t) -> i32 {
    unsafe {
        self::libdtrace::dtrace_errno(handle)
    }
}

fn dtrace_handle_drop(handle: *mut self::libdtrace::dtrace_hdl_t,
    hdlr: self::libdtrace::dtrace_handle_drop_f,
    arg: *mut ::std::os::raw::c_void) -> ::std::os::raw::c_int {
    unsafe {
        self::libdtrace::dtrace_handle_drop(handle, hdlr, arg)
    }
}

fn dtrace_handle_buffered(handle: *mut self::libdtrace::dtrace_hdl_t,
    hdlr: self::libdtrace::dtrace_handle_buffered_f,
    arg: *mut ::std::os::raw::c_void) -> ::std::os::raw::c_int {
    unsafe {
        self::libdtrace::dtrace_handle_buffered(handle, hdlr, arg)
    }
}

// TODO how to elegantly exit here
// need to delete script from instrumentation or only add when successfully compiled?
pub fn instrument_endpoint(script: String,
    rx: mpsc::Receiver<InstrumentationThreadMessage>) {

    // Parse the configuration file specifying where the DTrace records are to
    // be sent
    unsafe {
        match toml::decode_str::<Config>(script.as_str()) {
            Some(config) => {
                let dtrace_version = 3;
                let flags = 0;
                let (handle, err) = dtrace_open(dtrace_version, flags);
                if err != 0 {
                    error!("dtrace error {} initializing", dtrace_errmsg(handle, err));
                    return;
                }
                info!("dtrace initialized");
                
                dtrace_setopt(handle, "bufsize", "4m");
                dtrace_setopt(handle, "aggsize", "4m");
                dtrace_setopt(handle, "temporal", "4m");;
                dtrace_setopt(handle, "arch", "x86_64");;
                info!("dtrace options set");

                let instr_script = config.instrumentation.unwrap().script.unwrap();
                let prog = dtrace_program_strcompile(handle, instr_script.as_str(),
                    self::libdtrace::dtrace_probespec::DTRACE_PROBESPEC_NAME, 0x0080);
                if prog.is_null() {
                    error!("failed to compile dtrace program {}",
                       dtrace_errmsg(handle, dtrace_errno(handle)));
                    dtrace_close(handle);
                    return;
                }
                info!("dtrace program compiled");

                let mut info: self::libdtrace::dtrace_proginfo_t = Default::default();
                let status = dtrace_program_exec(handle, prog, &mut info);
                if status == -1 {
                    error!("failed to enable dtrace probes {}",
                       dtrace_errmsg(handle, dtrace_errno(handle)));
                    dtrace_close(handle);
                    return;
                }
                info!("dtrace probes enables");

                if dtrace_go(handle) != 0 {
                    error!("could not start dtrace instrumentation {}",
                        dtrace_errmsg(handle, dtrace_errno(handle)));
                    dtrace_close(handle);
                    return;
                }
                info!("dtrace instrumentation started...");
               
                let mut handler = TransportBridge::new(
                    "../transport/unix_socket/target/debug/libddtrace_unix_socket.so");
                   // "../transport/tcp/target/debug/libddtrace_tcp.so");
                handler.open(script.as_str());

                unsafe {
                    if dtrace_handle_drop(handle, drop_handler,
                       ::std::ptr::null_mut()) == -1 {
                        error!("failed to register dtrace drop handler");
                        dtrace_close(handle);
                        return;
                    }

                    let lib_ptr: *mut ::std::os::raw::c_void =
                        &mut handler as *mut _ as *mut ::std::os::raw::c_void;
                    if dtrace_handle_buffered(handle, buffered_handler, lib_ptr) == -1 {
                        error!("failed to register dtrace buffered handler");
                        dtrace_close(handle);
                        return;
                    }
                
                    let mut done = false;
                    while {
                        if done == false {
                           dtrace_sleep(handle);
                        }

                        trace!("dtrace work...");
                        match dtrace_work(handle, ::std::ptr::null_mut(), chew, chewrec, lib_ptr ){
//                            handle as *mut ::std::os::raw::c_void) {
                            dtrace_workstatus_t::DTRACE_WORKSTATUS_ERROR => {
                                if dtrace_errno(handle) != EINTR {
                                    error!("{}", dtrace_errmsg(handle, dtrace_errno(handle)));
                                    done = true;
                                }
                            },
                            dtrace_workstatus_t::DTRACE_WORKSTATUS_OKAY => {
                                done = false;
                            },
                            dtrace_workstatus_t::DTRACE_WORKSTATUS_DONE => {
                                done = true;
                            }
                        }

                        done = match rx.try_recv() {
                            Ok(ref msg) if *msg == InstrumentationThreadMessage::Stop => {
                                true
                            },
                            Ok(_) => {
                                done
                            },
                            Err(error) if error == mpsc::TryRecvError::Empty => {
                                done
                            },
                            Err(error) => {
                                error!("{}", error);
                                false
                            },
                        };

                        done == false
                    } {}
                }

                // Close the transport handler
                handler.close();

                info!("dtrace stopping");
                dtrace_stop(handle);

                info!("dtrace closing");
                dtrace_close(handle);
            },
            None => {
                error!("Failed decoding TOML config");
                return;
            }
        }
    }
}

unsafe extern fn drop_handler(
   data : *const self::libdtrace::dtrace_dropdata_t,
   arg: *mut ::std::os::raw::c_void) -> ::std::os::raw::c_int {
   
   error!("{:?}", data);
   0 // DTRACE_HANDLE_OK
}

unsafe extern fn buffered_handler(
   bufdata : *const self::libdtrace::dtrace_bufdata_t,
   arg: *mut ::std::os::raw::c_void) -> ::std::os::raw::c_int {
       
   // Write the records upstream using the specified transport handler
   let handler = arg as *const TransportBridge;
   (* handler).write(CStr::from_ptr((* bufdata).dtbda_buffered).to_bytes())
}
       
unsafe extern fn chew(data: *const self::libdtrace::dtrace_probedata_t,
    arg: *mut ::std::os::raw::c_void) -> i32 {
    
    info!("chew");

    let pd: *mut self::libdtrace::dtrace_probedesc_t = (* data).dtpda_pdesc;
    //trace!("id = {}", (* pd).dtpd_id); timestamp

    //trace!("id = {}", (* data).processorid_t); cpu
    trace!("id = {}", (* pd).dtpd_id);
    trace!("func = {:?}", CStr::from_ptr((* pd).dtpd_func.as_ptr()));
    trace!("name = {:?}", CStr::from_ptr((* pd).dtpd_name.as_ptr()));

    // Consume this record - DTRACE_CONSUME_THIS
    return DTRACE_CONSUME_THIS;
}

unsafe extern fn chewrec(_data: *const self::libdtrace::dtrace_probedata_t,
    rec: *const self::libdtrace::dtrace_recdesc_t,
    arg: *mut ::std::os::raw::c_void) -> i32 {

    trace!("chewing DTrace record");

    if rec.is_null() {
        // Consume next record - DTRACE_CONSUME_NEXT
        trace!("consume next");
       
        // Flush the records upstream using the specified transport handler
        let handler = arg as *const TransportBridge;
        (* handler).flush();

        return DTRACE_CONSUME_NEXT;
    } else {
        let action = (* rec).dtrd_action;
        trace!("chewrec() record action = {}", action);
        if action == 2 { 
            // Consume next record - DTRACE_CONSUME_NEXT
            trace!("chewrec() consume next");
       
            // Flush the records upstream using the specified transport handler
            let handler = arg as *const TransportBridge;
            (* handler).flush();

            return DTRACE_CONSUME_NEXT;
        } else {
            // Consume this record - DTRACE_CONSUME_THIS
            trace!("chewrec() consume this");
            return DTRACE_CONSUME_THIS;
        }
    }
}

