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

//! A line of styled text, as much layout information precalculated as possible.

use direct2d::RenderTarget;
use direct2d::brush;
use directwrite::{self, TextFormat, TextLayout};
use directwrite::text_layout;

use xi_win_shell::util::default_text_options;

use linecache::Line;

pub struct TextLine {
    layout: TextLayout,
    // This is in utf-16 code units. Can make the case it should be floats so we
    // don't have to re-measure in draw_cursor, but whatever.
    cursor: Vec<usize>,
}

impl TextLine {
    pub fn create_from_line(line: &Line, factory: &directwrite::Factory, format: &TextFormat)
        -> TextLine
    {
        let text = line.text();
        let params = text_layout::ParamBuilder::new()
            .text(text)
            .font(format.clone())
            .width(1e6)
            .height(1e6)
            .build().unwrap();
        let layout = factory.create(params).unwrap();
        let cursor = line.cursor().iter().map(|&offset_utf8|
            count_utf16(&text[..offset_utf8])).collect();
        TextLine {
            layout,
            cursor,
        }
    }

    /// Draw the text at the specified coordinate. Does not draw background or cursor.
    ///
    /// Note: the `fg` param will probably go away, as styles will be incorporated
    /// into the TextLine itself.
    pub fn draw_text(&self, rt: &mut RenderTarget, x: f32, y: f32, fg: &brush::SolidColor) {
        rt.draw_text_layout(&(x, y).into(), &self.layout, fg, default_text_options());
    }

    /// Draw the carets.
    pub fn draw_cursor(&self, rt: &mut RenderTarget, x: f32, y: f32, fg: &brush::SolidColor) {
        for &offset in &self.cursor {
            if let Some(pos) = self.layout.hit_test_text_position(offset as u32, true) {
                let xc = x + pos.point_x;
                rt.draw_line(&((xc, y)).into(),
                    &((xc, y + 17.0)).into(),
                    fg, 1.0, None);
            }
        }
    }

    /// Return the utf-8 offset corresponding to the point (relative to top left corner).
    ///
    /// The `text` parameter is for utf-16 to utf-8 conversion, and is to avoid having
    /// to stash a separate copy.
    pub fn hit_test(&self, x: f32, y: f32, text: &str) -> usize {
        let hit = self.layout.hit_test_point(x, y);
        let utf16_offset = hit.metrics.text_position() as usize;
        conv_utf16_to_utf8_offset(text, utf16_offset)
        // TODO: if hit.is_trailing_hit is true, we want the next grapheme cluster
        // boundary (requires wiring up unicode segmentation crate).
    }
}

/// Counts the number of utf-16 code units in the given string.
fn count_utf16(s: &str) -> usize {
    let mut utf16_count = 0;
    for &b in s.as_bytes() {
        if (b as i8) >= -0x40 { utf16_count += 1; }
        if b >= 0xf0 { utf16_count += 1; }
    }
    utf16_count
}

/// Convert utf-16 code unit offset to utf-8 code unit offset.
fn conv_utf16_to_utf8_offset(s: &str, utf16_offset: usize) -> usize {
    let mut utf16_count = 0;
    for (i, &b) in s.as_bytes().iter().enumerate() {
        if utf16_count == utf16_offset {
            return i;
        }
        if (b as i8) >= -0x40 { utf16_count += 1; }
        if b >= 0xf0 { utf16_count += 1; }
    }
    s.len()
}
