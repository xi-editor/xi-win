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

//! The line cache (text, styles and cursors for a view).

use serde_json::Value;

pub struct Line {
    text: String,
    cursor: Vec<usize>,
}

impl Line {
    pub fn from_json(v: &Value) -> Line {
        let text = v["text"].as_str().unwrap().to_owned();
        let mut cursor = Vec::new();
        if let Some(arr) = v["cursor"].as_array() {
            for c in arr {
                // TODO: this is probably the best place to convert to utf-16
                cursor.push(c.as_u64().unwrap() as usize);
            }
        }
        Line { text, cursor }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> &[usize] {
        &self.cursor
    }
}

pub struct LineCache {
    lines: Vec<Option<Line>>
}

impl LineCache {
    pub fn new() -> LineCache {
        LineCache {
            lines: Vec::new(),
        }
    }

    pub fn apply_update(&mut self, update: &Value) {
        let mut lines = Vec::new();
        for op in update["ops"].as_array().unwrap() {
            //println!("op: {:?}", op);
            let op_type = &op["op"];
            if op_type == "ins" {
                for line in op["lines"].as_array().unwrap() {
                    lines.push(Some(Line::from_json(line)));
                }
            }
        }
        self.lines = lines;
    }

    pub fn height(&self) -> usize {
        self.lines.len()
    }

    pub fn get_line(&self, ix: usize) -> Option<&Line> {
        if ix < self.lines.len() {
            self.lines[ix].as_ref()
        } else {
            None
        }
    }
}
