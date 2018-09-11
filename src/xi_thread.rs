// Copyright 2017 The xi-editor Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Startup and communication with the xi core thread.

use std::io::{self, BufRead, ErrorKind, Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
#[allow(unused_imports)]
use std::time::Duration;

// Needed for semaphore, currently disabled
//use winapi::um::synchapi::{CreateSemaphoreW, ReleaseSemaphore};
//use winapi::shared::ntdef::HANDLE;

use serde_json::{self, Value};

use xi_core_lib::XiCore;
use xi_rpc::RpcLoop;

pub struct XiPeer {
    tx: Sender<String>,
}

impl XiPeer {
    pub fn send(&self, s: String) {
        let _ = self.tx.send(s);
    }

    pub fn send_json(&self, v: &Value) {
        self.send(serde_json::to_string(v).unwrap());
    }
}

pub fn start_xi_thread() -> (XiPeer, Receiver<Value>) {
    let (to_core_tx, to_core_rx) = channel();
    let to_core_rx = ChanReader(to_core_rx);
    let (from_core_tx, from_core_rx) = channel();
    let from_core_tx = ChanWriter {
        sender: from_core_tx,
    };
    let mut state = XiCore::new();
    let mut rpc_looper = RpcLoop::new(from_core_tx);
    thread::spawn(move ||
        rpc_looper.mainloop(|| to_core_rx, &mut state)
    );
    let peer = XiPeer {
        tx: to_core_tx,
    };
    (peer, from_core_rx)
}

struct ChanReader(Receiver<String>);

impl Read for ChanReader {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        unreachable!("didn't expect xi-rpc to call read");
    }
}

// Note: we don't properly implement BufRead, only the stylized call patterns
// used by xi-rpc.
impl BufRead for ChanReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        unreachable!("didn't expect xi-rpc to call fill_buf");
    }

    fn consume(&mut self, _amt: usize) {
        unreachable!("didn't expect xi-rpc to call consume");
    }

    fn read_line(&mut self, buf: &mut String) -> io::Result<usize> {
        match self.0.recv() {
            Ok(s) => {
                buf.push_str(&s);
                Ok(s.len())
            }
            Err(_) => {
                Ok(0)
            }
        }
    }
}

struct ChanWriter {
    sender: Sender<Value>,
}

impl Write for ChanWriter {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        unreachable!("didn't expect xi-rpc to call write");
    }

    fn flush(&mut self) -> io::Result<()> {
        unreachable!("didn't expect xi-rpc to call flush");
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        let json = serde_json::from_slice::<Value>(buf).unwrap();
        //thread::sleep(Duration::from_secs(1));
        self.sender.send(json).map_err(|_|
            io::Error::new(ErrorKind::BrokenPipe, "rpc rx thread lost")
        )
    }
}

// We're not using the semaphore for now, but it might come in handy at
// some point.
/*
pub struct Semaphore(HANDLE);
unsafe impl Send for Semaphore {}

impl Semaphore {
    fn new() -> Semaphore {
        unsafe {
            let handle = CreateSemaphoreW(null_mut(), 0, 0xffff, null_mut());
            Semaphore(handle)
        }
    }

    // Note: this just leaks the semaphore, which is fine for this app,
    // but in general we'd want to use DuplicateHandle / CloseHandle
    fn clone(&self) -> Semaphore {
        Semaphore(self.0)
    }

    fn release(&self) {
        unsafe {
            let _ok = ReleaseSemaphore(self.0, 1, null_mut());
        }
    }

    pub fn get_handle(&self) -> HANDLE {
        self.0
    }
}
*/
