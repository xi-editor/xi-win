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

//! Bureaucracy to create hwnd render targets.
//!
//! Note that hwnd render targets are (relatively) easy, but for high
//! performance we want dxgi render targets so we can use present
//! options for scrolling and minimal invalidation.

use std::ptr::null_mut;

use winapi::*;

use direct2d::render_target::RenderTargetBacking;

pub struct HwndRtParams {
    pub hwnd: HWND,
    pub width: u32,
    pub height: u32,
}

unsafe impl RenderTargetBacking for HwndRtParams {
    fn create_target(self, factory: &mut ID2D1Factory) -> Result<*mut ID2D1RenderTarget, HRESULT> {
        unsafe {
            let mut ptr: *mut ID2D1HwndRenderTarget = null_mut();
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                _type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_UNKNOWN,
                    alphaMode: D2D1_ALPHA_MODE_UNKNOWN,
                },
                dpiX: 0.0,
                dpiY: 0.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let hprops = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd: self.hwnd,
                pixelSize: D2D1_SIZE_U {
                    width: self.width,
                    height: self.height,
                },
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };
            let hr = factory.CreateHwndRenderTarget(
                &props,
                &hprops,
                &mut ptr as *mut _,
            );

            if SUCCEEDED(hr) {
                Ok(ptr as *mut _)
            } else {
                Err(From::from(hr))
            }
        }
    }
}
