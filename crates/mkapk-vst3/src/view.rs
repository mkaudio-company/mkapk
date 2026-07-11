//! Bridge functions the C++ shim's `VstEditorView` (`cpp/VstView.cpp`)
//! calls into via `cpp/VstBridge.h`. Wraps a `mkapk_host::PluginEditor` and
//! forwards UI-originated parameter edits/resize requests to the host via
//! the raw C function pointers `VstEditorView` registers through
//! `mkapk_vst3_editor_set_host_context` -- the one direction the plugin
//! calls *out* to the host, kept as plain `extern "C"` callbacks (like
//! everything else in this bridge) rather than reaching for a second FFI
//! paradigm (e.g. `cxx`) for just this.
use std::os::raw::c_void;
use std::sync::Arc;

use mkapk_core::Sizef;
use mkapk_host::{
    EditorHost, LockFreeParameterGateway, NormalizedValue, ParameterId, ParameterMessage,
    ParentWindowHandle, PluginEditor,
};

type BeginEditFn = extern "C" fn(*mut c_void, u32);
type PerformEditFn = extern "C" fn(*mut c_void, u32, f64);
type EndEditFn = extern "C" fn(*mut c_void, u32);
type ResizeViewFn = extern "C" fn(*mut c_void, f32, f32);

#[derive(Default)]
struct HostContext {
    context: *mut c_void,
    begin_edit: Option<BeginEditFn>,
    perform_edit: Option<PerformEditFn>,
    end_edit: Option<EndEditFn>,
    resize_view: Option<ResizeViewFn>,
}

impl EditorHost for HostContext {
    fn request_resize(&self, size: Sizef) {
        if let Some(resize_view) = self.resize_view {
            resize_view(self.context, size.width, size.height);
        }
    }

    fn start_parameter_gesture(&self, id: ParameterId) {
        if let Some(begin_edit) = self.begin_edit {
            begin_edit(self.context, id.0);
        }
    }

    fn end_parameter_gesture(&self, id: ParameterId) {
        if let Some(end_edit) = self.end_edit {
            end_edit(self.context, id.0);
        }
    }

    fn set_parameter_normalized(&self, id: ParameterId, value: NormalizedValue) {
        if let Some(perform_edit) = self.perform_edit {
            perform_edit(self.context, id.0, value.get());
        }
    }
}

pub struct EditorBridge {
    editor: Box<dyn PluginEditor>,
    /// Drained once per [`idle`] tick and forwarded to the host via
    /// `host`'s begin/perform/end-edit callbacks -- the path UI widgets use
    /// to report parameter edits (see `mkapk_host::LockFreeParameterGateway`),
    /// distinct from `host`'s direct use as this editor's `EditorHost` in
    /// [`open`].
    gateway: Arc<LockFreeParameterGateway>,
    host: HostContext,
}

pub fn create(
    make_editor: &mut dyn FnMut(Arc<LockFreeParameterGateway>) -> Box<dyn PluginEditor>,
) -> *mut c_void {
    let gateway = Arc::new(LockFreeParameterGateway::default());
    let bridge = Box::new(EditorBridge {
        editor: make_editor(gateway.clone()),
        gateway,
        host: HostContext::default(),
    });
    Box::into_raw(bridge) as *mut c_void
}

/// # Safety
/// `handle` must be a still-live pointer previously returned by [`create`],
/// not yet destroyed.
pub unsafe fn destroy(handle: *mut c_void) {
    unsafe {
        drop(Box::from_raw(handle as *mut EditorBridge));
    }
}

/// # Safety
/// `handle` must be a still-live pointer previously returned by [`create`].
unsafe fn with_bridge<'a>(handle: *mut c_void) -> &'a mut EditorBridge {
    unsafe { &mut *(handle as *mut EditorBridge) }
}

/// # Safety
/// `handle` must be a still-live pointer previously returned by [`create`];
/// `parent` must be a valid native parent view/window handle for the
/// current platform.
pub unsafe fn open(handle: *mut c_void, parent: *mut c_void) {
    let bridge = unsafe { with_bridge(handle) };
    let parent_handle = {
        #[cfg(target_os = "windows")]
        {
            ParentWindowHandle::Windows(parent)
        }
        #[cfg(target_os = "macos")]
        {
            ParentWindowHandle::Mac(parent)
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let _ = parent;
            return;
        }
    };
    bridge.editor.open(parent_handle, &bridge.host);
}

/// # Safety
/// `handle` must be a still-live pointer previously returned by [`create`].
pub unsafe fn close(handle: *mut c_void) {
    unsafe {
        with_bridge(handle).editor.close();
    }
}

/// # Safety
/// `handle` must be a still-live pointer previously returned by [`create`];
/// `out_width`/`out_height` must each be valid for one written `f32`.
pub unsafe fn get_size(handle: *mut c_void, out_width: *mut f32, out_height: *mut f32) {
    let bridge = unsafe { with_bridge(handle) };
    let size = bridge
        .editor
        .size_constraints()
        .min_size
        .unwrap_or_else(|| Sizef::new(400.0, 300.0));
    unsafe {
        *out_width = size.width;
        *out_height = size.height;
    }
}

/// # Safety
/// `handle` must be a still-live pointer previously returned by [`create`].
pub unsafe fn resize(handle: *mut c_void, width: f32, height: f32) {
    unsafe {
        with_bridge(handle).editor.resize(Sizef::new(width, height));
    }
}

/// # Safety
/// `handle` must be a still-live pointer previously returned by [`create`].
pub unsafe fn idle(handle: *mut c_void) {
    let bridge = unsafe { with_bridge(handle) };
    let host = &bridge.host;
    bridge.gateway.poll_audio_changes(|msg| {
        if let ParameterMessage::SetNormalized(id, value) = msg {
            host.start_parameter_gesture(id);
            host.set_parameter_normalized(id, value);
            host.end_parameter_gesture(id);
        }
    });
    bridge.editor.idle();
}

/// # Safety
/// `handle` must be a still-live pointer previously returned by [`create`].
pub unsafe fn parameter_changed(handle: *mut c_void, param_id: u32, normalized_value: f64) {
    unsafe {
        with_bridge(handle).editor.on_parameter_changed(
            ParameterId(param_id),
            NormalizedValue::new(normalized_value),
        );
    }
}

/// # Safety
/// `handle` must be a still-live pointer previously returned by [`create`].
/// The function pointers, if non-null, must remain valid for the editor's
/// lifetime -- upheld by this crate's C++ shim, the only caller.
pub unsafe fn set_host_context(
    handle: *mut c_void,
    context: *mut c_void,
    begin_edit: Option<BeginEditFn>,
    perform_edit: Option<PerformEditFn>,
    end_edit: Option<EndEditFn>,
    resize_view: Option<ResizeViewFn>,
) {
    let bridge = unsafe { with_bridge(handle) };
    bridge.host = HostContext {
        context,
        begin_edit,
        perform_edit,
        end_edit,
        resize_view,
    };
}
