#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case)]

#[cfg_attr(
    all(target_os = "macos"),
    link(name = "ndi")
)]
extern "C" {
    pub fn NDIlib_initialize() -> bool;
}