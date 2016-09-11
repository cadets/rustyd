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

extern {
   pub fn gethostname(name: *mut libc::c_char, size: libc::size_t)
      -> libc::c_int;
}

use dtrace_rust::instrument::InstrumentationThreadMessage;
use dtrace_rust::instrument::instrument_endpoint;
use docopt::Docopt;
use std::collections;
use std::default::Default;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;
use zookeeper::{CreateMode, Watcher, WatchedEvent, ZkState, ZooKeeper};
use zookeeper::acls;
use zookeeper::recipes::cache::{PathChildrenCache, PathChildrenCacheEvent};
use chan_signal::Signal;

// Docopts usage specification
const USAGE: &'static str = "
DTrace Rust consumer

Usage:
    ddtrace_rust [options]

Options:
    -h, --help  Displays this message    
    -b <kafka_cluster>, --broker <kafka_cluster>  Kafka cluster 
    -o <topic>, --output-topic <topic>  Kafka output topic
    -z <zookeeper_cluster>, --zookeeper <zookeeper_cluster>  Zookeeper cluster 
";

// Host agent information
const NAME: &'static str = env!("CARGO_PKG_NAME");
const VERSION: &'static str = env!("CARGO_PKG_VERSION");

// ZooKeeper 
const DDTRACE_PATH: &'static str = "/ddtrace";
const DDTRACE_INSTRUMENTATION_PATH: &'static str = "/ddtrace/instrumentation";

struct InstrumentedEndpoint {
   zk: Arc<ZooKeeper>,
   instrumentation: Mutex<collections::HashMap<String, Instrumentation>>,
   kafka_brokers: Vec<String>,
   kafka_output_topic: String,
}

impl InstrumentedEndpoint {
   fn new(zk: Arc<ZooKeeper>, brokers: Vec<String>, output_topic: String)
      -> InstrumentedEndpoint {
      InstrumentedEndpoint {
         zk: zk,
         instrumentation: Mutex::new(collections::HashMap::new()),
         kafka_brokers: brokers,
         kafka_output_topic: output_topic,
      }
   }
}

struct Instrumentation {
   tx: mpsc::Sender<InstrumentationThreadMessage>,
   script: String,
}

#[derive(RustcDecodable)]
struct Args {
    flag_b: String,
    flag_o: String,
    flag_z: String,
}

