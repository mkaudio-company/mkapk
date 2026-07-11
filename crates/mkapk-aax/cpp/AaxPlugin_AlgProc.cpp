// Component includes
#include "AaxPlugin_Alg.h"

// AAX includes
#include "AAX.h"

// Standard includes
#include <algorithm>
#include <cmath>
#include <cstring>

//==============================================================================
// Processing function definition -- generic across any Processor with at
// most kAaxGeneric_MaxParams parameters.
//==============================================================================

void AAX_CALLBACK AaxGeneric_AlgorithmProcessFunction(SAaxGeneric_Alg_Context* const inInstancesBegin[],
                                                        const void* inInstancesEnd)
{
    const int32_t numParams = gui_aax_parameter_count();
    const bool acceptsMidi = gui_aax_accepts_midi() != 0;

    for (SAaxGeneric_Alg_Context* const* walk = inInstancesBegin; walk < inInstancesEnd; ++walk)
    {
        SAaxGeneric_Alg_Context* instance = *walk;

        const int32_t bypass = *instance->mCtrlBypassP;
        const int32_t buffersize = *instance->mBufferSize;
        const float* const AAX_RESTRICT pdI = instance->mInputPP[0];
        float* const AAX_RESTRICT pdO = instance->mOutputPP[0];
        float* const meterTaps = *instance->mMetersPP;

        // Gathered here (not inside the `bypass` branch below) so a
        // bypassed block still drains the MIDI node's queued packets --
        // otherwise they'd pile up and all arrive at once the next time
        // the processor is un-bypassed.
        uint8_t midiBytes[kAaxGeneric_MaxMidiMessages * 3];
        int32_t numMidiMessages = 0;
        if (acceptsMidi && instance->mMIDINodeInP != nullptr)
        {
            const AAX_CMidiStream* const midiStream = instance->mMIDINodeInP->GetNodeBuffer();
            if (midiStream != nullptr)
            {
                const AAX_CMidiPacket* packet = midiStream->mBuffer;
                uint32_t remaining = midiStream->mBufferSize;
                while (remaining > 0 && numMidiMessages < kAaxGeneric_MaxMidiMessages)
                {
                    uint8_t* const dest = &midiBytes[numMidiMessages * 3];
                    dest[0] = packet->mData[0];
                    dest[1] = packet->mLength > 1 ? packet->mData[1] : 0;
                    dest[2] = packet->mLength > 2 ? packet->mData[2] : 0;
                    ++numMidiMessages;
                    ++packet;
                    --remaining;
                }
            }
        }

        if (bypass)
        {
            // Bypass means "pass audio through unchanged", not "process
            // with some neutral parameter value" -- unlike a pure gain
            // processor (where gain=1.0 happens to be a passthrough), that
            // isn't true for an arbitrary future Processor, so this shim
            // never routes bypass through gui_aax_process_block at all.
            std::memcpy(pdO, pdI, static_cast<size_t>(buffersize) * sizeof(float));
        }
        else
        {
            float values[kAaxGeneric_MaxParams];
            for (int32_t i = 0; i < numParams; ++i)
            {
                values[i] = *instance->mParamValueP[i];
            }
            gui_aax_process_block(values, numParams, midiBytes, numMidiMessages, pdI, pdO,
                                   buffersize);
        }

        // Accumulate the max value for metering. This will get cleared for
        // us by the shell when it sends the accumulated value up to the
        // host.
        for (int32_t t = 0; t < buffersize; ++t)
        {
            meterTaps[eMeterTap_PreGain] = std::max(std::fabs(pdI[t]), meterTaps[eMeterTap_PreGain]);
            meterTaps[eMeterTap_PostGain] = std::max(std::fabs(pdO[t]), meterTaps[eMeterTap_PostGain]);
        }
    }
}
