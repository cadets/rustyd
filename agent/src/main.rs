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

#![deny(unused_mut)]
extern crate zookeeper;
#[macro_use]
extern crate log;
extern crate log4rs;
extern crate time;
extern crate rustc_serialize;
extern crate docopt;
extern crate errno;
extern crate libc;
extern crate kafka;
#[macro_use]
extern crate chan;
extern crate chan_signal;
extern crate dtrace_rust;

use dtrace_rust::instrument::InstrumentationThreadMessage;
use dtrace_rust::instrument::instrument_endpoint;
use docopt::Docopt;
use std::collections;
use std::default::Default;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;
use zookeeper::{acls, CreateMode, Watcher, WatchedEvent, WatchedEventType, ZkResult, ZkState, ZooKeeper};
use zookeeper::recipes::cache::{PathChildrenCache, PathChildrenCacheEvent};
use chan_signal::Signal;

extern {
   pub fn gethostname(name: *mut libc::c_char, size: libc::size_t)
      -> libc::c_int;
}

// Docopts usage specification
const USAGE: &'static str = "
DTrace Rust consumer

Usage:
    ddtrace_rust [options]

Options:
    -h, --help  Displays this message    
    -z <zookeeper_cluster>, --zookeeper <zookeeper_cluster>  Zookeeper cluster 
";

// Host agent information
const NAME: &'static str = env!("CARGO_PKG_NAME");
const VERSION: &'static str = env!("CARGO_PKG_VERSION");

// ZooKeeper paths
const DDTRACE_PATH: &'static str = "/ddtrace";
const DDTRACE_ENDPOINTS_PATH: &'static str = "/ddtrace/endpoints";
const DDTRACE_INSTRUMENTATION_PATH: &'static str = "/ddtrace/instrumentation";

struct InstrumentedEndpoint {
   instrumentation: Mutex<collections::HashMap<String, Instrumentation>>,
   name: String,
   zk: Arc<ZooKeeper>,
}

impl InstrumentedEndpoint {
   fn new(zk: Arc<ZooKeeper>) -> InstrumentedEndpoint {
      InstrumentedEndpoint {
         instrumentation: Mutex::new(collections::HashMap::new()),
         name: ddtrace_gethostname().unwrap(), 
         zk: zk,
      }
   }
}

struct Instrumentation {
   tx: mpsc::Sender<InstrumentationThreadMessage>,
   script: String,
}

#[derive(RustcDecodable)]
struct Args {
    flag_z: String,
}

struct LoggingWatcher;
impl Watcher for LoggingWatcher {
   fn handle(&self, event: WatchedEvent) {
      info!("{:?}", event);
   }
}

struct EndpointInstrumentationPathWatcher {
    endpoint : Arc<InstrumentedEndpoint>,
}

impl EndpointInstrumentationPathWatcher {
    pub fn new(endpoint: Arc<InstrumentedEndpoint>) -> EndpointInstrumentationPathWatcher {
        EndpointInstrumentationPathWatcher  {
           endpoint: endpoint,
       }
    }
}
  
impl Watcher for EndpointInstrumentationPathWatcher {
   fn handle(&self, event: WatchedEvent) {
      trace!("{:?}", event);
      match event.event_type {
         WatchedEventType::NodeCreated => {
             info!("endpoint node created");

         process_instrumentation(self.endpoint.clone());
         },
         WatchedEventType::NodeDeleted => {
             // TODO do I need to handle this?
             // Looks like if the node is delete I no longer get told about it being added
             // again
         }
         _ => { warn!("unhandled WatchedEvent {:?}", event); }
      }
   }
}

fn ddtrace_gethostname() -> Result<String, ()> {
   let len = 255;
   let mut buf = Vec::<u8>::with_capacity(len);
   let ptr = buf.as_mut_slice().as_mut_ptr();

   let err = unsafe {
      gethostname(ptr as *mut libc::c_char, len as libc::size_t)
   } as i32;
   match err {
      0 => {
         let mut real_len = len;
         let mut i = 0;
         loop {
            let byte = unsafe { *(((ptr as u64) + (i as u64)) as *const u8) };
            if byte == 0 {
               real_len = i;
               break;
            }
            i += 1;
         }
         unsafe { buf.set_len(real_len) }
         Ok(String::from_utf8_lossy(buf.as_slice()).into_owned())
      },
      _ => {
         Err(())
      }
   }
}

fn register_endpoint(endpoint: Arc<InstrumentedEndpoint>) -> ZkResult<String> {
    
    // Register endpoint in ZooKeeper
    // (The endpoint is registered as an ephemeral node, thus it serves
    // as an indication that the endpoint is alive and available to instrument)
    let hostname_path = format!("{}/{}", DDTRACE_ENDPOINTS_PATH, endpoint.name);
    let hostname_path_data = format!("{name} ({version})",
    name = NAME, version = VERSION).to_string().into_bytes();
        
    let value = try!(endpoint.zk.create(hostname_path.as_ref(), hostname_path_data,                
    acls::OPEN_ACL_UNSAFE.clone(),
    CreateMode::Ephemeral));
    Ok(value)
} 

