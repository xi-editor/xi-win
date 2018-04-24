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
#[macro_use]
extern crate xi_win_shell;

mod dialog;
mod edit_view;
mod linecache;
mod menus;
mod rpc;
mod xi_thread;

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use winapi::shared::windef::*;

use serde_json::Value;

use edit_view::EditView;
use menus::MenuEntries;
use rpc::{Core, Handler};
use xi_win_shell::util::Error;
use dialog::{get_open_file_dialog_path, get_save_file_dialog_path};
use xi_thread::start_xi_thread;

use xi_win_shell::paint::PaintCtx;
use xi_win_shell::win_main::{self, RunLoopHandle};
use xi_win_shell::window::{IdleHandle, MouseButton, MouseType, WindowBuilder, WindowHandle,
    WinHandler};

struct MainWinState {
    edit_view: EditView,
}

impl MainWinState {
    fn new() -> MainWinState {
        MainWinState {
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
    core: RefCell<Core>,
    handle: RefCell<WindowHandle>,
    state: RefCell<MainWinState>,
}

impl MainWin {
    fn new(core: Core, state: MainWinState) -> MainWin {
        MainWin {
            core: RefCell::new(core),
            handle: Default::default(),
            state: RefCell::new(state),
        }
    }

    fn send_notification(&self, method: &str, params: &Value) {
        self.core.borrow().send_notification(method, params);
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

    // TODO: arguably these should be moved to MainWinHandler to avoid the need
    // for the parent reference.
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

            x if x == MenuEntries::Undo as u32 => {
                self.win.state.borrow_mut().edit_view.undo(&self.win);
            }
            x if x == MenuEntries::Redo as u32 => {
                self.win.state.borrow_mut().edit_view.redo(&self.win);
            }
            // TODO: cut, copy, paste (requires pasteboard)
            x if x == MenuEntries::UpperCase as u32 => {
                self.win.state.borrow_mut().edit_view.upper_case(&self.win);
            }
            x if x == MenuEntries::LowerCase as u32 => {
                self.win.state.borrow_mut().edit_view.lower_case(&self.win);
            }
            x if x == MenuEntries::Transpose as u32 => {
                self.win.state.borrow_mut().edit_view.transpose(&self.win);
            }

            x if x == MenuEntries::AddCursorAbove as u32 => {
                self.win.state.borrow_mut().edit_view.add_cursor_above(&self.win);
            }
            x if x == MenuEntries::AddCursorBelow as u32 => {
                self.win.state.borrow_mut().edit_view.add_cursor_below(&self.win);
            }
            x if x == MenuEntries::SingleSelection as u32 => {
                self.win.state.borrow_mut().edit_view.single_selection(&self.win);
            }
            x if x == MenuEntries::SelectAll as u32 => {
                self.win.state.borrow_mut().edit_view.select_all(&self.win);
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

    fn mouse(&self, x: i32, y: i32, mods: u32, which: MouseButton, ty: MouseType) {
        let (x_px, y_px) = self.win.handle.borrow().pixels_to_px_xy(x, y);
        let edit_view = &mut self.win.state.borrow_mut().edit_view;
        edit_view.mouse(x_px, y_px, mods, which, ty, &self.win);
    }

    fn mouse_wheel(&self, delta: i32, mods: u32) {
        let edit_view = &mut self.win.state.borrow_mut().edit_view;
        edit_view.mouse_wheel(delta, mods, &self.win)
    }

    fn destroy(&self) {
        win_main::request_quit();
    }

    fn as_any(&self) -> &Any { self }
}

impl MainWin {
    fn req_new_view(&self, filename: Option<&str>) {
        let mut params = json!({});
        if let Some(filename) = filename {
            params["file_path"] = json!(filename);
        }
        let handle = self.handle.borrow().get_idle_handle().unwrap();
        self.core.borrow_mut().send_request("new_view", &params,
            move |value| {
                let value = value.clone();
                handle.add_idle(move |a| {
                    let handler = a.downcast_ref::<MainWinHandler>().unwrap();
                    let edit_view = &mut handler.win.state.borrow_mut().edit_view;
                    edit_view.set_view_id(value.as_str().unwrap());
                });
            }
        );
    }

    fn handle_cmd(&self, method: &str, params: &Value) {
        let mut state = self.state.borrow_mut();
        match method {
            "update" => state.edit_view.apply_update(&params["update"]),
            "scroll_to" => state.edit_view.scroll_to(params["line"].as_u64().unwrap() as usize),
            "available_themes" => (), // TODO
            "available_plugins" => (), // TODO
            "config_changed" => (), // TODO
            _ => println!("unhandled core->fe method {}", method),
        }
        // TODO: edit view should probably handle this logic
        self.invalidate();
    }

    pub fn invalidate(&self) {
        self.handle.borrow().invalidate();
    }
}

fn create_main(core: Core) -> Result<WindowHandle, Error> {
    let main_state = MainWinState::new();
    let main_win = Rc::new(MainWin::new(core, main_state));
    let main_win_handler = MainWinHandler {
        win: main_win,
    };

    let menubar = menus::create_menus();

    let mut builder = WindowBuilder::new();
    builder.set_handler(Box::new(main_win_handler));
    builder.set_title("xi-editor");
    builder.set_menu(menubar);
    let window = builder.build().unwrap();
    Ok(window)
}

#[derive(Clone)]
struct MyHandler {
    runloop: RunLoopHandle,
    win_handle: Arc<Mutex<Option<IdleHandle>>>,
}

impl MyHandler {
    fn new(runloop: RunLoopHandle) -> MyHandler {
        MyHandler {
            runloop,
            win_handle: Default::default(),
        }
    }
}

impl Handler for MyHandler {
    fn notification(&self, method: &str, params: &Value) {
        if let Some(idle_handle) = self.win_handle.lock().unwrap().as_ref() {
            // Note: could pass these by ownership, but we'll change where they get parsed.
            let method = method.to_owned();
            let params = params.clone();
            idle_handle.add_idle(move |a| {
                let handler = a.downcast_ref::<MainWinHandler>().unwrap();
                handler.win.handle_cmd(&method, &params);
            });
        }
    }
}

fn main() {
    xi_win_shell::init();

    let (xi_peer, rx) = start_xi_thread();

    let mut runloop = win_main::RunLoop::new();
    menus::set_accel(&mut runloop);
    let handler = MyHandler::new(runloop.get_handle());
    let core = Core::new(xi_peer, rx, handler.clone());
    let window = create_main(core).unwrap();
    *handler.win_handle.lock().unwrap() = window.get_idle_handle();
    window.show();
    runloop.run();
}
