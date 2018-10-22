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

//! The main edit view.

use std::cmp::min;
use std::ops::Range;

use serde_json::Value;

use winapi::um::winuser::*;

use direct2d::brush::SolidColorBrush;
use direct2d::math::*;
use direct2d::RenderTarget;
use directwrite;
use directwrite::TextFormat;

use xi_win_shell::paint::PaintCtx;
use xi_win_shell::window::{M_ALT, M_CTRL, M_SHIFT, MouseButton, MouseType};

use xi_win_ui::widget::Widget;

use MainWin;

use linecache::LineCache;
use textline::TextLine;

/// State and behavior for one editor view.
pub struct EditView {
    // Note: these public fields should be properly encapsulated.
    pub view_id: String,
    pub filename: Option<String>,
    line_cache: LineCache,
    dwrite_factory: directwrite::Factory,
    resources: Option<Resources>,
    scroll_offset: f32,
    size: (f32, f32),  // in px units
    viewport: Range<usize>,
}

struct Resources {
    fg: SolidColorBrush,
    bg: SolidColorBrush,
    sel: SolidColorBrush,
    text_format: TextFormat,
}

const TOP_PAD: f32 = 6.0;
const LEFT_PAD: f32 = 6.0;
const LINE_SPACE: f32 = 17.0;

impl Widget for EditView {
    
}

impl EditView {
    pub fn new() -> EditView {
        EditView {
            view_id: "".into(),
            filename: None,
            line_cache: LineCache::new(),
            dwrite_factory: directwrite::Factory::new().unwrap(),
            resources: None,
            scroll_offset: 0.0,
            size: (0.0, 0.0),
            viewport: 0..0,
        }
    }

    fn create_resources(&mut self, p: &mut PaintCtx) -> Resources {
        let rt = p.render_target();
        let text_format = TextFormat::create(&self.dwrite_factory)
            .with_family("Consolas")
            .with_size(15.0)
            .build()
            .unwrap();
        Resources {
            fg: SolidColorBrush::create(rt).with_color(0xf0f0ea).build().unwrap(),
            bg: SolidColorBrush::create(rt).with_color(0x272822).build().unwrap(),
            sel: SolidColorBrush::create(rt).with_color(0x49483e).build().unwrap(),
            text_format: text_format,
        }
    }

    pub fn rebuild_resources(&mut self) {
        self.resources = None;
    }

    pub fn size(&mut self, x: f32, y: f32) {
        self.size = (x, y);
        self.constrain_scroll();
    }

    pub fn clear_line_cache(&mut self) {
        self.line_cache = LineCache::new();
    }

    pub fn render(&mut self, p: &mut PaintCtx) {
        if self.resources.is_none() {
            self.resources = Some(self.create_resources(p));
        }
        let resources = &self.resources.as_ref().unwrap();
        let rt = p.render_target();
        let rect = RectF::from((0.0, 0.0, self.size.0, self.size.1));
        rt.fill_rectangle(rect, &resources.bg);

        let first_line = self.y_to_line(0.0);
        let last_line = min(self.y_to_line(self.size.1) + 1, self.line_cache.height());

        let x0 = LEFT_PAD;
        let mut y = self.line_to_content_y(first_line) - self.scroll_offset;
        for line_num in first_line..last_line {
            if let Some(textline) = self.get_text_line(line_num) {
                textline.draw_bg(rt, x0, y, &resources.sel);
            }
            y += LINE_SPACE;
        }
        let mut y = self.line_to_content_y(first_line) - self.scroll_offset;
        for line_num in first_line..last_line {
            if let Some(textline) = self.get_text_line(line_num) {
                textline.draw_text(rt, x0, y, &resources.fg);
                textline.draw_cursor(rt, x0, y, &resources.fg);
            }
            y += LINE_SPACE;
        }
    }

    // signature will change when we start caching
    fn get_text_line(&self, line_num: usize) -> Option<TextLine> {
        self.line_cache.get_line(line_num).map(|line| {
            let format = &self.resources.as_ref().unwrap().text_format;
            TextLine::create_from_line(&line, &self.dwrite_factory, format)
        })
    }

    pub fn set_view_id(&mut self, view_id: &str) {
        self.view_id = view_id.into();
    }

    pub fn apply_update(&mut self, update: &Value) {
        self.line_cache.apply_update(update);
        self.constrain_scroll();
    }

    pub fn char(&self, ch: u32, _mods: u32, win: &MainWin) {
        if let Some(c) = ::std::char::from_u32(ch) {
            if ch >= 0x20 {
                // Don't insert control characters
                let params = json!({"chars": c.to_string()});
                self.send_edit_cmd("insert", &params, win);
            }
        }
    }

