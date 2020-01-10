mod ndisys;
use crate::ndi::ndisys::*;

use std::ffi;
use std::mem;
use std::ptr;

pub fn initialize() -> bool {
    unsafe { NDIlib_initialize() }
}

#[derive(Debug)]
pub struct FindBuilder<'a> {
    show_local_sources: bool,
    groups: Option<&'a str>,
    extra_ips: Option<&'a str>,
}

impl<'a> Default for FindBuilder<'a> {
    fn default() -> Self {
        Self {
            show_local_sources: true,
            groups: None,
            extra_ips: None,
        }
    }
}

impl<'a> FindBuilder<'a> {
    pub fn show_local_sources(self, show_local_sources: bool) -> Self {
        Self {
            show_local_sources,
            ..self
        }
    }

    pub fn build(self) -> Option<FindInstance> {
        let groups = self.groups.map(|s| ffi::CString::new(s).unwrap());
        let extra_ips = self.extra_ips.map(|s| ffi::CString::new(s).unwrap());

        unsafe {
            let ptr = NDIlib_find_create_v2(&NDIlib_find_create_t {
                show_local_sources: self.show_local_sources,
                p_groups: groups.as_ref().map(|s| s.as_ptr()).unwrap_or(ptr::null()),
                p_extra_ips: extra_ips
                    .as_ref()
                    .map(|s| s.as_ptr())
                    .unwrap_or(ptr::null()),
            });
            if ptr.is_null() {
                None
            } else {
                Some(FindInstance(ptr::NonNull::new_unchecked(ptr)))
            }
        }
    }
}

#[derive(Debug)]
pub struct FindInstance(ptr::NonNull<::std::os::raw::c_void>);
unsafe impl Send for FindInstance {}

impl FindInstance {
    pub fn builder<'a>() -> FindBuilder<'a> {
        FindBuilder::default()
    }

    pub fn wait_for_sources(&mut self, timeout_in_ms: u32) -> bool {
        unsafe { NDIlib_find_wait_for_sources(self.0.as_ptr(), timeout_in_ms) }
    }

    pub fn get_current_sources(&mut self) -> Vec<Source> {
        unsafe {
            let mut no_sources = mem::MaybeUninit::uninit();
            let sources_ptr =
                NDIlib_find_get_current_sources(self.0.as_ptr(), no_sources.as_mut_ptr());
            let no_sources = no_sources.assume_init();

            if sources_ptr.is_null() || no_sources == 0 {
                return vec![];
            }

            let mut sources = vec![];
            for i in 0..no_sources {
                sources.push(Source::Borrowed(
                    ptr::NonNull::new_unchecked(sources_ptr.add(i as usize) as *mut _),
                    self,
                ));
            }

            sources
        }
    }
}

impl Drop for FindInstance {
    fn drop(&mut self) {
        unsafe {
            NDIlib_find_destroy(self.0.as_mut());
        }
    }
}

#[derive(Debug)]
pub enum Source<'a> {
    Borrowed(ptr::NonNull<NDIlib_source_t>, &'a FindInstance),
    Owned(NDIlib_source_t, ffi::CString, ffi::CString),
}

unsafe impl<'a> Send for Source<'a> {}

impl<'a> Source<'a> {
    pub fn ndi_name(&self) -> &str {
        unsafe {
            let ptr = match *self {
                Source::Borrowed(ptr, _) => &*ptr.as_ptr(),
                Source::Owned(ref source, _, _) => source,
            };

            assert!(!ptr.p_ndi_name.is_null());
            ffi::CStr::from_ptr(ptr.p_ndi_name).to_str().unwrap()
        }
    }

    pub fn ip_address(&self) -> &str {
        unsafe {
            let ptr = match *self {
                Source::Borrowed(ptr, _) => &*ptr.as_ptr(),
                Source::Owned(ref source, _, _) => source,
            };

            assert!(!ptr.p_ip_address.is_null());
            ffi::CStr::from_ptr(ptr.p_ip_address).to_str().unwrap()
        }
    }

    fn ndi_name_ptr(&self) -> *const ::std::os::raw::c_char {
        unsafe {
            match *self {
                Source::Borrowed(ptr, _) => ptr.as_ref().p_ndi_name,
                Source::Owned(_, ref ndi_name, _) => ndi_name.as_ptr(),
            }
        }
    }

    fn ip_address_ptr(&self) -> *const ::std::os::raw::c_char {
        unsafe {
            match *self {
                Source::Borrowed(ptr, _) => ptr.as_ref().p_ip_address,
                Source::Owned(_, _, ref ip_address) => ip_address.as_ptr(),
            }
        }
    }

    pub fn to_owned<'b>(&self) -> Source<'b> {
        unsafe {
            let (ndi_name, ip_address) = match *self {
                Source::Borrowed(ptr, _) => (ptr.as_ref().p_ndi_name, ptr.as_ref().p_ip_address),
                Source::Owned(_, ref ndi_name, ref ip_address) => {
                    (ndi_name.as_ptr(), ip_address.as_ptr())
                }
            };

            let ndi_name = ffi::CString::new(ffi::CStr::from_ptr(ndi_name).to_bytes()).unwrap();
            let ip_address = ffi::CString::new(ffi::CStr::from_ptr(ip_address).to_bytes()).unwrap();

            Source::Owned(
                NDIlib_source_t {
                    p_ndi_name: ndi_name.as_ptr(),
                    p_ip_address: ip_address.as_ptr(),
                },
                ndi_name,
                ip_address,
            )
        }
    }
}