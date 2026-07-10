//! Real VST3 `IComponent` + `IAudioProcessor` implementation, bridging any
//! `gui_host::Processor` into a loadable VST3 plugin's audio-processing
//! side. Not generic over the concrete `Processor` type (COM vtable
//! generation via `#[VST3(implements(...))]` needs a concrete struct); any
//! plugin project supplies its processor as a `Box<dyn Processor>`.
use std::cell::RefCell;
use std::mem;
use std::os::raw::c_void;

use gui_host::{NormalizedValue, ParameterId, Processor};
use vst3_com::sys::GUID;
use vst3_com::{IID, VstPtr};
use vst3_sys::VST3;
use vst3_sys::base::{
    IBStream, IPluginBase, IUnknown, TBool, kInvalidArgument, kNotImplemented, kResultFalse,
    kResultOk, kResultTrue, tresult,
};
use vst3_sys::utils::SharedVstPtr;
use vst3_sys::vst::{
    BusDirection, BusInfo, IAudioProcessor, IComponent, IParamValueQueue, IParameterChanges,
    IProcessContextRequirements, IoMode, K_SAMPLE32, MediaType, ProcessData, ProcessSetup,
    RoutingInfo, SpeakerArrangement,
};

use crate::util::wstrcpy;

struct AudioBus {
    name: String,
    speaker_arr: SpeakerArrangement,
    active: TBool,
}

fn speaker_arrangement_for_channels(channels: u32) -> SpeakerArrangement {
    match channels {
        0 => 0,
        1 => 1, // mono
        _ => 3, // stereo (left | right)
    }
}

fn count_channels(arr: SpeakerArrangement) -> i32 {
    let mut arr = arr;
    let mut count = 0;
    while arr != 0 {
        count += (arr & 1) as i32;
        arr >>= 1;
    }
    count
}

/// Real `IComponent` + `IAudioProcessor` bridging a boxed
/// `gui_host::Processor` into VST3's audio-processing contract.
/// `IProcessContextRequirements` has been mandatory since VST SDK 3.7;
/// confirmed via the real Steinberg validator (`validator.exe`/`validator`,
/// built from `vst3sdk`'s own CMake project), which otherwise fails a
/// component with "Missing mandatory IProcessContextRequirements
/// extension!" even though every other test passes.
#[VST3(implements(IComponent, IAudioProcessor, IProcessContextRequirements))]
pub struct VstAudioProcessor {
    processor: RefCell<Box<dyn Processor>>,
    controller_cid: GUID,
    process_setup: RefCell<ProcessSetup>,
    audio_inputs: RefCell<Vec<AudioBus>>,
    audio_outputs: RefCell<Vec<AudioBus>>,
    context: RefCell<Option<VstPtr<dyn IUnknown>>>,
}

impl VstAudioProcessor {
    pub fn new(controller_cid: GUID, processor: Box<dyn Processor>) -> Box<Self> {
        Self::allocate(
            RefCell::new(processor),
            controller_cid,
            RefCell::new(ProcessSetup {
                process_mode: 0,
                symbolic_sample_size: K_SAMPLE32,
                max_samples_per_block: 0,
                sample_rate: 0.0,
            }),
            RefCell::new(Vec::new()),
            RefCell::new(Vec::new()),
            RefCell::new(None),
        )
    }

    pub fn create_instance(controller_cid: GUID, processor: Box<dyn Processor>) -> *mut c_void {
        Box::into_raw(Self::new(controller_cid, processor)) as *mut c_void
    }
}

impl IPluginBase for VstAudioProcessor {
    unsafe fn initialize(&self, context: *mut c_void) -> tresult {
        if self.context.borrow().is_some() || context.is_null() {
            return kResultFalse;
        }
        *self.context.borrow_mut() = unsafe { VstPtr::shared(context as *mut _) };

        let layout = self.processor.borrow().channel_layout();
        if layout.input_channels > 0 {
            self.audio_inputs.borrow_mut().push(AudioBus {
                name: "Input".to_string(),
                speaker_arr: speaker_arrangement_for_channels(layout.input_channels),
                active: true as TBool,
            });
        }
        if layout.output_channels > 0 {
            self.audio_outputs.borrow_mut().push(AudioBus {
                name: "Output".to_string(),
                speaker_arr: speaker_arrangement_for_channels(layout.output_channels),
                active: true as TBool,
            });
        }

        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        self.audio_inputs.borrow_mut().clear();
        self.audio_outputs.borrow_mut().clear();
        *self.context.borrow_mut() = None;
        kResultOk
    }
}