    fn send_edit_cmd(&self, method: &str, params: &Value, win: &MainWin) {
        win.send_edit_cmd(method, params, &self.view_id);
    }

    /// Sends a simple action with no parameters
    fn send_action(&self, method: &str, win: &MainWin) {
        self.send_edit_cmd(method, &json!([]), win);
    }

    pub fn keydown(&mut self, vk_code: i32, mods: u32, win: &MainWin) -> bool {
        // Handle special keys here
        match vk_code {
            VK_RETURN => {
                // TODO: modifiers are variants of open
                self.send_action("insert_newline", win);
            }
            VK_TAB => {
                // TODO: modified versions
                self.send_action("insert_tab", win);
            }
            VK_UP => {
                if mods == M_CTRL {
                    self.scroll_offset -= LINE_SPACE;
                    self.constrain_scroll();
                    self.update_viewport(win);
                    win.invalidate();
                } else {
                    let action = if mods == M_CTRL | M_ALT {
                        "add_selection_above"
                    } else {
                        s(mods, "move_up", "move_up_and_modify_selection")
                    };
                    // TODO: swap line up is ctrl + shift
                    self.send_action(action, win);
                }
            }
            VK_DOWN => {
                if mods == M_CTRL {
                    self.scroll_offset += LINE_SPACE;
                    self.constrain_scroll();
                    self.update_viewport(win);
                    win.invalidate();
                } else {
                    let action = if mods == M_CTRL | M_ALT {
                        "add_selection_below"
                    } else {
                        s(mods, "move_down", "move_down_and_modify_selection")
                    };
                    self.send_action(action, win);
                }
            }
            VK_LEFT => {
                // TODO: there is a subtle distinction between alt and ctrl
                let action = if (mods & (M_ALT | M_CTRL)) != 0 {
                    s(mods, "move_word_left", "move_word_left_and_modify_selection")
                } else {
                    s(mods, "move_left", "move_left_and_modify_selection")
                };
                self.send_action(action, win);
            }
            VK_RIGHT => {
                // TODO: there is a subtle distinction between alt and ctrl
                let action = if (mods & (M_ALT | M_CTRL)) != 0 {
                    s(mods, "move_word_right", "move_word_right_and_modify_selection")
                } else {
                    s(mods, "move_right", "move_right_and_modify_selection")
                };
                self.send_action(action, win);
            }
            VK_PRIOR => {
                self.send_action(s(mods, "scroll_page_up",
                    "page_up_and_modify_selection"), win);
            }
            VK_NEXT => {
                self.send_action(s(mods, "scroll_page_down",
                    "page_down_and_modify_selection"), win);
            }
            VK_HOME => {
                let action = if (mods & M_CTRL) != 0 {
                    s(mods, "move_to_beginning_of_document",
                        "move_to_beginning_of_document_and_modify_selection")
                } else {
                    s(mods, "move_to_left_end_of_line",
                        "move_to_left_end_of_line_and_modify_selection")
                };
                self.send_action(action, win);
            }
            VK_END => {
                let action = if (mods & M_CTRL) != 0 {
                    s(mods, "move_to_end_of_document",
                        "move_to_end_of_document_and_modify_selection")
                } else {
                    s(mods, "move_to_right_end_of_line",
                        "move_to_right_end_of_line_and_modify_selection")
                };
                self.send_action(action, win);
            }
            VK_ESCAPE => {
                self.send_action("cancel_operation", win);
            }
            VK_BACK => {
                let action = if (mods & M_CTRL) != 0 {
                    // should be "delete to beginning of paragraph" but not supported
                    s(mods, "delete_word_backward", "delete_to_beginning_of_line")
                } else {
                    "delete_backward"
                };
                self.send_action(action, win);
            }
            VK_DELETE => {
                let action = if (mods & M_CTRL) != 0 {
                    s(mods, "delete_word_forward", "delete_to_end_of_paragraph")
                } else {
                    // TODO: shift-delete should be "delete line"
                    "delete_forward"
                };
                self.send_action(action, win);
            }
            VK_OEM_4 => {
                // generally '[' key, but might vary on non-US keyboards
                if mods == M_CTRL {
                    self.send_action("outdent", win);
                } else {
                    return false
                }
            }
            VK_OEM_6 => {
                // generally ']' key, but might vary on non-US keyboards
                if mods == M_CTRL {
                    self.send_action("indent", win);
                } else {
                    return false
                }
            }
            _ => {
                return false
            }
        }
        true
    }

