#include "VstProcessor.h"

#include "VstBridge.h"

#include "pluginterfaces/base/ibstream.h"
#include "pluginterfaces/vst/ivstevents.h"
#include "pluginterfaces/vst/ivstparameterchanges.h"
#include "pluginterfaces/vst/vstspeaker.h"

#include <algorithm>
#include <cmath>
#include <cstring>

using namespace Steinberg;
using namespace Steinberg::Vst;

VstProcessor::VstProcessor() {
    TUID controllerCid;
    std::memcpy(controllerCid, mkapk_vst3_controller_cid(), sizeof(TUID));
    setControllerClass(controllerCid);
}

VstProcessor::~VstProcessor() {}

tresult PLUGIN_API VstProcessor::initialize(FUnknown* context) {
    tresult result = AudioEffect::initialize(context);
    if (result != kResultOk) {
        return result;
    }

    const uint32_t inputChannels = mkapk_vst3_input_channels();
    const uint32_t outputChannels = mkapk_vst3_output_channels();
    if (inputChannels > 0) {
        addAudioInput(u"Input",
                      inputChannels == 1 ? SpeakerArr::kMono : SpeakerArr::kStereo);
    }
    if (outputChannels > 0) {
        addAudioOutput(u"Output",
                       outputChannels == 1 ? SpeakerArr::kMono : SpeakerArr::kStereo);
    }

    hasMidiInput = mkapk_vst3_accepts_midi() != 0;
    if (hasMidiInput) {
        addEventInput(u"MIDI In", 16);
    }

    processorHandle = mkapk_vst3_processor_create();
    return kResultOk;
}

tresult PLUGIN_API VstProcessor::terminate() {
    if (processorHandle) {
        mkapk_vst3_processor_destroy(processorHandle);
        processorHandle = nullptr;
    }
    return AudioEffect::terminate();
}

tresult PLUGIN_API VstProcessor::setState(IBStream* state) {
    if (!state) {
        return kResultFalse;
    }
    const int32_t count = mkapk_vst3_parameter_count();
    for (int32_t i = 0; i < count; ++i) {
        double value = 0.0;
        int32 bytesRead = 0;
        state->read(&value, sizeof(double), &bytesRead);
        if (bytesRead == sizeof(double)) {
            mkapk_vst3_processor_set_parameter(processorHandle, mkapk_vst3_parameter_id(i), value);
        }
    }
    return kResultOk;
}

tresult PLUGIN_API VstProcessor::getState(IBStream* state) {
    if (!state) {
        return kResultFalse;
    }
    const int32_t count = mkapk_vst3_parameter_count();
    for (int32_t i = 0; i < count; ++i) {
        const double value =
            mkapk_vst3_processor_get_parameter(processorHandle, mkapk_vst3_parameter_id(i));
        int32 bytesWritten = 0;
        state->write(const_cast<double*>(&value), sizeof(double), &bytesWritten);
    }
    return kResultOk;
}

tresult PLUGIN_API VstProcessor::process(ProcessData& data) {
    // Apply host-delivered automation (the last point in each queue,
    // matching this crate's block-granular parameter model rather than
    // full sample-accurate automation curves).
    if (data.inputParameterChanges) {
        const int32 paramCount = data.inputParameterChanges->getParameterCount();
        for (int32 i = 0; i < paramCount; ++i) {
            IParamValueQueue* queue = data.inputParameterChanges->getParameterData(i);
            if (!queue) {
                continue;
            }
            const int32 pointCount = queue->getPointCount();
            if (pointCount <= 0) {
                continue;
            }
            int32 offset = 0;
            double value = 0.0;
            if (queue->getPoint(pointCount - 1, offset, value) == kResultTrue) {
                mkapk_vst3_processor_set_parameter(processorHandle, queue->getParameterId(), value);
            }
        }
    }

    // Real MIDI note/pressure events, only read at all when the processor
    // declared a MIDI input bus (see `initialize`) -- matching this crate's
    // block-granular model, every queued event is applied via
    // `mkapk_vst3_processor_handle_midi` before `process`, not interpolated
    // to its exact `sampleOffset` within the block.
    if (hasMidiInput && data.inputEvents) {
        const int32 eventCount = data.inputEvents->getEventCount();
        for (int32 i = 0; i < eventCount; ++i) {
            Event event{};
            if (data.inputEvents->getEvent(i, event) != kResultTrue) {
                continue;
            }
            if (event.type == Event::kNoteOnEvent) {
                const auto& noteOn = event.noteOn;
                const uint8_t channel = static_cast<uint8_t>(std::max<int16>(noteOn.channel, 0)) & 0x0F;
                const uint8_t note =
                    static_cast<uint8_t>(std::clamp<int16>(noteOn.pitch, 0, 127));
                const uint8_t velocity = static_cast<uint8_t>(
                    std::clamp(std::lround(noteOn.velocity * 127.0f), 0L, 127L));
                mkapk_vst3_processor_handle_midi(processorHandle, 0x90 | channel, note, velocity);
            } else if (event.type == Event::kNoteOffEvent) {
                const auto& noteOff = event.noteOff;
                const uint8_t channel =
                    static_cast<uint8_t>(std::max<int16>(noteOff.channel, 0)) & 0x0F;
                const uint8_t note =
                    static_cast<uint8_t>(std::clamp<int16>(noteOff.pitch, 0, 127));
                const uint8_t velocity = static_cast<uint8_t>(
                    std::clamp(std::lround(noteOff.velocity * 127.0f), 0L, 127L));
                mkapk_vst3_processor_handle_midi(processorHandle, 0x80 | channel, note, velocity);
            } else if (event.type == Event::kPolyPressureEvent) {
                const auto& polyPressure = event.polyPressure;
                const uint8_t channel =
                    static_cast<uint8_t>(std::max<int16>(polyPressure.channel, 0)) & 0x0F;
                const uint8_t note =
                    static_cast<uint8_t>(std::clamp<int16>(polyPressure.pitch, 0, 127));
                const uint8_t pressure = static_cast<uint8_t>(
                    std::clamp(std::lround(polyPressure.pressure * 127.0f), 0L, 127L));
                mkapk_vst3_processor_handle_midi(processorHandle, 0xA0 | channel, note, pressure);
            }
        }
    }

    if (data.numInputs == 0 && data.numOutputs == 0) {
        return kResultOk;
    }

    const float* const* inputs =
        data.numInputs > 0 ? const_cast<const float* const*>(data.inputs[0].channelBuffers32)
                           : nullptr;
    float* const* outputs =
        data.numOutputs > 0 ? data.outputs[0].channelBuffers32 : nullptr;
    const int32_t inputChannels = data.numInputs > 0 ? data.inputs[0].numChannels : 0;
    const int32_t outputChannels = data.numOutputs > 0 ? data.outputs[0].numChannels : 0;

    mkapk_vst3_processor_process(processorHandle, inputs, inputChannels, outputs, outputChannels,
                                  data.numSamples);

    return kResultOk;
}
