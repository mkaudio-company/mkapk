// Self Include
#include "AaxPlugin_Parameters.h"

// Component includes
#include "AaxPlugin_Alg.h"
#include "gui_aax_bridge.h"

// AAX Includes
#include "AAX_Assert.h"
#include "AAX_CBinaryDisplayDelegate.h"
#include "AAX_CBinaryTaperDelegate.h"
#include "AAX_CLinearTaperDelegate.h"
#include "AAX_CNumberDisplayDelegate.h"

// Standard Includes
#include <cstdio>
#include <memory>

// *******************************************************************************
// ROUTINE:	Create
// *******************************************************************************
AAX_CEffectParameters* AAX_CALLBACK AaxGeneric_Parameters::Create()
{
    return new AaxGeneric_Parameters();
}

// *******************************************************************************
// METHOD:	AaxGeneric_Parameters
// *******************************************************************************
AaxGeneric_Parameters::AaxGeneric_Parameters() : AAX_CEffectParameters() {}

// *******************************************************************************
// METHOD:	EffectInit
// *******************************************************************************
AAX_Result AaxGeneric_Parameters::EffectInit()
{
    // Master Bypass is an AAX-standard control, not one of the Rust
    // processor's own parameters (gui_host::Processor has no notion of
    // bypass -- see AaxPlugin_AlgProc.cpp, which passes audio straight
    // through on bypass rather than calling into the processor at all).
    {
        AAX_CString id = cDefaultMasterBypassID;
        std::unique_ptr<AAX_IParameter> param(new AAX_CParameter<bool>(
            id, AAX_CString("Master Bypass"), false, AAX_CBinaryTaperDelegate<bool>(),
            AAX_CBinaryDisplayDelegate<bool>("bypass", "on"), true));
        param->SetNumberOfSteps(2);
        param->SetType(AAX_eParameterType_Discrete);
        mParameterManager.AddParameter(param.release());
        mPacketDispatcher.RegisterPacket(id.CString(), eAlgPortID_BypassIn);
    }

    // Every parameter the Rust processor exposes, driven entirely by data
    // queried from Rust -- nothing below this point is plugin-specific.
    // AAX_CPacketDispatcher's default single-value handler
    // (RegisterPacket's 2-argument overload) already copies each
    // parameter's current value into its packet with no custom code, so
    // there is no per-parameter callback to write.
    const int32_t numParams = gui_aax_parameter_count();
    for (int32_t i = 0; i < numParams && i < kAaxGeneric_MaxParams; ++i)
    {
        char nameBuf[64];
        gui_aax_parameter_name(i, nameBuf, sizeof(nameBuf));

        char idBuf[16];
        std::snprintf(idBuf, sizeof(idBuf), "p%u", gui_aax_parameter_id(i));

        const int32_t stepCount = gui_aax_parameter_step_count(i);
        const float defaultValue = gui_aax_parameter_default(i);

        AAX_CString id(idBuf);
        std::unique_ptr<AAX_IParameter> param(new AAX_CParameter<float>(
            id, AAX_CString(nameBuf), defaultValue, AAX_CLinearTaperDelegate<float>(0.0f, 1.0f),
            AAX_CNumberDisplayDelegate<float>(), true));
        if (stepCount > 0)
        {
            param->SetNumberOfSteps(static_cast<uint32_t>(stepCount));
            param->SetType(AAX_eParameterType_Discrete);
        }
        else
        {
            param->SetType(AAX_eParameterType_Continuous);
        }
        mParameterManager.AddParameter(param.release());
        mPacketDispatcher.RegisterPacket(id.CString(), eAlgPortID_Param0 + i);
    }

    return AAX_SUCCESS;
}
