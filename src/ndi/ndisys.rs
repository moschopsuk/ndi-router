#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case)]

#[cfg_attr(
    all(target_os = "macos"),
    link(name = "ndi")
)]
extern "C" {
    pub fn NDIlib_initialize() -> bool;

    pub fn NDIlib_find_create_v2(
        p_create_settings: *const NDIlib_find_create_t,
    ) -> NDIlib_find_instance_t;

    pub fn NDIlib_find_destroy(
        p_instance: NDIlib_find_instance_t
    );

    pub fn NDIlib_find_wait_for_sources(
        p_instance: NDIlib_find_instance_t,
        timeout_in_ms: u32,
    ) -> bool;

    pub fn NDIlib_find_get_current_sources(
        p_instance: NDIlib_find_instance_t,
        p_no_sources: *mut u32,
    ) -> *const NDIlib_source_t;

    pub fn NDIlib_routing_create(
        p_create_settings: *const NDIlib_routing_create_t
    ) -> NDIlib_routing_instance_t;

    pub fn  NDIlib_routing_destroy(
        p_instance: NDIlib_routing_instance_t
    );

    pub fn  NDIlib_routing_change(
        p_instance: NDIlib_routing_instance_t,
        p_source: *const NDIlib_source_t
    );

    pub fn  NDIlib_routing_clear(
        p_instance: NDIlib_routing_instance_t
    );
}

pub type NDIlib_find_instance_t = *mut ::std::os::raw::c_void;
pub type NDIlib_routing_instance_t = *mut ::std::os::raw::c_void;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct NDIlib_find_create_t {
    pub show_local_sources: bool,
    pub p_groups: *const ::std::os::raw::c_char,
    pub p_extra_ips: *const ::std::os::raw::c_char,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct NDIlib_source_t {
    pub p_ndi_name: *const ::std::os::raw::c_char,
    pub p_ip_address: *const ::std::os::raw::c_char,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct NDIlib_routing_create_t {
    pub p_ndi_name: *const ::std::os::raw::c_char,
    pub p_groups: *const ::std::os::raw::c_char,
}
