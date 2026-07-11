/// Generic `AudioEffect` implementation: adds whatever audio/MIDI buses
/// `mkapk_vst3_input_channels()`/`_output_channels()`/`_accepts_midi()`
/// report, then forwards `process`/`setState`/`getState` to the bridge's
/// `mkapk_vst3_processor_*` functions -- no plugin-specific code (see
/// `VstBridge.h`). Everything else (`setBusArrangements`, `getBusArrangement`,
/// `canProcessSampleSize`, `setupProcessing`, `setActive`,
/// `getProcessContextRequirements`) is handled correctly by `AudioEffect`'s
/// own default implementation already, so this class doesn't override them.
#pragma once
#ifndef MKAPK_VST3_PROCESSOR_H
#define MKAPK_VST3_PROCESSOR_H

#include "public.sdk/source/vst/vstaudioeffect.h"

class VstProcessor : public Steinberg::Vst::AudioEffect {
public:
    VstProcessor();
    ~VstProcessor() override;

    static Steinberg::FUnknown* createInstance(void* /*context*/) {
        return static_cast<Steinberg::Vst::IAudioProcessor*>(new VstProcessor());
    }

    Steinberg::tresult PLUGIN_API initialize(Steinberg::FUnknown* context) SMTG_OVERRIDE;
    Steinberg::tresult PLUGIN_API terminate() SMTG_OVERRIDE;
    Steinberg::tresult PLUGIN_API setState(Steinberg::IBStream* state) SMTG_OVERRIDE;
    Steinberg::tresult PLUGIN_API getState(Steinberg::IBStream* state) SMTG_OVERRIDE;
    Steinberg::tresult PLUGIN_API process(Steinberg::Vst::ProcessData& data) SMTG_OVERRIDE;

private:
    void* processorHandle = nullptr;
    bool hasMidiInput = false;
};

#endif  // MKAPK_VST3_PROCESSOR_H
