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

mod linecache;
mod menus;
mod dialog;
mod xi_thread;

use std::cell::RefCell;
use std::sync::mpsc::TryRecvError;
use std::rc::Rc;

use winapi::shared::windef::*;
use winapi::um::winuser::*;

use direct2d::brush;
use direct2d::math::*;
use direct2d::render_target::DrawTextOption;
use directwrite::text_format::{self, TextFormat};
use directwrite::text_layout::{self, TextLayout};

use serde_json::Value;

use linecache::LineCache;
use menus::MenuEntries;
use xi_win_shell::util::Error;
use dialog::{get_open_file_dialog_path, get_save_file_dialog_path};
use xi_thread::{start_xi_thread, XiPeer};

use xi_win_shell::paint::PaintCtx;
use xi_win_shell::win_main;
use xi_win_shell::window::{WindowBuilder, WindowHandle, WinHandler};

struct Resources {
    fg: brush::SolidColor,
    bg: brush::SolidColor,
    text_format: TextFormat,
}

impl Resources {
    fn create_text_layout(&self, factory: &directwrite::Factory, text: &str) -> TextLayout {
        let params = text_layout::ParamBuilder::new()
            .text(text)
            .font(self.text_format.clone())
            .width(1e6)
            .height(1e6)
            .build().unwrap();
        factory.create(params).unwrap()
    }
}

struct MainWinState {
    rpc_id: usize,
    view_id: String,
    line_cache: LineCache,
    label: String,
    dwrite_factory: directwrite::Factory,
    resources: Option<Resources>,
    filename: Option<String>,
}

impl MainWinState {
    fn new() -> MainWinState {
        MainWinState {
            rpc_id: 0,
            view_id: String::new(),
            line_cache: LineCache::new(),
            label: "hello direct2d".to_string(),
            dwrite_factory: directwrite::Factory::new().unwrap(),
            resources: None,
            filename: None
        }
    }

    fn create_resources(&mut self, p: &mut PaintCtx) -> Resources {
        let rt = p.render_target();
        let text_format_params = text_format::ParamBuilder::new()
            .size(15.0)
            .family("Consolas")
            .build().unwrap();
        let text_format = self.dwrite_factory.create(text_format_params).unwrap();
        Resources {
            fg: rt.create_solid_color_brush(0xf0f0ea, &BrushProperties::default()).unwrap(),
            bg: rt.create_solid_color_brush(0x272822, &BrushProperties::default()).unwrap(),
            text_format: text_format,
        }
    }

    fn render(&mut self, p: &mut PaintCtx) {
        if self.resources.is_none() {
            self.resources = Some(self.create_resources(p));
        }
        let resources = &self.resources.as_ref().unwrap();
        let rt = p.render_target();
        let size = rt.get_size();
        let rect = RectF::from((0.0, 0.0, size.width, size.height));
        rt.fill_rectangle(&rect, &resources.bg);

        let x0 = 6.0;
        let mut y = 6.0;
        for line_num in 0..self.line_cache.height() {
            if let Some(line) = self.line_cache.get_line(line_num) {
                let layout = resources.create_text_layout(&self.dwrite_factory, line.text());
                rt.draw_text_layout(
                    &Point2F::from((x0, y)),
                    &layout,
                    &resources.fg,
                    &[DrawTextOption::EnableColorFont]
                );
                for &offset in line.cursor() {
                    if let Some(pos) = layout.hit_test_text_position(offset as u32, true) {
                        let x = x0 + pos.point_x;
                        rt.draw_line(&Point2F::from((x, y)),
                            &Point2F::from((x, y + 17.0)),
                            &resources.fg, 1.0, None);
                    }
                }
            }
            y += 17.0;
        }
    }
}

struct MainWinHandler {
    win: Rc<MainWin>,
}

// Maybe combine all this, put as a single item inside a RefCell.
struct MainWin {
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
    fn send_edit_cmd(&self, method: &str, params: &Value) {
        let view_id = &self.state.borrow_mut().view_id;
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
            state.filename = Some(filename);
            state.line_cache = LineCache::new();
        }
    }

    fn file_save(&self, hwnd_owner: HWND) {
        let filename: Option<String> = self.state.borrow_mut().filename.clone();
        if filename.is_none() {
            self.file_save_as(hwnd_owner);
        } else {
            let state = self.state.borrow_mut();
            self.send_notification("save", &json!({
                "view_id": state.view_id,
                "file_path": filename,
            }));
        }
    }

    fn file_save_as(&self, hwnd_owner: HWND) {
        if let Some(filename) = unsafe { get_save_file_dialog_path(hwnd_owner) } {
            let mut state = self.state.borrow_mut();
            self.send_notification("save", &json!({
                "view_id": state.view_id,
                "file_path": filename,
            }));

            state.filename = Some(filename.clone());
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

    fn paint(&self, paint_ctx: &mut PaintCtx) {
        let mut state = self.win.state.borrow_mut();
        state.render(paint_ctx);
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

    fn char(&self, ch: u32) {
        match ch {
            0x08 => {
                self.win.send_edit_cmd("delete_backward", &json!([]));
            },
            0x0d => {
                self.win.send_edit_cmd("insert_newline", &json!([]));
            },
            _ => {
                if let Some(c) = ::std::char::from_u32(ch) {
                    let params = json!({"chars": c.to_string()});
                    self.win.send_edit_cmd("insert", &params);
                }
            }
        }
    }

    fn keydown(&self, vk_code: i32) {
        // Handle special keys here
        match vk_code {
            VK_UP => {
                self.win.send_edit_cmd("move_up", &json!([]));
            },
            VK_DOWN => {
                self.win.send_edit_cmd("move_down", &json!([]));
            },
            VK_LEFT => {
                self.win.send_edit_cmd("move_left", &json!([]));
            },
            VK_RIGHT => {
                self.win.send_edit_cmd("move_right", &json!([]));
            },
            VK_DELETE => {
                self.win.send_edit_cmd("delete_forward", &json!([]));
            },
            _ => ()
        }
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
            state.view_id = tab_name.to_string();
        } else {
            let ref method = v["method"];
            if method == "update" {
                state.line_cache.apply_update(&v["params"]["update"]);
            }
        }
        state.label = serde_json::to_string(v).unwrap();
        self.handle.borrow().invalidate();
    }
}

fn create_main(xi_peer: XiPeer) -> Result<(WindowHandle, Rc<MainWin>), Error> {
    let main_state = MainWinState::new();
    let main_win = Rc::new(MainWin::new(xi_peer, main_state));
    let main_win_handler = MainWinHandler {
        win: main_win.clone(),
    };

    let menubar = menus::create_menus();

    let mut builder = WindowBuilder::new();
    builder.set_handler(Box::new(main_win_handler));
    builder.set_title("xi-editor");
    builder.set_menu(menubar);
    let window = builder.build().unwrap();
    Ok((window, main_win))
}

fn main() {
    xi_win_shell::init();

    let (xi_peer, rx, semaphore) = start_xi_thread();

    let (window, main_win) = create_main(xi_peer).unwrap();
    window.show();
    let mut run_loop = win_main::RunLoop::new();
    let run_handle = run_loop.get_handle();
    unsafe {
        run_handle.add_handler(semaphore.get_handle(), move || {
            match rx.try_recv() {
                Ok(v) => main_win.handle_cmd(&v),
                Err(TryRecvError::Disconnected) => {
                    println!("core disconnected");
                }
                Err(TryRecvError::Empty) => (),
            }
        });
    }
    run_loop.run();
}
