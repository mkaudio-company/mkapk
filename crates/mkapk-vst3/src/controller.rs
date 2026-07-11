//! Real VST3 `IEditController` implementation, bridging any
//! `mkapk_host::PluginEditor` (constructed with a shared
//! `LockFreeParameterGateway`) into VST3's edit-controller contract. Reuses
//! the existing `PluginView` (`crate::view`) for `create_view`.
//!
//! Parameter automation flows two ways here:
//! - UI -> host: the injected `LockFreeParameterGateway` is drained once per
//!   `idle()` tick (see `HostNotifyingEditor`) and forwarded to the host via
//!   `IComponentHandler::{begin_edit,perform_edit,end_edit}`.
//! - host -> audio: handled entirely on the `VstAudioProcessor` side (see
//!   `crate::processor`), which reads `ProcessData::input_param_changes`
//!   directly; it does not go through this controller.
//!
//! Known limitation: host-driven automation is not reflected back into the
//! UI's own widget state in this pass (e.g. a slider dragged by automation
//! played back from the host will not visibly move) -- `IEditController::
//! set_param_normalized` only updates this controller's own cached value,
//! which VST3 hosts do read back via `get_param_normalized` (so automation
//! lanes/displays in the host itself stay correct), but the plugin's own
//! rendered widget does not currently observe it.
use std::cell::RefCell;
use std::os::raw::c_void;
use std::rc::Rc;
use std::sync::Arc;

use mkapk_core::Sizef;
use mkapk_host::{
    EditorHost, LockFreeParameterGateway, NormalizedValue, ParameterId, ParameterInfo,
    ParameterMessage, ParentWindowHandle, PluginEditor, SizeConstraints,
};
use vst3_sys::VST3;
use vst3_sys::base::{
    FIDString, IBStream, IPluginBase, kResultFalse, kResultOk, kResultTrue, tresult,
};
use vst3_sys::utils::SharedVstPtr;
use vst3_sys::vst::{CtrlNumber, IComponentHandler, IEditController, IMidiMapping, ParamID, TChar};

use crate::view::{ComponentHandlerCell, PluginView};

/// Wraps a `PluginEditor` so that parameter changes queued via the shared
/// `LockFreeParameterGateway` (the pattern this project's UI files use,
/// e.g. `plugins/gain/src/ui.rs`'s slider callback) are also reported to
/// the VST3 host as automation, instead of only taking effect in-process.
struct HostNotifyingEditor {
    inner: Box<dyn PluginEditor>,
    gateway: Arc<LockFreeParameterGateway>,
    component_handler: ComponentHandlerCell,
}

impl PluginEditor for HostNotifyingEditor {
    fn open(&mut self, parent: ParentWindowHandle, host: &dyn EditorHost) {
        self.inner.open(parent, host);
    }

    fn close(&mut self) {
        self.inner.close();
    }

    fn resize(&mut self, size: Sizef) {
        self.inner.resize(size);
    }

    fn idle(&mut self) {
        let handler = self.component_handler.clone();
        self.gateway.poll_audio_changes(|msg| {
            if let ParameterMessage::SetNormalized(id, value) = msg {
                if let Some(handler) = handler.borrow().as_ref() {
                    unsafe {
                        handler.begin_edit(id.0);
                        handler.perform_edit(id.0, value.get());
                        handler.end_edit(id.0);
                    }
                }
            }
        });
        self.inner.idle();
    }

    fn on_parameter_changed(&mut self, id: ParameterId, value: NormalizedValue) {
        self.inner.on_parameter_changed(id, value);
    }

    fn size_constraints(&self) -> SizeConstraints {
        self.inner.size_constraints()
    }
}

type EditorFactory = Box<dyn FnOnce(Arc<LockFreeParameterGateway>) -> Box<dyn PluginEditor>>;

/// Real `IEditController` bridging a `mkapk_host::PluginEditor` factory
/// closure into VST3's controller contract. Also implements `IMidiMapping`
/// so hosts can route a MIDI CC directly to a parameter's own automation
/// lane -- VST3's idiomatic mechanism for MIDI-CC automation (there is no
/// raw CC event in `IEventList`; see `crate::processor`'s note/pressure
/// event handling for the MIDI this crate delivers as `handle_midi`
/// instead). Confirmed against the real Steinberg validator.
#[VST3(implements(IEditController, IMidiMapping))]
pub struct VstEditController {
    parameters: RefCell<Vec<(ParameterInfo, f64)>>,
    make_editor: RefCell<Option<EditorFactory>>,
    gateway: Arc<LockFreeParameterGateway>,
    component_handler: ComponentHandlerCell,
    midi_cc_map: &'static [(u8, ParameterId)],
}

impl VstEditController {
    /// `parameters` describes every automatable parameter; `make_editor` is
    /// called once (lazily, from `create_view`) to construct the plugin's
    /// `PluginEditor`, given the gateway it should use for UI-to-host
    /// parameter forwarding. `midi_cc_map` pairs a MIDI CC number with the
    /// parameter it should automate (e.g. `(7, GAIN_PARAM)` for CC 7 ->
    /// gain); an empty slice means this plugin has no MIDI-CC automation.
    pub fn new(
        parameters: Vec<ParameterInfo>,
        make_editor: impl FnOnce(Arc<LockFreeParameterGateway>) -> Box<dyn PluginEditor> + 'static,
        midi_cc_map: &'static [(u8, ParameterId)],
    ) -> Box<Self> {
        let initial: Vec<(ParameterInfo, f64)> = parameters
            .into_iter()
            .map(|info| {
                let default = info.default_value.get();
                (info, default)
            })
            .collect();

        Self::allocate(
            RefCell::new(initial),
            RefCell::new(Some(Box::new(make_editor))),
            Arc::new(LockFreeParameterGateway::default()),
            Rc::new(RefCell::new(None)),
            midi_cc_map,
        )
    }

