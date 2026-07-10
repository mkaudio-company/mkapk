//! Real VST3 `IPluginFactory`/`IPluginFactory2` implementation. Reusable
//! across plugin projects: holds a `PluginInfo` plus two plain constructor
//! functions (no captured state needed, since everything they need is
//! either a compile-time CID or freshly constructed inside the function
//! body), and dispatches `create_instance` by CID.
use std::os::raw::c_void;

use vst3_com::IID;
use vst3_com::sys::GUID;
use vst3_sys::VST3;
use vst3_sys::base::{
    ClassCardinality, IPluginFactory, IPluginFactory2, PClassInfo, PClassInfo2, PFactoryInfo,
    kInvalidArgument, kResultFalse, kResultOk, tresult,
};

use crate::util::strcpy;

/// Static plugin identity used to answer the host's factory/class queries.
pub struct PluginInfo {
    pub processor_cid: GUID,
    pub controller_cid: GUID,
    pub name: &'static str,
    pub vendor: &'static str,
    pub url: &'static str,
    pub email: &'static str,
    pub version: &'static str,
}

/// Builds a `GUID` from 16 raw bytes -- a small convenience so plugin
/// projects don't need to depend on `vst3-sys`/`vst3-com` directly just to
/// name a CID.
pub const fn guid(data: [u8; 16]) -> GUID {
    GUID { data }
}

type CreateInstanceFn = fn() -> *mut c_void;

#[VST3(implements(IPluginFactory2, IPluginFactory))]
pub struct VstFactory {
    info: PluginInfo,
    create_processor: CreateInstanceFn,
    create_controller: CreateInstanceFn,
}

impl VstFactory {
    pub fn new(
        info: PluginInfo,
        create_processor: CreateInstanceFn,
        create_controller: CreateInstanceFn,
    ) -> Box<Self> {
        Self::allocate(info, create_processor, create_controller)
    }

    pub fn create_instance(
        info: PluginInfo,
        create_processor: CreateInstanceFn,
        create_controller: CreateInstanceFn,
    ) -> *mut c_void {
        Box::into_raw(Self::new(info, create_processor, create_controller)) as *mut c_void
    }
}

impl IPluginFactory for VstFactory {
    unsafe fn get_factory_info(&self, info: *mut PFactoryInfo) -> tresult {
        unsafe {
            let out = &mut *info;
            strcpy(self.info.vendor, out.vendor.as_mut_ptr());
            strcpy(self.info.url, out.url.as_mut_ptr());
            strcpy(self.info.email, out.email.as_mut_ptr());
            out.flags = 8; // kUnicode
        }
        kResultOk
    }

    unsafe fn count_classes(&self) -> i32 {
        2
    }

    unsafe fn get_class_info(&self, index: i32, info: *mut PClassInfo) -> tresult {
        unsafe {
            match index {
                0 => {
                    let out = &mut *info;
                    out.cid = self.info.processor_cid;
                    out.cardinality = ClassCardinality::kManyInstances as i32;
                    strcpy("Audio Module Class", out.category.as_mut_ptr());
                    strcpy(self.info.name, out.name.as_mut_ptr());
                }
                1 => {
                    let out = &mut *info;
                    out.cid = self.info.controller_cid;
                    out.cardinality = ClassCardinality::kManyInstances as i32;
                    strcpy("Component Controller Class", out.category.as_mut_ptr());
                    strcpy(self.info.name, out.name.as_mut_ptr());
                }
                _ => return kInvalidArgument,
            }
        }
        kResultOk
    }

    unsafe fn create_instance(
        &self,
        cid: *const IID,
        _iid: *const IID,
        obj: *mut *mut c_void,
    ) -> tresult {
        unsafe {
            if *cid == self.info.processor_cid {
                *obj = (self.create_processor)();
                return kResultOk;
            }
            if *cid == self.info.controller_cid {
                *obj = (self.create_controller)();
                return kResultOk;
            }
        }
        kResultFalse
    }
}

impl IPluginFactory2 for VstFactory {
    unsafe fn get_class_info2(&self, index: i32, info: *mut PClassInfo2) -> tresult {
        unsafe {
            match index {
                0 => {
                    let out = &mut *info;
                    out.cid = self.info.processor_cid;
                    out.cardinality = ClassCardinality::kManyInstances as i32;
                    strcpy("Audio Module Class", out.category.as_mut_ptr());
                    strcpy(self.info.name, out.name.as_mut_ptr());
                    out.class_flags = 1;
                    strcpy("Fx", out.subcategories.as_mut_ptr());
                    strcpy(self.info.vendor, out.vendor.as_mut_ptr());
                    strcpy(self.info.version, out.version.as_mut_ptr());
                    strcpy("VST 3.7.0", out.sdk_version.as_mut_ptr());
                }
                1 => {
                    let out = &mut *info;
                    out.cid = self.info.controller_cid;
                    out.cardinality = ClassCardinality::kManyInstances as i32;
                    strcpy("Component Controller Class", out.category.as_mut_ptr());
                    strcpy(self.info.name, out.name.as_mut_ptr());
                    out.class_flags = 0;
                    strcpy("", out.subcategories.as_mut_ptr());
                    strcpy(self.info.vendor, out.vendor.as_mut_ptr());
                    strcpy(self.info.version, out.version.as_mut_ptr());
                    strcpy("VST 3.7.0", out.sdk_version.as_mut_ptr());
                }
                _ => return kInvalidArgument,
            }
        }
        kResultOk
    }
}