    // Commands

    pub fn undo(&mut self, win: &MainWin) {
        self.send_action("undo", win);
    }

    pub fn redo(&mut self, win: &MainWin) {
        self.send_action("redo", win);
    }

    pub fn upper_case(&mut self, win: &MainWin) {
        self.send_action("uppercase", win);
    }

    pub fn lower_case(&mut self, win: &MainWin) {
        self.send_action("lowercase", win);
    }

    pub fn transpose(&mut self, win: &MainWin) {
        self.send_action("transpose", win);
    }

    pub fn add_cursor_above(&mut self, win: &MainWin) {
        // Note: some subtlety around find, the escape key cancels it, but the menu
        // shouldn't.
        self.send_action("add_selection_above", win);
    }

    pub fn add_cursor_below(&mut self, win: &MainWin) {
        // Note: some subtlety around find, the escape key cancels it, but the menu
        // shouldn't.
        self.send_action("add_selection_below", win);
    }

    pub fn single_selection(&mut self, win: &MainWin) {
        // Note: some subtlety around find, the escape key cancels it, but the menu
        // shouldn't.
        self.send_action("cancel_operation", win);
    }

    pub fn select_all(&mut self, win: &MainWin) {
        // Note: some subtlety around find, the escape key cancels it, but the menu
        // shouldn't.
        self.send_action("select_all", win);
    }

    pub fn mouse(&self, x: f32, y: f32, mods: u32, which: MouseButton, ty: MouseType,
        win: &MainWin)
    {
        if which == MouseButton::Left && ty == MouseType::Down {
            let (line, col) = self.xy_to_line_col(x, y);
            let params = json!({
                "ty": "point_select",
                "line": line,
                "col": col,
            });
            self.send_edit_cmd("gesture", &params, win);
            println!("{},{} (line {}) {} {:?} {:?}", x, y, line, mods, which, ty);
        }
    }

    pub fn mouse_wheel(&mut self, delta: i32, _mods: u32, win: &MainWin) {
        // TODO: scale properly, taking SPI_GETWHEELSCROLLLINES into account
        let scroll_scaling = 0.5;
        self.scroll_offset -= (delta as f32) * scroll_scaling;
        self.constrain_scroll();
        self.update_viewport(win);
        win.handle.borrow().invalidate();
    }

    fn constrain_scroll(&mut self) {
        let max_scroll = TOP_PAD + LINE_SPACE *
            (self.line_cache.height().saturating_sub(1)) as f32;
        if self.scroll_offset < 0.0 {
            self.scroll_offset = 0.0;
        } else if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }
    }

    // Takes y in screen-space px.
    fn y_to_line(&self, y: f32) -> usize {
        let mut line = (y + self.scroll_offset - TOP_PAD) / LINE_SPACE;
        if line < 0.0 { line = 0.0; }
        let line = line.floor() as usize;
        min(line, self.line_cache.height())
    }

    /// Takes x, y in screen-space px, returns line number and utf8 offset within line.
    fn xy_to_line_col(&self, x: f32, y: f32) -> (usize, usize) {
        let line_num = self.y_to_line(y);
        let col = if let (Some(textline), Some(line)) =
            (self.get_text_line(line_num), self.line_cache.get_line(line_num))
        {
            textline.hit_test(x - LEFT_PAD, 0.0, line.text())
        } else {
            0
        };
        (line_num, col)
    }

    /// Convert line number to y coordinate in content space.
    fn line_to_content_y(&self, line: usize) -> f32 {
        TOP_PAD + (line as f32) * LINE_SPACE
    }

    fn update_viewport(&mut self, win: &MainWin) {
        let first_line = self.y_to_line(0.0);
        let last_line = first_line + ((self.size.1 / LINE_SPACE).floor() as usize) + 1;
        let viewport = first_line..last_line;
        if viewport != self.viewport {
            self.viewport = viewport;
            self.send_edit_cmd("scroll", &json!([first_line, last_line]), win);
        }
    }

    pub fn scroll_to(&mut self, line: usize) {
        let y = self.line_to_content_y(line);
        let bottom_slop = 20.0;
        if y < self.scroll_offset {
            self.scroll_offset = y;
        } else if y > self.scroll_offset + self.size.1 - bottom_slop {
            self.scroll_offset = y - (self.size.1 - bottom_slop)
        }
    }
}

// Helper function for choosing between normal and shifted action
fn s<'a>(mods: u32, normal: &'a str, shifted: &'a str) -> &'a str {
    if (mods & M_SHIFT) != 0 { shifted } else { normal }
}
