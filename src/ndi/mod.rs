mod ndisys;
use crate::ndi::ndisys::*;

pub fn initialize() -> bool {
    unsafe { NDIlib_initialize() }
}