fn process_instrumentation(endpoint: Arc<InstrumentedEndpoint>) -> ZkResult<()> { 

    // Process all instrumentation present in the endpoint's Zookeeper path 
    let endpoint_instrumentation_path = format!("{}/{}",
        DDTRACE_INSTRUMENTATION_PATH, endpoint.name);

    let mut pcc = PathChildrenCache::new(endpoint.zk.clone(),
        endpoint_instrumentation_path.as_ref()).unwrap();
    try!(pcc.start());
    info!("cache started {}", endpoint_instrumentation_path);

    let _pcc_subscription = pcc.add_listener(move |e| {
        match e {
            PathChildrenCacheEvent::ChildAdded(script, script_data) => {
                // TODO
                let script_str = String::from_utf8_lossy(
                    &script_data[..]).into_owned();
                let script_str_copy = script_str.clone();
                info!("received event {}", script_str_copy);

                // Start a new thread for the requested instrumentation 
                let (tx, rx): (mpsc::Sender<InstrumentationThreadMessage>,
                    mpsc::Receiver<InstrumentationThreadMessage>) = mpsc::channel();
                let builder = thread::Builder::new();       
                match builder.spawn(move || {
                    instrument_endpoint(script_str, rx); }) {
                    Ok(_child) => {
                        trace!("spawned instrumentation thread");

                        // Update the instrumentation managed by the endpoint
                        let instrumentation =
                        Instrumentation{tx: tx, script: script_str_copy};
                        endpoint.instrumentation.lock().unwrap().insert(script, instrumentation);
                    },
                    Err(e) => {
                        error!("Failed spawning thread to instrument endpoint{:?}", e)
                    }
                }; 
            },
            PathChildrenCacheEvent::ChildUpdated(
                _script, _script_data) => {
                // TODO
            },
            PathChildrenCacheEvent::ChildRemoved(script) => {
                // Stop instrumentation for the given script
                match endpoint.instrumentation.lock().unwrap().get(&script) {
                    Some(value) => {
                        value.tx.send(InstrumentationThreadMessage::Stop).unwrap();
                        endpoint.instrumentation.lock().unwrap().remove(&script);
                        info!("stopped {}", script);
                    },
                    None => {
                        warn!("{} not found", script);
                    }
                }
            },
            PathChildrenCacheEvent::Initialized(_data) => {
                trace!("Initialized Zookeeper PathCache"); 
            }
            _ => { trace!("PathChildrenCacheEvent {:?}", e); }
        }
    });
    Ok(())
}

fn main() {
   // Notify on SIGNINT and SIGTERM
   // (Note this must be done before any threads are spawned)
   let signal = chan_signal::notify(&[Signal::INT, Signal::TERM]);

   // Initialise the global logger
   log4rs::init_file("config/log.toml", Default::default()).unwrap();
 
   info!("initializing...");

   // Parse the command line arguments
   let args: Args = Docopt::new(USAGE)
      .and_then(|d| d.decode())
      .unwrap_or_else(|e| e.exit());

   // Create a connection to ZooKeeper
   info!("connecting to ZooKeeper {}", args.flag_z);
   match ZooKeeper::connect(&*args.flag_z, Duration::from_secs(5), LoggingWatcher) {
      Ok(zk) => {
         let endpoint_arc = Arc::new(
            InstrumentedEndpoint::new(Arc::new(zk)));

         // Register for changes in the ZooKeeper state
         // (currently unused, but could re-establish connections and so on)
         let zk_cleanup = endpoint_arc.zk.clone();
         let zk_listen_subscription =
            endpoint_arc.zk.clone().add_listener(move |state: ZkState| {
            match state {
               _ => { trace!("zkState = {:?}", state); }
            }
         });

         let endpoint = endpoint_arc.clone();
         match register_endpoint(endpoint) {
             Ok(value) => {

                 match process_instrumentation(endpoint_arc.clone()) {
                     Ok(_subscription) => {
                         info!("value {}", value);
                         loop {
                             chan_select! {
                                 // Await notified signals (SIGINT and SIGTERM)
                                 signal.recv() -> signal => {
                                     trace!("received signal SIG{:?}", signal.unwrap());
                                     break;
                                 },
                             }
                         }
                     },
                     Err(e) => {
                         error!("error registering endpoint with Zookeeper {:?}", e);
                     }
                 }
             },
             Err(e) => {
                 error!("error registering endpoint with Zookeeper {:?}", e);
             }
         }
                 
         // Remove ZooKeeper state listener
         info!("removing ZooKeeper connection state listener");
         zk_cleanup.remove_listener(zk_listen_subscription);

         // Close ZooKeeper handle
         info!("closing ZooKeeper connection");
         match zk_cleanup.close() {
            Ok(_) => {
                info!("closed Zookeeper connection")
            },
            Err(e) => {
                error!("failed closing ZooKeeper connection: {:?}", e);
            }
         }

         info!("cleanup finished");
      },
      Err(e) => {
         error!("could not connect to ZooKeeper {}: {:?}", args.flag_z, e);
      }
   }
}

