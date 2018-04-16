// Copyright 2018 Google LLC
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

use direct2d::brush;
use direct2d::math::*;
use directwrite::{self, TextFormat, TextLayout};
use directwrite::text_format;
use directwrite::text_layout;

use xi_win_shell::paint::PaintCtx;
use xi_win_shell::util::default_text_options;

use MainWin;

use linecache::LineCache;

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
    fg: brush::SolidColor,
    bg: brush::SolidColor,
    text_format: TextFormat,
}

const TOP_PAD: f32 = 6.0;
const LINE_SPACE: f32 = 17.0;

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
        rt.fill_rectangle(&rect, &resources.bg);

        let first_line = self.y_to_line(0.0);
        let last_line = min(self.y_to_line(self.size.1) + 1, self.line_cache.height());

        let x0 = 6.0;
        let mut y = TOP_PAD + (first_line as f32) * LINE_SPACE - self.scroll_offset;
        for line_num in first_line..last_line {
            if let Some(line) = self.line_cache.get_line(line_num) {
                let layout = resources.create_text_layout(&self.dwrite_factory, line.text());
                rt.draw_text_layout(
                    &Point2F::from((x0, y)),
                    &layout,
                    &resources.fg,
                    default_text_options()
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
            y += LINE_SPACE;
        }
    }

    pub fn set_view_id(&mut self, view_id: &str) {
        self.view_id = view_id.into();
    }

    pub fn apply_update(&mut self, update: &Value) {
        self.line_cache.apply_update(update);
        self.constrain_scroll();
    }

    pub fn char(&self, ch: u32, _mods: u32, win: &MainWin) {
        let view_id = &self.view_id;
        match ch {
            0x08 => {
                win.send_edit_cmd("delete_backward", &json!([]), view_id);
            },
            0x0d => {
                win.send_edit_cmd("insert_newline", &json!([]), view_id);
            },
            _ => {
                if let Some(c) = ::std::char::from_u32(ch) {
                    let params = json!({"chars": c.to_string()});
                    win.send_edit_cmd("insert", &params, view_id);
                }
            }
        }
    }

    pub fn keydown(&self, vk_code: i32, _mods: u32, win: &MainWin) -> bool {
        let view_id = &self.view_id;
        // Handle special keys here
        match vk_code {
            VK_UP => {
                win.send_edit_cmd("move_up", &json!([]), view_id);
            },
            VK_DOWN => {
                win.send_edit_cmd("move_down", &json!([]), view_id );
            },
            VK_LEFT => {
                win.send_edit_cmd("move_left", &json!([]), view_id);
            },
            VK_RIGHT => {
                win.send_edit_cmd("move_right", &json!([]), view_id);
            },
            VK_DELETE => {
                win.send_edit_cmd("delete_forward", &json!([]), view_id);
            },
            _ => return false
        }
        true
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

    fn update_viewport(&mut self, win: &MainWin) {
        let first_line = self.y_to_line(0.0);
        let last_line = first_line + ((self.size.1 / LINE_SPACE).floor() as usize) + 1;
        let viewport = first_line..last_line;
        if viewport != self.viewport {
            self.viewport = viewport;
            let view_id = &self.view_id;
            win.send_edit_cmd("scroll", &json!([first_line, last_line]), view_id);
        }
    }
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
