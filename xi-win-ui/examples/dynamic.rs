// Copyright 2018 The xi-editor Authors.
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

//! An example of dynamic graph mutation.

extern crate xi_win_shell;
extern crate xi_win_ui;
extern crate direct2d;
extern crate directwrite;

use xi_win_shell::win_main;
use xi_win_shell::window::WindowBuilder;

use xi_win_ui::{UiMain, UiState};
use xi_win_ui::widget::{Button, Row, Padding};

fn main() {
    xi_win_shell::init();

    let mut run_loop = win_main::RunLoop::new();
    let mut builder = WindowBuilder::new();
    let mut state = UiState::new();
    let button = Button::new("Add").ui(&mut state);
    let buttonp = Padding::uniform(10.0).ui(button, &mut state);
    let root = Row::new().ui(&[buttonp], &mut state);
    state.set_root(root);
    state.add_listener(button, move |_: &mut bool, mut ctx| {
        let new_button = Button::new("New").ui(&mut ctx);
        ctx.add_listener(new_button, |_: &mut bool, mut _ctx| {
            println!("new button was clicked");
        });
        let padded = Padding::uniform(10.0).ui(new_button, &mut ctx);
        ctx.append_child(root, padded);
    });
    builder.set_handler(Box::new(UiMain::new(state)));
    builder.set_title("Dynamic example");
    let window = builder.build().unwrap();
    window.show();
    run_loop.run();
}