    pub fn create_instance(
        parameters: Vec<ParameterInfo>,
        make_editor: impl FnOnce(Arc<LockFreeParameterGateway>) -> Box<dyn PluginEditor> + 'static,
        midi_cc_map: &'static [(u8, ParameterId)],
    ) -> *mut c_void {
        Box::into_raw(Self::new(parameters, make_editor, midi_cc_map)) as *mut c_void
    }

    fn param_index(&self, id: u32) -> Option<usize> {
        self.parameters
            .borrow()
            .iter()
            .position(|(info, _)| info.id.0 == id)
    }
}

impl IMidiMapping for VstEditController {
    /// Ignores `bus_index`/`channel` (this crate's MIDI-CC automation is
    /// global, not per-channel) and looks `midi_cc_number` up in
    /// `midi_cc_map`, writing the matching parameter's ID and returning
    /// `kResultTrue` -- from here, the host delivers that CC as ordinary
    /// `IParameterChanges` automation, which `VstAudioProcessor::process`
    /// already applies.
    unsafe fn get_midi_controller_assignment(
        &self,
        _bus_index: i32,
        _channel: i16,
        midi_cc_number: CtrlNumber,
        param_id: *mut ParamID,
    ) -> tresult {
        let Ok(cc) = u8::try_from(midi_cc_number) else {
            return kResultFalse;
        };
        match self
            .midi_cc_map
            .iter()
            .find(|(mapped_cc, _)| *mapped_cc == cc)
        {
            Some((_, mapped_param)) => {
                unsafe {
                    *param_id = mapped_param.0;
                }
                kResultTrue
            }
            None => kResultFalse,
        }
    }
}

impl IPluginBase for VstEditController {
    unsafe fn initialize(&self, _context: *mut c_void) -> tresult {
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        kResultOk
    }
}

impl IEditController for VstEditController {
    unsafe fn set_component_state(&self, _state: SharedVstPtr<dyn IBStream>) -> tresult {
        kResultOk
    }

    unsafe fn set_state(&self, _state: SharedVstPtr<dyn IBStream>) -> tresult {
        kResultOk
    }

    unsafe fn get_state(&self, _state: SharedVstPtr<dyn IBStream>) -> tresult {
        kResultOk
    }

    unsafe fn get_parameter_count(&self) -> i32 {
        self.parameters.borrow().len() as i32
    }

    unsafe fn get_parameter_info(
        &self,
        param_index: i32,
        info: *mut vst3_sys::vst::ParameterInfo,
    ) -> tresult {
        let params = self.parameters.borrow();
        if param_index < 0 || param_index as usize >= params.len() {
            return kResultFalse;
        }
        let (param, _) = &params[param_index as usize];
        unsafe {
            let out = &mut *info;
            out.id = param.id.0;
            crate::util::wstrcpy(param.name, out.title.as_mut_ptr());
            crate::util::wstrcpy(param.name, out.short_title.as_mut_ptr());
            out.units = [0; 128];
            out.step_count = param.step_count.unwrap_or(0) as i32;
            out.default_normalized_value = param.default_value.get();
            out.unit_id = 0;
            out.flags = 1; // kCanAutomate
        }
        kResultTrue
    }

    unsafe fn get_param_string_by_value(
        &self,
        id: u32,
        value_normalized: f64,
        string: *mut TChar,
    ) -> tresult {
        let text = format!("{:.0}", value_normalized * 100.0);
        unsafe {
            crate::util::wstrcpy(&text, string);
        }
        let _ = id;
        kResultTrue
    }

    unsafe fn get_param_value_by_string(
        &self,
        _id: u32,
        _string: *const TChar,
        _value_normalized: *mut f64,
    ) -> tresult {
        kResultFalse
    }

    unsafe fn normalized_param_to_plain(&self, _id: u32, value_normalized: f64) -> f64 {
        value_normalized * 100.0
    }

    unsafe fn plain_param_to_normalized(&self, _id: u32, plain_value: f64) -> f64 {
        plain_value / 100.0
    }

    unsafe fn get_param_normalized(&self, id: u32) -> f64 {
        match self.param_index(id) {
            Some(index) => self.parameters.borrow()[index].1,
            None => 0.0,
        }
    }

    unsafe fn set_param_normalized(&self, id: u32, value: f64) -> tresult {
        match self.param_index(id) {
            Some(index) => {
                self.parameters.borrow_mut()[index].1 = value;
                kResultTrue
            }
            None => kResultFalse,
        }
    }

    unsafe fn set_component_handler(
        &self,
        handler: SharedVstPtr<dyn IComponentHandler>,
    ) -> tresult {
        *self.component_handler.borrow_mut() = handler.upgrade();
        kResultOk
    }

    unsafe fn create_view(&self, _name: FIDString) -> *mut c_void {
        let Some(make_editor) = self.make_editor.borrow_mut().take() else {
            // A view was already created once; VST3 hosts only ever
            // request a single view per controller instance in practice.
            return std::ptr::null_mut();
        };
        let editor = make_editor(self.gateway.clone());
        let wrapped = HostNotifyingEditor {
            inner: editor,
            gateway: self.gateway.clone(),
            component_handler: self.component_handler.clone(),
        };
        Box::into_raw(PluginView::with_component_handler(
            Box::new(wrapped),
            self.component_handler.clone(),
        )) as *mut c_void
    }
}
