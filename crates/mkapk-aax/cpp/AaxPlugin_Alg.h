/// Real-time algorithm context, generic across any `gui_host::Processor`
/// with at most `kAaxGeneric_MaxParams` parameters -- modeled on the shape
/// of the AAX SDK's own `DemoGain_Alg.h`, but with a fixed array of
/// parameter-value slots instead of one hand-named field per parameter.
///
/// The actual per-sample math lives in Rust (whichever `Processor` the
/// plugin project built, via `gui_aax_process_block`), not here: this file
/// only describes the real-time data AAX delivers into/out of that call.
#pragma once
#ifndef AAXPLUGIN_ALG_H
#define AAXPLUGIN_ALG_H

#include "AAX.h"
#include "AAX_IMIDINode.h"
#include "gui_aax_bridge.h"

enum EAaxGeneric_MeterTaps
{
    eMeterTap_PreGain = 0,
    eMeterTap_PostGain,

    eMeterTap_Count
};

// Fixed, generic meter-tap type IDs -- an input/output level meter is a
// reasonable default for any processor and isn't derived from Rust; it's
// internal bookkeeping for this shim, not part of a plugin's identity.
constexpr AAX_CTypeID kAaxGeneric_MeterID[eMeterTap_Count] = {
    AAX_CTypeID('mtrI'),
    AAX_CTypeID('mtrO'),
};

#include AAX_ALIGN_FILE_BEGIN
#include AAX_ALIGN_FILE_ALG
#include AAX_ALIGN_FILE_END
// Context structure. `mParamValueP` holds one pointer per registered
// parameter packet; only the first `gui_aax_parameter_count()` entries are
// ever read (all context fields are declared up-front because AAX's field
// indices are computed at compile time from this struct's layout, but
// unused trailing slots are simply never dereferenced). `mMIDINodeInP` is
// only populated by AAX (via `AddMIDINode`) when `gui_aax_accepts_midi()`
// is true -- reading it otherwise would dereference an AAX-side field that
// was never wired up, so `AaxGeneric_AlgorithmProcessFunction` gates on the
// same getter before touching it.
struct SAaxGeneric_Alg_Context
{
    int32_t* mCtrlBypassP;                        // Master bypass control message
    float* mParamValueP[kAaxGeneric_MaxParams];    // One packet per parameter
    AAX_IMIDINode* mMIDINodeInP;                   // MIDI input node (only if accepts_midi)

    float** mInputPP;    // Audio signal destination
    float** mOutputPP;   // Audio signal source
    int32_t* mBufferSize;

    float** mMetersPP;   // Meter taps
};
#include AAX_ALIGN_FILE_BEGIN
#include AAX_ALIGN_FILE_RESET
#include AAX_ALIGN_FILE_END

// Physical addresses within the context. `eAlgPortID_Param0 + i` is the
// field index for `mParamValueP[i]` -- valid because array elements are
// consecutive pointer-sized slots, so each subsequent index is exactly one
// field-index unit past the last.
enum EAaxGeneric_Alg_PortID
{
    eAlgPortID_BypassIn = AAX_FIELD_INDEX(SAaxGeneric_Alg_Context, mCtrlBypassP),
    eAlgPortID_Param0 = AAX_FIELD_INDEX(SAaxGeneric_Alg_Context, mParamValueP[0]),
    eAlgPortID_MIDINodeIn = AAX_FIELD_INDEX(SAaxGeneric_Alg_Context, mMIDINodeInP),

    eAlgFieldID_AudioIn = AAX_FIELD_INDEX(SAaxGeneric_Alg_Context, mInputPP),
    eAlgFieldID_AudioOut = AAX_FIELD_INDEX(SAaxGeneric_Alg_Context, mOutputPP),
    eAlgFieldID_BufferSize = AAX_FIELD_INDEX(SAaxGeneric_Alg_Context, mBufferSize),

    eAlgFieldID_Meters = AAX_FIELD_INDEX(SAaxGeneric_Alg_Context, mMetersPP),
};

void AAX_CALLBACK AaxGeneric_AlgorithmProcessFunction(SAaxGeneric_Alg_Context* const inInstancesBegin[],
                                                        const void* inInstancesEnd);

#endif  // AAXPLUGIN_ALG_H
