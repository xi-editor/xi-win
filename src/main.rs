// Copyright 2017 Google Inc. All rights reserved.
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

//! The main module for the xi editor front end.


// NOTE: This disables stdout, so no println :(
// TODO: If we checked what GetStdHandle returns for stdout and see
// that it is an invalid handle (either -1 or 0), then we can open up
// up a file to log stdout and SetStdHandle.
#![windows_subsystem = "windows"]

#[macro_use]
extern crate winapi;
extern crate direct2d;
extern crate directwrite;

extern crate serde;
#[macro_use]
extern crate serde_json;

extern crate xi_core_lib;
extern crate xi_rpc;
extern crate xi_win_shell;

mod dialog;
mod edit_view;
mod linecache;
mod menus;
mod xi_thread;

use std::cell::RefCell;
use std::sync::mpsc::TryRecvError;
use std::rc::Rc;

use winapi::shared::windef::*;

use serde_json::Value;

use edit_view::EditView;
use menus::MenuEntries;
use xi_win_shell::util::Error;
use dialog::{get_open_file_dialog_path, get_save_file_dialog_path};
use xi_thread::{start_xi_thread, XiPeer};

use xi_win_shell::paint::PaintCtx;
use xi_win_shell::win_main::{self, RunLoopHandle};
use xi_win_shell::window::{WindowBuilder, WindowHandle, WinHandler};

struct MainWinState {
    rpc_id: usize,
    label: String,
    edit_view: EditView,
}

impl MainWinState {
    fn new() -> MainWinState {
        MainWinState {
            rpc_id: 0,
            label: "hello direct2d".to_string(),
            edit_view: EditView::new(),
        }
    }

    fn render(&mut self, p: &mut PaintCtx) {
        self.edit_view.render(p);
    }
}

struct MainWinHandler {
    win: Rc<MainWin>,
}

// Maybe combine all this, put as a single item inside a RefCell.
pub struct MainWin {
    peer: XiPeer,
    handle: RefCell<WindowHandle>,
    state: RefCell<MainWinState>,
}

impl MainWin {
    fn new(peer: XiPeer, state: MainWinState) -> MainWin {
        MainWin {
            peer: peer,
            handle: Default::default(),
            state: RefCell::new(state),
        }
    }

    fn send_notification(&self, method: &str, params: &Value) {
        let cmd = json!({
            "method": method,
            "params": params,
        });
        self.peer.send_json(&cmd);
    }

    // Note: caller can't be borrowing the state.
    fn send_edit_cmd(&self, method: &str, params: &Value, view_id: &str) {
        let edit_params = json!({
            "method": method,
            "params": params,
            "view_id": view_id,
        });
        self.send_notification("edit", &edit_params);
    }

    fn file_open(&self, hwnd_owner: HWND) {
        let filename = unsafe { get_open_file_dialog_path(hwnd_owner) };
        if let Some(filename) = filename {
            self.req_new_view(Some(&filename));
            let mut state = self.state.borrow_mut();
            state.edit_view.filename = Some(filename);
            state.edit_view.clear_line_cache();
        }
    }

    fn file_save(&self, hwnd_owner: HWND) {
        let filename: Option<String> = self.state.borrow_mut().edit_view.filename.clone();
        if filename.is_none() {
            self.file_save_as(hwnd_owner);
        } else {
            let state = self.state.borrow_mut();
            self.send_notification("save", &json!({
                "view_id": state.edit_view.view_id,
                "file_path": filename,
            }));
        }
    }

    fn file_save_as(&self, hwnd_owner: HWND) {
        if let Some(filename) = unsafe { get_save_file_dialog_path(hwnd_owner) } {
            let mut state = self.state.borrow_mut();
            self.send_notification("save", &json!({
                "view_id": state.edit_view.view_id,
                "file_path": filename,
            }));

            state.edit_view.filename = Some(filename.clone());
        }
    }

    fn req_new_view(&self, filename: Option<&str>) {
        let mut params = json!({});
        if let Some(filename) = filename {
            params["file_path"] = json!(filename);
        }
        let cmd = json!({
            "method": "new_view",
            "params": params,
            "id": self.state.borrow().rpc_id,
        });
        self.state.borrow_mut().rpc_id += 1;
        self.peer.send_json(&cmd);
    }
}

