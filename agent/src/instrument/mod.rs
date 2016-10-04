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

//extern crate log;
//extern crate log4rs;
//extern crate time;
extern crate libc;
//extern crate rustc_serialize;
//extern crate docopt;
//extern crate errno;
//extern crate kafka;
extern crate libloading;

use kafka::producer::{Producer, Record};
use std::ffi::{CString, CStr};
use self::libdtrace::dtrace_workstatus_t;
use std::process::exit;
use std::default::Default;
use std::sync::mpsc;

mod libdtrace;
mod libxo;

#[derive(PartialEq)]
pub enum InstrumentationThreadMessage {
   Stop,
}

// libdtrace constants;
// note these do not get generated automatically by the 'bindgen' tool
const XO_STYLE_JSON: libxo::xo_style_t = 2;

const XOF_WARN: libxo::xo_xof_flags_t = 16;
const XOF_DTRT: libxo::xo_xof_flags_t = 1024;

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

    unsafe {
        self::libdtrace::dtrace_program_strcompile(handle,
            CString::new(script).unwrap().as_ptr(), spec,
            cflags, 0, ::std::ptr::null())
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

fn dtrace_handle_buffered(handle: *mut self::libdtrace::dtrace_hdl_t,
    hdlr: self::libdtrace::dtrace_handle_buffered_f,
    arg: *mut ::std::os::raw::c_void) -> ::std::os::raw::c_int {
    unsafe {
        self::libdtrace::dtrace_handle_buffered(handle, hdlr, arg)
    }
}

fn xo_create(style: libxo::xo_style_t, flags: libxo::xo_xof_flags_t)
    -> *mut libxo::xo_handle_t {
    unsafe {
        libxo::xo_create(style, flags)
    }
}

fn xo_finish_h(xop: *mut libxo::xo_handle_t) -> ::std::os::raw::c_int {
    unsafe {
        libxo::xo_finish_h(xop)
    }
}

fn xo_destroy(xop: *mut libxo::xo_handle_t) {
    unsafe {
        libxo::xo_destroy(xop)
    }
}

fn xo_set_writer(xop: *mut libxo::xo_handle_t,
    opaque: *mut ::std::os::raw::c_void,
    write_func: libxo::xo_write_func_t,
    close_func: libxo::xo_close_func_t,
    flush_func: libxo::xo_flush_func_t) {
    unsafe {
        libxo::xo_set_writer(xop, opaque, write_func, close_func,
           flush_func)
    }
}

pub fn instrument_endpoint(script: String,
   rx: mpsc::Receiver<InstrumentationThreadMessage>) {

    let dtrace_version = 3;
    let flags = 0;
    let (handle, err) = dtrace_open(dtrace_version, flags);
    if err != 0 {
        error!("dtrace error {} initializing", dtrace_errmsg(handle, err));
        return;
        //exit(1);
    }
    info!("dtrace initialized");
    
//    dtrace_setopt(handle, "oformat", "json");
    dtrace_setopt(handle, "bufsize", "4m");
    dtrace_setopt(handle, "aggsize", "4m");
    dtrace_setopt(handle, "temporal", "4m");;
    dtrace_setopt(handle, "arch", "x86_64");;
    info!("dtrace options set");
 
    let prog = dtrace_program_strcompile(handle, script.as_str(),
        self::libdtrace::dtrace_probespec::DTRACE_PROBESPEC_NAME, 0x0080);
    if prog.is_null() {
        error!("failed to compile dtrace program {}",
           dtrace_errmsg(handle, dtrace_errno(handle)));
        // TODO how to elegantly exit here - need to delete script from instrumentation or only add when successfully compiled?
        //exit(1);
        dtrace_close(handle);
        return;
    }
    info!("dtrace program compiled");

    let mut info: self::libdtrace::dtrace_proginfo_t = Default::default();
    let status = dtrace_program_exec(handle, prog, &mut info);
    if status == -1 {
        error!("failed to enable dtrace probes {}",
           dtrace_errmsg(handle, dtrace_errno(handle)));
        exit(1);
    }
    info!("dtrace probes enables");

    if dtrace_go(handle) != 0 {
       error!("could not start dtrace instrumentation {}",
           dtrace_errmsg(handle, dtrace_errno(handle)));
       exit(1);
    }
    info!("dtrace instrumentation started...");

    if dtrace_handle_buffered(handle, buffered_handler,
        ::std::ptr::null_mut()) == -1 {
        error!("failed to register dtrace buffered handler");
        exit(1);
    }

    let mut done = false;
    while {
        if done == false {
           dtrace_sleep(handle);
        }

        trace!("dtrace work...");
        match dtrace_work(handle, ::std::ptr::null_mut(), chew, chewrec,
            handle as *mut ::std::os::raw::c_void) {
            dtrace_workstatus_t::DTRACE_WORKSTATUS_ERROR => {
                if dtrace_errno(handle) != EINTR {
                    error!("{}", dtrace_errmsg(handle, dtrace_errno(handle)));
                    done = true;
                }
            }
			dtrace_workstatus_t::DTRACE_WORKSTATUS_OKAY => {
                done = false;
            }
            dtrace_workstatus_t::DTRACE_WORKSTATUS_DONE => {
                done = true;
            }
        }

        done = match rx.try_recv() {
           Ok(ref msg) if *msg == InstrumentationThreadMessage::Stop => {true},
           Ok(_) => {done},
           Err(error) if error == mpsc::TryRecvError::Empty => { done },
           Err(error) => { error!("{}", error); false},
        };

        done == false
    } {}

    info!("dtrace stopping");
    dtrace_stop(handle);

    info!("dtrace closing");
    dtrace_close(handle);
}

unsafe extern fn buffered_handler(
   bufdata : *const self::libdtrace::dtrace_bufdata_t,
   _arg: *mut ::std::os::raw::c_void) -> ::std::os::raw::c_int {
       
   info!("buffered_handler {:?}", CStr::from_ptr((* bufdata).dtbda_buffered));
 
   match libloading::Library::new("../transport/tcp/target/debug/libddtrace_tcp.so") {
      Ok(lib) => {
         if let Ok(func) =lib.get::<libloading::Symbol<unsafe extern fn(&[u8]) -> i32>>(b"my_write") {
            func(CStr::from_ptr((* bufdata).dtbda_buffered).to_bytes())
         } else { -1 }
      },
      Err(_e) => { -1 }
   }
}

unsafe extern fn ddtrace_xo_write(_arg1: *mut ::std::os::raw::c_void,
    buf: *const ::std::os::raw::c_char) -> ::std::os::raw::c_int {
   
    info!("ddtrace_xo_write");
 
/*    
    let mut producer =
        match Producer::from_hosts(vec!["172.16.100.165:9092".to_owned()])
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
        value: CStr::from_ptr(buf).to_bytes(),}) {
           Ok(_) => {},
           Err(e) => {error!("sending to Kafka {}", e); return -1;},
       }; 
*/
    return 0;
}

unsafe extern fn chew(data: *const self::libdtrace::dtrace_probedata_t,
    arg: *mut ::std::os::raw::c_void) -> i32 {
    
    info!("chew");

    // Create a libxo handle
    let xop = xo_create(XO_STYLE_JSON, XOF_WARN | XOF_DTRT);
    xo_set_writer(xop, ::std::ptr::null_mut(),
        Some(ddtrace_xo_write), None, None);

    let handle = arg as *mut self::libdtrace::dtrace_hdl_t;
    (* handle).dt_xo_hdl = xop as *mut self::libdtrace::xo_handle_s;
 
   match libloading::Library::new("../transport/tcp/target/debug/libddtrace_tcp.so") {
      Ok(lib) => {
         if let Ok(func) =lib.get::<libloading::Symbol<unsafe extern fn() -> i32>>(b"my_open") {
            func();
         } else { return DTRACE_CONSUME_NEXT; }
      },
      Err(_e) => { return DTRACE_CONSUME_NEXT; }
   }

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

    info!("chewrec");

    if rec.is_null() {
        // Consume next record - DTRACE_CONSUME_NEXT
        trace!("consume next");
 
   match libloading::Library::new("../transport/tcp/target/debug/libddtrace_tcp.so") {
      Ok(lib) => {
         if let Ok(func) =lib.get::<libloading::Symbol<unsafe extern fn() -> i32>>(b"my_close") {
            func();
         } else { return DTRACE_CONSUME_NEXT; }
      },
      Err(_e) => { return DTRACE_CONSUME_NEXT; }
   }
        let handle = arg as *mut self::libdtrace::dtrace_hdl_t;
	xo_finish_h((* handle).dt_xo_hdl as *mut libxo::xo_handle_s);
        xo_destroy((* handle).dt_xo_hdl as *mut libxo::xo_handle_s);
        return DTRACE_CONSUME_NEXT;
    } else {
        let action = (* rec).dtrd_action;
		trace!("action = {}", action);
        if action == 2 { 
            // Consume next record - DTRACE_CONSUME_NEXT
            trace!("consume next");
            return DTRACE_CONSUME_NEXT;
        } else {
            // Consume this record - DTRACE_CONSUME_THIS
            trace!("consume this");
            return DTRACE_CONSUME_THIS;
        }
    }
}