struct LoggingWatcher;
impl Watcher for LoggingWatcher {
   fn handle(&self, event: WatchedEvent) {
      info!("{:?}", event);
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

fn zk_connected(endpoint: Arc<InstrumentedEndpoint>) {
   info!("ZKState = Connected");                 
   thread::spawn(move || { 
      // Check that the endpoint is present in ZooKeeper
      match endpoint.zk.clone().exists_w(
         DDTRACE_INSTRUMENTATION_PATH, LoggingWatcher) {
         Ok(_stat) => {
            info!("{} path registered in ZooKeeper",
               DDTRACE_INSTRUMENTATION_PATH);
            register_endpoint(endpoint);
         },
         Err(e) => {
            error!("failed ZooKeeper: {:?}", e);
         }
      }
   });
} 

fn register_endpoint(endpoint: Arc<InstrumentedEndpoint>) {

   // Register endpoint in ZooKeeper
   // (The endpoint is registered as an ephemeral node, thus it serves
   // as an indication that the endpoint is alive and available to
   // instrument)
   let hostname = ddtrace_gethostname().unwrap();

   let hostname_path = format!("{}/{}", DDTRACE_PATH, hostname);
   let hostname_path_data = format!("{name} ({version})",
      name = NAME, version = VERSION).to_string().into_bytes();
                 
   match endpoint.zk.clone().create(
      hostname_path.as_ref(),
      hostname_path_data,                  
      acls::OPEN_ACL_UNSAFE.clone(),
      CreateMode::Ephemeral) {
      Ok(_) => {
         info!("registered {} with ZooKeeper", hostname);
         process_instrumentation(endpoint);
      },
      Err(e) => {
         error!("failed registering endpoint with ZooKeeper {:?}", e);
      }
   }
}

fn process_instrumentation(endpoint: Arc<InstrumentedEndpoint>) {

   let mut pcc = PathChildrenCache::new(endpoint.zk.clone(),
      DDTRACE_INSTRUMENTATION_PATH).unwrap();
   match pcc.start() {
      Err(err) => {
         error!("failed starting cache: {:?}", err);
         return;
      }
      _ => {
         info!("cache started");
      }
   }

   let (ev_tx, ev_rx) = mpsc::channel();
   pcc.add_listener(move |e| {
      match ev_tx.send(e) {
         Err(e) => { error!("{}", e); },
         _ => {}
      }
   });

   for ev in ev_rx {
      info!("received event {:?}", ev);
      match ev {
         PathChildrenCacheEvent::ChildAdded(
            script, script_data) => {
            let script_str =
               String::from_utf8_lossy(&script_data[..]).into_owned();
            let script_str_copy = script_str.clone();

            // Start a new thread for the requested
            // instrumentation 
            let (tx, rx): (mpsc::Sender<InstrumentationThreadMessage>, mpsc::Receiver<InstrumentationThreadMessage>) = mpsc::channel();
            let _child = thread::spawn(move || {
               instrument_endpoint(script_str, rx);
             }); 

             // Update the instrumentation managed
             // by the endpoint
             let instrumentation =
                Instrumentation{tx: tx, script: script_str_copy};
             endpoint.instrumentation.lock().unwrap()
                .insert(script, instrumentation);
          },
          PathChildrenCacheEvent::ChildUpdated(
             _script, _script_data) => {
          // TODO - do I want to support this
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
                   error!("{} not found", script);
                }
             }
          },
          _ => { }
       }
    }
}

fn main() {
   // Notify on SIGNINT and SIGTERM
   // (Note this must be done before and threads are spawned)
   let signal = chan_signal::notify(&[Signal::INT, Signal::TERM]);

   // Initialise the global logger
   log4rs::init_file("config/log.toml", Default::default()).unwrap();
 
   info!("initializing...");

   // Parse the command line arguments
   let args: Args = Docopt::new(USAGE)
      .and_then(|d| d.decode())
      .unwrap_or_else(|e| e.exit());

   // Construct the instrumented endpoint
   // TODO: The Kafka conf should also be in ZooKeeper
   let brokers = args.flag_b.split(",").map(|x| x.to_owned())
      .collect::<Vec<String>>();
   let output_topic = args.flag_o;

   // Create a connection to ZooKeeper
   info!("connecting to ZooKeeper {}", args.flag_z);
   match ZooKeeper::connect(&*args.flag_z, Duration::from_secs(5),
      LoggingWatcher) {
      Ok(zk) => {
         let endpoint_arc = Arc::new(
            InstrumentedEndpoint::new(Arc::new(zk), brokers, output_topic));

         // Register for changes in the ZooKeeper state
         let zk_cleanup = endpoint_arc.zk.clone();
         let zk_listen_subscription =
            endpoint_arc.zk.clone().add_listener(move |state: ZkState| {
            match state {
               ZkState::Connected => {

                  let endpoint = endpoint_arc.clone();
                  zk_connected(endpoint);
               },
               _ => { info!("ZKState = {:?}", state) }
            }
         });

         loop {
            chan_select! {
               // Await notified signals (SIGINT and SIGTERM)
               signal.recv() -> signal => {
                  info!("received signal SIG{:?}", signal.unwrap());
                  break;
               },
            }
         }
                 
         // Remove ZooKeeper state listener
         info!("removing ZooKepper state listener");
         zk_cleanup.remove_listener(zk_listen_subscription);

         // Close ZooKeeper handle
         info!("closing ZooKepper connection");
         match zk_cleanup.close() {
            Ok(_) => { },
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