impl IComponent for VstAudioProcessor {
    unsafe fn get_controller_class_id(&self, tuid: *mut IID) -> tresult {
        unsafe {
            *tuid = self.controller_cid;
        }
        kResultOk
    }

    unsafe fn set_io_mode(&self, _mode: IoMode) -> tresult {
        kNotImplemented
    }

    unsafe fn get_bus_count(&self, type_: MediaType, dir: BusDirection) -> i32 {
        if type_ != 0 {
            return 0;
        }
        if dir == 0 {
            self.audio_inputs.borrow().len() as i32
        } else {
            self.audio_outputs.borrow().len() as i32
        }
    }

    unsafe fn get_bus_info(
        &self,
        type_: MediaType,
        dir: BusDirection,
        index: i32,
        info: *mut BusInfo,
    ) -> tresult {
        if type_ != 0 {
            return kResultFalse;
        }
        let buses = if dir == 0 {
            self.audio_inputs.borrow()
        } else {
            self.audio_outputs.borrow()
        };
        if index < 0 || index as usize >= buses.len() {
            return kInvalidArgument;
        }
        let bus = &buses[index as usize];
        unsafe {
            let info = &mut *info;
            info.media_type = type_;
            info.direction = dir;
            info.channel_count = count_channels(bus.speaker_arr);
            wstrcpy(&bus.name, info.name.as_mut_ptr());
            info.bus_type = 0; // kMain
            info.flags = 1; // kDefaultActive
        }
        kResultTrue
    }

    unsafe fn get_routing_info(
        &self,
        _in_info: *mut RoutingInfo,
        _out_info: *mut RoutingInfo,
    ) -> tresult {
        kNotImplemented
    }

    unsafe fn activate_bus(
        &self,
        type_: MediaType,
        dir: BusDirection,
        index: i32,
        state: TBool,
    ) -> tresult {
        if type_ != 0 {
            return kInvalidArgument;
        }
        let mut buses = if dir == 0 {
            self.audio_inputs.borrow_mut()
        } else {
            self.audio_outputs.borrow_mut()
        };
        if index < 0 || index as usize >= buses.len() {
            return kInvalidArgument;
        }
        buses[index as usize].active = state;
        kResultTrue
    }

    unsafe fn set_active(&self, _state: TBool) -> tresult {
        kResultOk
    }

    unsafe fn set_state(&self, state: SharedVstPtr<dyn IBStream>) -> tresult {
        let Some(state) = state.upgrade() else {
            return kResultFalse;
        };
        let mut processor = self.processor.borrow_mut();
        let ids: Vec<ParameterId> = processor.parameters().iter().map(|p| p.id).collect();
        for id in ids {
            let mut value = 0.0_f64;
            let mut read = 0;
            unsafe {
                state.read(
                    &mut value as *mut f64 as *mut c_void,
                    mem::size_of::<f64>() as i32,
                    &mut read,
                );
            }
            if read as usize == mem::size_of::<f64>() {
                processor.set_parameter(id, NormalizedValue::new(value));
            }
        }
        kResultOk
    }

    unsafe fn get_state(&self, state: SharedVstPtr<dyn IBStream>) -> tresult {
        let Some(state) = state.upgrade() else {
            return kResultFalse;
        };
        let processor = self.processor.borrow();
        for param in processor.parameters() {
            let value = processor.parameter_value(param.id).get();
            let mut written = 0;
            unsafe {
                state.write(
                    &value as *const f64 as *mut c_void,
                    mem::size_of::<f64>() as i32,
                    &mut written,
                );
            }
        }
        kResultOk
    }
}

impl IAudioProcessor for VstAudioProcessor {
    unsafe fn set_bus_arrangements(
        &self,
        _inputs: *mut SpeakerArrangement,
        _num_ins: i32,
        _outputs: *mut SpeakerArrangement,
        _num_outs: i32,
    ) -> tresult {
        kResultFalse
    }

