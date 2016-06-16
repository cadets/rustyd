
#[macro_use]
extern crate log;
extern crate fern;
extern crate time;
extern crate libc;
extern crate rustc_serialize;
extern crate docopt;

use kafka::consumer::{Consumer, FetchOffset};
use kafka::error::Error as KafkaError;
use libc::{c_char, c_int};
use std::ffi::{CString, CStr};
use std::process::exit;
use std::default::Default;
use docopt::Docopt;

mod libdtrace;

const USAGE: &'static str = "
DTrace Rust consumer

Usage:
    ddtrace_rust [options]

Options:
    -h, --help    Displays this message    
    -n SCRIPT     DLangauge script
";

#[derive(RustcDecodable)]
struct Args {
    flag_n: String,
}

fn dtrace_open(version: i32, flags: i32)
    -> (*mut libdtrace::dtrace_hdl_t, i32) {

    let mut err: libc::c_int = 0;    
    let handle = unsafe {
        libdtrace::dtrace_open(version, flags, &mut err)
    };
    (handle, err)
}

fn dtrace_setopt(handle: *mut libdtrace::dtrace_hdl_t,
    opt: &'static str, val: &'static str) {

    unsafe {
        libdtrace::dtrace_setopt(handle,
            CString::new(opt).unwrap().as_ptr(),
            CString::new(val).unwrap().as_ptr());
    }
}

fn dtrace_errmsg(handle: *mut libdtrace::dtrace_hdl_t, err: i32) -> String {

    unsafe {
        CStr::from_ptr(libdtrace::dtrace_errmsg(handle, err))
             .to_string_lossy().into_owned()
    }
}

fn dtrace_program_strcompile(handle: *mut libdtrace::dtrace_hdl_t,
    script: & str, spec: libdtrace::dtrace_probespec_t, cflags: u32)
    -> *mut libdtrace::dtrace_prog_t {

    let args = std::env::args().map(|arg| CString::new(arg).unwrap())
        .collect::<Vec<CString>>();
    let c_args = args.iter().map(|arg| arg.as_ptr())
        .collect::<Vec<*const c_char>>();
    unsafe {
        libdtrace::dtrace_program_strcompile(handle,
            CString::new(script).unwrap().as_ptr(), spec,
            cflags, c_args.len() as c_int, c_args.as_ptr())
    }
}
    
fn dtrace_program_exec(handle: *mut libdtrace::dtrace_hdl_t,
    prog: *mut libdtrace::dtrace_prog_t,
    info: *mut libdtrace::dtrace_proginfo_t) -> i32 {

    unsafe {
        libdtrace::dtrace_program_exec(handle, prog, info)
    }
}

fn dtrace_go(handle: *mut libdtrace::dtrace_hdl_t) -> i32 {
    unsafe {
        libdtrace::dtrace_go(handle)
    }
}

fn dtrace_work(handle: *mut libdtrace::dtrace_hdl_t,
    fp: *mut libdtrace::__sFILE, pfunc: libdtrace::dtrace_consume_probe_f,
    rfunc: libdtrace::dtrace_consume_rec_f, arg: *mut std::os::raw::c_void)
    -> libdtrace::dtrace_workstatus_t{
    unsafe {
        //libdtrace::dtrace_work(handle, fp, pfunc, rfunc, arg)
        libdtrace::dtrace_work(handle, libdtrace::__stdoutp, pfunc, rfunc, arg)
    }
}

fn dtrace_sleep(handle: *mut libdtrace::dtrace_hdl_t) {
    unsafe {
        libdtrace::dtrace_sleep(handle)
    }
}

fn dtrace_stop(handle: *mut libdtrace::dtrace_hdl_t) -> i32 {
    unsafe {
        libdtrace::dtrace_stop(handle)
    }
}

fn dtrace_close(handle: *mut libdtrace::dtrace_hdl_t) {
    unsafe {
        libdtrace::dtrace_close(handle)
    }
}

fn dtrace_errno(handle: *mut libdtrace::dtrace_hdl_t) -> i32 {
    unsafe {
        libdtrace::dtrace_errno(handle)
    }
}

fn dtrace_handle_buffered(handle: *mut libdtrace::dtrace_hdl_t,
    hdlr: libdtrace::dtrace_handle_buffered_f,
    arg: *mut ::std::os::raw::c_void) -> i32 {
    unsafe {
        libdtrace::dtrace_handle_buffered(handle, hdlr, arg)
    }
}

