mod ndisys;
use crate::ndi::ndisys::*;

use std::ffi;
use std::mem;
use std::ptr;
use std::sync::{Arc, Mutex};

use log::{debug, info};

pub fn initialize() -> bool {
    unsafe { NDIlib_initialize() }
}

/*
                                                           dddddddd
FFFFFFFFFFFFFFFFFFFFFF  iiii                               d::::::d
F::::::::::::::::::::F i::::i                              d::::::d
F::::::::::::::::::::F  iiii                               d::::::d
FF::::::FFFFFFFFF::::F                                     d:::::d 
  F:::::F       FFFFFFiiiiiiinnnn  nnnnnnnn        ddddddddd:::::d 
  F:::::F             i:::::in:::nn::::::::nn    dd::::::::::::::d 
  F::::::FFFFFFFFFF    i::::in::::::::::::::nn  d::::::::::::::::d 
  F:::::::::::::::F    i::::inn:::::::::::::::nd:::::::ddddd:::::d 
  F:::::::::::::::F    i::::i  n:::::nnnn:::::nd::::::d    d:::::d 
  F::::::FFFFFFFFFF    i::::i  n::::n    n::::nd:::::d     d:::::d 
  F:::::F              i::::i  n::::n    n::::nd:::::d     d:::::d 
  F:::::F              i::::i  n::::n    n::::nd:::::d     d:::::d 
FF:::::::FF           i::::::i n::::n    n::::nd::::::ddddd::::::dd
F::::::::FF           i::::::i n::::n    n::::n d:::::::::::::::::d
F::::::::FF           i::::::i n::::n    n::::n  d:::::::::ddd::::d
FFFFFFFFFFF           iiiiiiii nnnnnn    nnnnnn   ddddddddd   ddddd
 */

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

/*
   SSSSSSSSSSSSSSS                                                                                               
 SS:::::::::::::::S                                                                                              
S:::::SSSSSS::::::S                                                                                              
S:::::S     SSSSSSS                                                                                              
S:::::S               ooooooooooo   uuuuuu    uuuuuu rrrrr   rrrrrrrrr       cccccccccccccccc    eeeeeeeeeeee    
S:::::S             oo:::::::::::oo u::::u    u::::u r::::rrr:::::::::r    cc:::::::::::::::c  ee::::::::::::ee  
 S::::SSSS         o:::::::::::::::ou::::u    u::::u r:::::::::::::::::r  c:::::::::::::::::c e::::::eeeee:::::ee
  SS::::::SSSSS    o:::::ooooo:::::ou::::u    u::::u rr::::::rrrrr::::::rc:::::::cccccc:::::ce::::::e     e:::::e
    SSS::::::::SS  o::::o     o::::ou::::u    u::::u  r:::::r     r:::::rc::::::c     ccccccce:::::::eeeee::::::e
       SSSSSS::::S o::::o     o::::ou::::u    u::::u  r:::::r     rrrrrrrc:::::c             e:::::::::::::::::e 
            S:::::So::::o     o::::ou::::u    u::::u  r:::::r            c:::::c             e::::::eeeeeeeeeee  
            S:::::So::::o     o::::ou:::::uuuu:::::u  r:::::r            c::::::c     ccccccce:::::::e           
SSSSSSS     S:::::So:::::ooooo:::::ou:::::::::::::::uur:::::r            c:::::::cccccc:::::ce::::::::e          
S::::::SSSSSS:::::So:::::::::::::::o u:::::::::::::::ur:::::r             c:::::::::::::::::c e::::::::eeeeeeee  
S:::::::::::::::SS  oo:::::::::::oo   uu::::::::uu:::ur:::::r              cc:::::::::::::::c  ee:::::::::::::e  
 SSSSSSSSSSSSSSS      ooooooooooo       uuuuuuuu  uuuurrrrrrr                cccccccccccccccc    eeeeeeeeeeeeee  
*/

#[derive(Debug)]
pub enum Source<'a> {
    Borrowed(ptr::NonNull<NDIlib_source_t>, &'a FindInstance),
    Owned(NDIlib_source_t, ffi::CString, ffi::CString),
}