    unsafe fn get_bus_arrangement(
        &self,
        dir: BusDirection,
        index: i32,
        arr: *mut SpeakerArrangement,
    ) -> tresult {
        let buses = if dir == 0 {
            self.audio_inputs.borrow()
        } else {
            self.audio_outputs.borrow()
        };
        if index < 0 || index as usize >= buses.len() {
            return kResultFalse;
        }
        unsafe {
            *arr = buses[index as usize].speaker_arr;
        }
        kResultTrue
    }

    unsafe fn can_process_sample_size(&self, symbolic_sample_size: i32) -> tresult {
        if symbolic_sample_size == K_SAMPLE32 {
            kResultTrue
        } else {
            kResultFalse
        }
    }

    unsafe fn get_latency_samples(&self) -> u32 {
        0
    }

    unsafe fn setup_processing(&self, setup: *const ProcessSetup) -> tresult {
        let setup = unsafe { &*setup };
        if setup.symbolic_sample_size != K_SAMPLE32 {
            return kResultFalse;
        }
        *self.process_setup.borrow_mut() = *setup;
        self.processor
            .borrow_mut()
            .prepare(setup.sample_rate, setup.max_samples_per_block as usize);
        kResultOk
    }

    unsafe fn set_processing(&self, _state: TBool) -> tresult {
        kResultOk
    }

    unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        let data = unsafe { &mut *data };

        // Apply host-delivered automation (the last point in each queue,
        // matching this crate's block-granular parameter model rather than
        // full sample-accurate automation curves).
        if let Some(param_changes) = data.input_param_changes.upgrade() {
            let count = unsafe { param_changes.get_parameter_count() };
            for i in 0..count {
                if let Some(queue) = unsafe { param_changes.get_parameter_data(i) }.upgrade() {
                    let id = unsafe { queue.get_parameter_id() };
                    let points = unsafe { queue.get_point_count() };
                    if points > 0 {
                        let mut value = 0.0;
                        let mut offset = 0;
                        let ok = unsafe { queue.get_point(points - 1, &mut offset, &mut value) };
                        if ok == kResultTrue {
                            self.processor
                                .borrow_mut()
                                .set_parameter(ParameterId(id), NormalizedValue::new(value));
                        }
                    }
                }
            }
        }

        if data.num_inputs == 0 && data.num_outputs == 0 {
            return kResultOk;
        }

        let num_samples = data.num_samples as usize;
        let mut processor = self.processor.borrow_mut();

        let input_channels = if data.num_inputs > 0 {
            unsafe { (*data.inputs).num_channels as usize }
        } else {
            0
        };
        let output_channels = if data.num_outputs > 0 {
            unsafe { (*data.outputs).num_channels as usize }
        } else {
            0
        };

        let input_ptrs: &[*mut f32] = if input_channels > 0 {
            unsafe {
                std::slice::from_raw_parts(
                    (*data.inputs).buffers as *const *mut f32,
                    input_channels,
                )
            }
        } else {
            &[]
        };
        let output_ptrs: &[*mut f32] = if output_channels > 0 {
            unsafe {
                std::slice::from_raw_parts(
                    (*data.outputs).buffers as *const *mut f32,
                    output_channels,
                )
            }
        } else {
            &[]
        };

        let inputs: Vec<&[f32]> = input_ptrs
            .iter()
            .map(|&p| unsafe { std::slice::from_raw_parts(p, num_samples) })
            .collect();
        let mut outputs: Vec<&mut [f32]> = output_ptrs
            .iter()
            .map(|&p| unsafe { std::slice::from_raw_parts_mut(p, num_samples) })
            .collect();

        processor.process(&inputs, &mut outputs);

        kResultOk
    }

    unsafe fn get_tail_samples(&self) -> u32 {
        0
    }
}

impl IProcessContextRequirements for VstAudioProcessor {
    /// This crate's `Processor::process` never reads tempo, transport
    /// state, or any other `ProcessContext` field (it's given plain sample
    /// buffers only), so it needs none of the optional context data a host
    /// could otherwise spend time computing/passing per block.
    unsafe fn get_process_context_requirements(&self) -> u32 {
        0
    }
}
