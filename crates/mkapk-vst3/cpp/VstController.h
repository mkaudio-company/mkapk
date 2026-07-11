/// Generic `EditControllerEx1` implementation: registers whatever
/// parameters `mkapk_vst3_parameter_count()`/`_id`/`_name`/`_default`/
/// `_step_count` report, with no plugin-specific code (see `VstBridge.h`).
/// `getParameterCount`/`getParameterInfo`/`getParamNormalized`/
/// `setParamNormalized`/`getParamStringByValue`/`normalizedParamToPlain`/
/// `plainParamToNormalized`/`setComponentHandler` are all handled correctly
/// by `EditController`'s own default implementation (backed by the
/// `Parameter` objects `initialize` registers), so this class doesn't
/// override them. Also implements `IMidiMapping` so hosts can route a MIDI
/// CC directly to a parameter's own automation lane.
#pragma once
#ifndef MKAPK_VST3_CONTROLLER_H
#define MKAPK_VST3_CONTROLLER_H

#include "public.sdk/source/vst/vsteditcontroller.h"

class VstController : public Steinberg::Vst::EditControllerEx1,
                       public Steinberg::Vst::IMidiMapping {
public:
    static Steinberg::FUnknown* createInstance(void* /*context*/) {
        return static_cast<Steinberg::Vst::IEditController*>(new VstController());
    }

    Steinberg::tresult PLUGIN_API initialize(Steinberg::FUnknown* context) SMTG_OVERRIDE;
    Steinberg::IPlugView* PLUGIN_API createView(Steinberg::FIDString name) SMTG_OVERRIDE;

    Steinberg::tresult PLUGIN_API getMidiControllerAssignment(
        Steinberg::int32 busIndex, Steinberg::int16 channel,
        Steinberg::Vst::CtrlNumber midiControllerNumber,
        Steinberg::Vst::ParamID& id) SMTG_OVERRIDE;

    DELEGATE_REFCOUNT(EditControllerEx1)
    Steinberg::tresult PLUGIN_API queryInterface(const Steinberg::TUID iid, void** obj) SMTG_OVERRIDE;
};

#endif  // MKAPK_VST3_CONTROLLER_H
