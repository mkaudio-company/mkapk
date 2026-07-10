/// Generic AAX_CEffectParameters implementation: registers Master Bypass
/// plus whatever parameters `gui_aax_parameter_count()`/`_name`/`_default`/
/// `_step_count` report, with no plugin-specific code. See
/// `AaxPlugin_Parameters.cpp` -- there are no per-parameter callback methods
/// to declare here because AAX_CPacketDispatcher::RegisterPacket's default
/// single-value handler (`GenerateSingleValuePacket`) already does exactly
/// what every one of our parameters needs (copy the current value into its
/// packet), so nothing plugin- or parameter-specific needs hand-writing.
#pragma once
#ifndef AAXPLUGIN_PARAMETERS_H
#define AAXPLUGIN_PARAMETERS_H

#include "AAX_CEffectParameters.h"

class AaxGeneric_Parameters : public AAX_CEffectParameters
{
public:
    AaxGeneric_Parameters(void);
    AAX_DEFAULT_DTOR_OVERRIDE(AaxGeneric_Parameters);

    // Create callback
    static AAX_CEffectParameters* AAX_CALLBACK Create();

public:
    // Overrides from AAX_CEffectParameters
    AAX_Result EffectInit() AAX_OVERRIDE;
};

#endif  // AAXPLUGIN_PARAMETERS_H