fn main() {

    // Initialise the global logger
    let logger_config = fern::DispatchConfig {
       format: Box::new(|msg: &str, level: &log::LogLevel,
       _location: &log::LogLocation| {
       format!("[{}][{}] {}",
       time::now().strftime("%Y-%m-%d][%H:%M:%S").unwrap(), level, msg)
       }),
       output: vec![fern::OutputConfig::stdout(),
       fern::OutputConfig::file("output.log")],
       level: log::LogLevelFilter::Trace,
    };

    if let Err(e) = fern::init_global_logger(logger_config,
        log::LogLevelFilter::Trace) {
        panic!("Failed to initialize global logger: {}", e);
    }
    
    info!("dtrace initializing...");

    // Parse the command line arguments
/*
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());
*/
    
    let dtrace_version = 3;
    let flags = 0;
    let (handle, err) = dtrace_open(dtrace_version, flags);
    if err != 0 {
        error!("dtrace error {} initializing", dtrace_errmsg(handle, err));
        exit(1);
    }
    info!("dtrace initialized");
    
    //dtrace_setopt(handle, "oformat", "json");
    dtrace_setopt(handle, "bufsize", "4m");
    dtrace_setopt(handle, "aggsize", "4m");
    dtrace_setopt(handle, "temporal", "4m");
    info!("dtrace options set");

    let prog = dtrace_program_strcompile(handle, "BEGIN {print(\"Hello\");}",
        libdtrace::dtrace_probespec::DTRACE_PROBESPEC_NAME, 0x0080);
    if prog.is_null() {
        error!("failed to compile dtrace program {}",
           dtrace_errmsg(handle, dtrace_errno(handle)));
        exit(1);
    }
    info!("dtrace program compiled");
/*
    if dtrace_handle_buffered(handle, buffered_handler,
        std::ptr::null_mut()) == -1 {
        error!("failed to register dtrace buffered handler");
        exit(1);
    }
*/
    let mut info: libdtrace::dtrace_proginfo_t = Default::default();
    let status = dtrace_program_exec(handle, prog, &mut info);
    if status == -1 {
        error!("failed to eanble dtrace probes {}",
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

    let mut done = 0;
    while {
        if done == 0 {
           dtrace_sleep(handle);
        }

        info!("dtrace work...");
        match dtrace_work(handle, std::ptr::null_mut(), chew, chewrec,
            std::ptr::null_mut()) {
            libdtrace::dtrace_workstatus_t::DTRACE_WORKSTATUS_ERROR => {
                if dtrace_errno(handle) != 9959 {
                    error!("{}", dtrace_errmsg(handle, dtrace_errno(handle)));
                    done = 1;
                }
            }
	    libdtrace::dtrace_workstatus_t::DTRACE_WORKSTATUS_OKAY => {
                done = 0;
            }
            libdtrace::dtrace_workstatus_t::DTRACE_WORKSTATUS_DONE => {
                done = 1;
            }
        }
        done == 0
    } {}

    info!("dtrace stopping");
    dtrace_stop(handle);

    info!("dtrace closing");
    dtrace_close(handle);
}

unsafe extern fn buffered_handler(bufdata : *const libdtrace::dtrace_bufdata_t,
    arg: *mut ::std::os::raw::c_void) -> i32 {

    info!("buffered_handler");
    0
}

unsafe extern fn chew(data: *const libdtrace::dtrace_probedata_t,
    arg: *mut std::os::raw::c_void) -> i32 {
    
    info!("chew");
    let pd: *mut libdtrace::dtrace_probedesc_t = (* data).dtpda_pdesc;
    trace!("id = {}", (* pd).dtpd_id);
    trace!("func = {:?}", CStr::from_ptr((* pd).dtpd_func.as_ptr()));
    trace!("name = {:?}", CStr::from_ptr((* pd).dtpd_name.as_ptr()));

    // Consume this record - DTRACE_CONSUME_THIS
    0
}

unsafe extern fn chewrec(data: *const libdtrace::dtrace_probedata_t,
    rec: *const libdtrace::dtrace_recdesc_t, arg: *mut std::os::raw::c_void)
    -> i32 {

    info!("chewrec");

    //if rec == std::ptr::null_mut() {
    if rec.is_null() {
        // Consume next record - DTRACE_CONSUME_NEXT
        trace!("consume next");
        1
    } else {
        let action = (* rec).dtrd_action;
	trace!("action = {}", action);
        if action  == 2 { 
            // Consume next record - DTRACE_CONSUME_NEXT
            trace!("consume next");
            1
        } else {
            // Consume this record - DTRACE_CONSUME_THIS
            trace!("consume this");
            0
        }
    }
}