impl WinHandler for MainWinHandler {
    fn connect(&self, handle: &WindowHandle) {
        *self.win.handle.borrow_mut() = handle.clone();
        self.win.send_notification("client_started", &json!({}));
        self.win.req_new_view(None);
    }

    fn size(&self, x: u32, y: u32) {
        let (x_px, y_px) = self.win.handle.borrow().pixels_to_px_xy(x, y);
        self.win.state.borrow_mut().edit_view.size(x_px, y_px);
    }

    fn paint(&self, paint_ctx: &mut PaintCtx) -> bool {
        let mut state = self.win.state.borrow_mut();
        state.render(paint_ctx);
        false
    }

    fn rebuild_resources(&self) {
        let mut state = self.win.state.borrow_mut();
        state.edit_view.rebuild_resources();
    }

    fn command(&self, id: u32) {
        match id {
            x if x == MenuEntries::Exit as u32 => {
                self.win.handle.borrow().close();
            }
            x if x == MenuEntries::Open as u32 => {
                let hwnd = self.win.handle.borrow().get_hwnd().unwrap();
                self.win.file_open(hwnd);
            }
            x if x == MenuEntries::Save as u32 => {
                let hwnd = self.win.handle.borrow().get_hwnd().unwrap();
                self.win.file_save(hwnd);
            }
            x if x == MenuEntries::SaveAs as u32 => {
                let hwnd = self.win.handle.borrow().get_hwnd().unwrap();
                self.win.file_save_as(hwnd);
            }
            _ => println!("unexpected id {}", id),
        }
    }

    fn char(&self, ch: u32, mods: u32) {
        let edit_view = &mut self.win.state.borrow_mut().edit_view;
        edit_view.char(ch, mods, &self.win);
    }

    fn keydown(&self, vk_code: i32, mods: u32) -> bool {
        let edit_view = &mut self.win.state.borrow_mut().edit_view;
        edit_view.keydown(vk_code, mods, &self.win)
    }

    fn mouse_wheel(&self, delta: i32, mods: u32) {
        let edit_view = &mut self.win.state.borrow_mut().edit_view;
        edit_view.mouse_wheel(delta, mods, &self.win)
    }

    fn destroy(&self) {
        win_main::request_quit();
    }
}

impl MainWin {
    fn handle_cmd(&self, v: &Value) {
        let mut state = self.state.borrow_mut();
        //println!("got {:?}", v);
        if let Some(tab_name) = v["result"].as_str() {
            // TODO: should match up id etc. This is quick and dirty.
            state.edit_view.set_view_id(tab_name);
        } else {
            let ref method = v["method"];
            if method == "update" {
                state.edit_view.apply_update(&v["params"]["update"]);
            }
        }
        state.label = serde_json::to_string(v).unwrap();
        self.handle.borrow().invalidate();
    }
}

fn create_main(xi_peer: XiPeer, run_loop: RunLoopHandle)
    -> Result<(WindowHandle, Rc<MainWin>), Error>
{
    let main_state = MainWinState::new();
    let main_win = Rc::new(MainWin::new(xi_peer, main_state));
    let main_win_handler = MainWinHandler {
        win: main_win.clone(),
    };

    let menubar = menus::create_menus();

    let mut builder = WindowBuilder::new(run_loop);
    builder.set_handler(Box::new(main_win_handler));
    builder.set_title("xi-editor");
    builder.set_menu(menubar);
    let window = builder.build().unwrap();
    Ok((window, main_win))
}

fn main() {
    xi_win_shell::init();

    let (xi_peer, rx, semaphore) = start_xi_thread();

    let mut run_loop = win_main::RunLoop::new();
    let (window, main_win) = create_main(xi_peer, run_loop.get_handle()).unwrap();
    window.show();
    let run_handle = run_loop.get_handle();
    unsafe {
        run_handle.add_handler(semaphore.get_handle(), move || {
            loop {
                match rx.try_recv() {
                    Ok(v) => main_win.handle_cmd(&v),
                    Err(TryRecvError::Disconnected) => {
                        println!("core disconnected");
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
        });
    }
    run_loop.run();
}
