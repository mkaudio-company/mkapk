#include "VstController.h"

#include "VstBridge.h"
#include "VstView.h"

#include <cstring>

using namespace Steinberg;
using namespace Steinberg::Vst;

tresult PLUGIN_API VstController::initialize(FUnknown* context) {
    tresult result = EditControllerEx1::initialize(context);
    if (result != kResultOk) {
        return result;
    }

    const int32_t count = mkapk_vst3_parameter_count();
    for (int32_t i = 0; i < count; ++i) {
        char16_t name[128];
        mkapk_vst3_parameter_name(i, name, 128);
        parameters.addParameter(reinterpret_cast<TChar*>(name), nullptr,
                                 mkapk_vst3_parameter_step_count(i), mkapk_vst3_parameter_default(i),
                                 ParameterInfo::kCanAutomate,
                                 static_cast<int32>(mkapk_vst3_parameter_id(i)));
    }

    return kResultOk;
}

IPlugView* PLUGIN_API VstController::createView(FIDString name) {
    if (!name || std::strcmp(name, ViewType::kEditor) != 0) {
        return nullptr;
    }
    return new VstEditorView(this);
}

tresult PLUGIN_API VstController::getMidiControllerAssignment(int32 busIndex, int16 /*channel*/,
                                                                CtrlNumber midiControllerNumber,
                                                                ParamID& id) {
    // This crate's MIDI-CC automation is global (all channels), single bus.
    if (busIndex != 0) {
        return kResultFalse;
    }
    uint32_t paramId = 0;
    if (mkapk_vst3_midi_cc_to_param(midiControllerNumber, &paramId)) {
        id = paramId;
        return kResultTrue;
    }
    return kResultFalse;
}

tresult PLUGIN_API VstController::queryInterface(const TUID iid, void** obj) {
    QUERY_INTERFACE(iid, obj, IMidiMapping::iid, IMidiMapping)
    return EditControllerEx1::queryInterface(iid, obj);
}