unsafe impl<'a> Send for Source<'a> {}
unsafe impl<'a> Sync for Source<'a> {}

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

    pub fn ndi_name_ptr(&self) -> *const ::std::os::raw::c_char {
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

/*
RRRRRRRRRRRRRRRRR                                             tttt                              
R::::::::::::::::R                                         ttt:::t                              
R::::::RRRRRR:::::R                                        t:::::t                              
RR:::::R     R:::::R                                       t:::::t                              
  R::::R     R:::::R   ooooooooooo   uuuuuu    uuuuuuttttttt:::::ttttttt        eeeeeeeeeeee    
  R::::R     R:::::R oo:::::::::::oo u::::u    u::::ut:::::::::::::::::t      ee::::::::::::ee  
  R::::RRRRRR:::::R o:::::::::::::::ou::::u    u::::ut:::::::::::::::::t     e::::::eeeee:::::ee
  R:::::::::::::RR  o:::::ooooo:::::ou::::u    u::::utttttt:::::::tttttt    e::::::e     e:::::e
  R::::RRRRRR:::::R o::::o     o::::ou::::u    u::::u      t:::::t          e:::::::eeeee::::::e
  R::::R     R:::::Ro::::o     o::::ou::::u    u::::u      t:::::t          e:::::::::::::::::e 
  R::::R     R:::::Ro::::o     o::::ou::::u    u::::u      t:::::t          e::::::eeeeeeeeeee  
  R::::R     R:::::Ro::::o     o::::ou:::::uuuu:::::u      t:::::t    tttttte:::::::e           
RR:::::R     R:::::Ro:::::ooooo:::::ou:::::::::::::::uu    t::::::tttt:::::te::::::::e          
R::::::R     R:::::Ro:::::::::::::::o u:::::::::::::::u    tt::::::::::::::t e::::::::eeeeeeee  
R::::::R     R:::::R oo:::::::::::oo   uu::::::::uu:::u      tt:::::::::::tt  ee:::::::::::::e  
RRRRRRRR     RRRRRRR   ooooooooooo       uuuuuuuu  uuuu        ttttttttttt      eeeeeeeeeeeeee  
*/

#[derive(Debug)]
pub struct RouteBuilder<'a> {
    ndi_name: &'a str,
    groups: Option<&'a str>,
}

impl<'a> RouteBuilder<'a> {
    pub fn build(self) -> Option<RouteInstance> {
        unsafe {
            let ndi_name = ffi::CString::new(self.ndi_name).unwrap();
            let groups = self.groups.map(|s| ffi::CString::new(s).unwrap());
            
            let ptr = NDIlib_routing_create(&NDIlib_routing_create_t {
                p_ndi_name: ndi_name.as_ptr(),
                p_groups: groups.as_ref().map(|s| s.as_ptr()).unwrap_or(ptr::null()),
            });

            debug!("creating NDI source {}", self.ndi_name);

            if ptr.is_null() {
                None
            } else {
                Some(RouteInstance(Arc::new((
                    RouteInstanceInner(ptr::NonNull::new_unchecked(ptr), self.ndi_name.to_owned()),
                    Mutex::new(()),
                ))))
            }
        }
    }
}

pub struct RouteInstance(Arc<(RouteInstanceInner, Mutex<()>)>);

#[derive(Debug)]
struct RouteInstanceInner(ptr::NonNull<::std::os::raw::c_void>, String);
unsafe impl Send for RouteInstanceInner {}
unsafe impl Sync for RouteInstanceInner {}

impl RouteInstance {
    pub fn builder<'a>(ndi_name: &'a str) -> RouteBuilder<'a> {
        let groups = None;
        RouteBuilder {
            ndi_name,
            groups
        }
    }

    pub fn change(&self, source: &Source) {
        unsafe {
            let _lock = (self.0).1.lock().unwrap();
            info!("routing {:?} to {:?}", source.ndi_name(), ((self.0).0).1);
            NDIlib_routing_change(
                ((self.0).0).0.as_ptr(),
                &NDIlib_source_t {
                    p_ndi_name: source.ndi_name_ptr(),
                    p_ip_address: source.ip_address_ptr(),
                });
        }
    }

    pub fn clear(&self) {
        unsafe {
            let _lock = (self.0).1.lock().unwrap();
            NDIlib_routing_clear(((self.0).0).0.as_ptr());
        }
    }
}


impl Drop for RouteInstanceInner {
    fn drop(&mut self) {
        unsafe { NDIlib_routing_destroy(self.0.as_ptr() as *mut _) }
    }
}