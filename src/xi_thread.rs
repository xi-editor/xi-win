use std::boxed;
use std::io::{self, BufRead, ErrorKind, Read, Write};
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};

use user32::PostMessageW;
use winapi::{HWND__, LPARAM, UINT, WM_USER, WPARAM};

use xi_core_lib;
use xi_rpc::RpcLoop;

pub const XI_FROM_CORE: UINT = WM_USER;
pub const XI_MAGIC: WPARAM = 0x7869;

pub struct XiPeer {
    tx: Sender<String>,
}

impl XiPeer {
    pub fn send(&self, s: String) {
        self.tx.send(s);
    }
}

pub fn start_xi_thread(hwnd: Arc<AtomicPtr<HWND__>>) -> XiPeer {
    let (to_core_tx, to_core_rx) = channel();
    let to_core_rx = ChanReader(to_core_rx);
    let from_core_tx = WinMsgWriter(hwnd);
    let mut state = xi_core_lib::MainState::new();
    let mut rpc_looper = RpcLoop::new(from_core_tx);
    thread::spawn(move ||
        rpc_looper.mainloop(|| to_core_rx, &mut state)
    );
    XiPeer { tx: to_core_tx }
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

struct WinMsgWriter(Arc<AtomicPtr<HWND__>>);

impl Write for WinMsgWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unreachable!("didn't expect xi-rpc to call write");
    }

    fn flush(&mut self) -> io::Result<()> {
        unreachable!("didn't expect xi-rpc to call flush");
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        let v = buf.to_vec();
        // could avoid the double boxing here by using wparam and lparam to represent
        // length and pointer, and do from_raw_parts trickery, but it's messy...

        // In any case, it's not an ideal protocol, it will leak buffers in flight
        // when a window is destroyed. Probably better to Arc<Mutex<Deque<String>>>
        // and just let the window message be a notification to check the queue.
        let bv = Box::new(v);
        let hwnd = self.0.load(Ordering::Acquire);
        if hwnd.is_null() {
            return Err(io::Error::new(ErrorKind::BrokenPipe, "hwnd destroyed"));
        }
        let ok = unsafe { PostMessageW(hwnd, XI_FROM_CORE, XI_MAGIC, Box::into_raw(bv) as LPARAM) };
        if ok != 0 {
            Ok(())
        } else {
            Err(io::Error::new(ErrorKind::Other, "couldn't PostMessage"))   
        }
    }
}
