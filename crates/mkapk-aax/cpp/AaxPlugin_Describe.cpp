// Component Includes
#include "AaxPlugin_Alg.h"
#include "AaxPlugin_Describe.h"
#include "AaxPlugin_Parameters.h"
#include "mkapk_aax_bridge.h"

// AAX Includes
#include "AAX_Assert.h"
#include "AAX_Errors.h"
#include "AAX_Exception.h"
#include "AAX_ICollection.h"
#include "AAX_IComponentDescriptor.h"
#include "AAX_IEffectDescriptor.h"
#include "AAX_IPropertyMap.h"

// ***************************************************************************
// ROUTINE:	DescribeAlgorithmComponent
//
// Generic across any Processor: registers Master Bypass plus one
// AddDataInPort per parameter slot, instead of a plugin hand-listing its
// own ports.
//
// Registers all kAaxGeneric_MaxParams slots unconditionally -- not just the
// `mkapk_aax_parameter_count()` actually in use -- because the real AAX
// Plug-In Validator (`test.load_unload`) warns "Algorithm context contains
// gaps between registered fields" when SAaxGeneric_Alg_Context declares
// more pointer slots than are registered as ports (confirmed empirically).
// Slots beyond the real parameter count are simply never read by
// AaxGeneric_AlgorithmProcessFunction.
// ***************************************************************************
static void DescribeAlgorithmComponent(AAX_IComponentDescriptor* outDesc)
{
    AAX_CheckedResult err;

    err = outDesc->AddAudioIn(eAlgFieldID_AudioIn);
    err = outDesc->AddAudioOut(eAlgFieldID_AudioOut);
    err = outDesc->AddAudioBufferLength(eAlgFieldID_BufferSize);
    err = outDesc->AddMeters(eAlgFieldID_Meters, kAaxGeneric_MeterID, eMeterTap_Count);

    err = outDesc->AddDataInPort(eAlgPortID_BypassIn, sizeof(int32_t));

    for (int32_t i = 0; i < kAaxGeneric_MaxParams; ++i)
    {
        err = outDesc->AddDataInPort(eAlgPortID_Param0 + i, sizeof(float));
    }

    // Only registered when the processor actually wants MIDI (see
    // `mkapk_host::Processor::accepts_midi`) -- unlike the parameter slots
    // above, a plain effect with no use for MIDI shouldn't show a "MIDI In"
    // node in the host at all. Native-only (per this shim's own scope):
    // the AAX SDK's own docs note MIDI isn't delivered to DSP algorithms,
    // only AAX Native, which is exactly what `Gain_Defs.h`'s FourCC-only
    // (no TI) identity already committed to.
    if (mkapk_aax_accepts_midi())
    {
        err = outDesc->AddMIDINode(eAlgPortID_MIDINodeIn, AAX_eMIDINodeType_LocalInput, "MIDI In",
                                    0xFFFF);
    }

    AAX_IPropertyMap* const properties = outDesc->NewPropertyMap();
    if (!properties)
        err = AAX_ERROR_NULL_OBJECT;

    err = properties->AddProperty(AAX_eProperty_ManufacturerID, mkapk_aax_manufacturer_id());
    err = properties->AddProperty(AAX_eProperty_ProductID, mkapk_aax_product_id());
    err = properties->AddProperty(AAX_eProperty_CanBypass, true);
    err = properties->AddProperty(AAX_eProperty_UsesClientGUI, true);  // Host's generic (auto) GUI

    err = properties->AddProperty(AAX_eProperty_InputStemFormat, AAX_eStemFormat_Mono);
    err = properties->AddProperty(AAX_eProperty_OutputStemFormat, AAX_eStemFormat_Mono);

    err = properties->AddProperty(AAX_eProperty_PlugInID_Native, mkapk_aax_plugin_id_native());

    err = properties->AddPointerProperty(AAX_eProperty_NativeProcessProc,
                                          reinterpret_cast<const void*>(&AaxGeneric_AlgorithmProcessFunction));

    err = outDesc->AddProcessProc(properties);
}

// ***************************************************************************
// ROUTINE:	DescribeEffect
// ***************************************************************************
static AAX_Result DescribeEffect(AAX_IEffectDescriptor* outDescriptor)
{
    AAX_CheckedResult err;
    AAX_IComponentDescriptor* const compDesc = outDescriptor->NewComponentDescriptor();
    if (!compDesc)
        err = AAX_ERROR_NULL_OBJECT;

    err = outDescriptor->AddName(reinterpret_cast<const char*>(mkapk_aax_plugin_name()));
    err = outDescriptor->AddCategory(AAX_ePlugInCategory_Effect);

    err = compDesc->Clear();
    DescribeAlgorithmComponent(compDesc);
    err = outDescriptor->AddComponent(compDesc);

    err = outDescriptor->AddProcPtr(reinterpret_cast<void*>(AaxGeneric_Parameters::Create),
                                    kAAX_ProcPtrID_Create_EffectParameters);

    // The page table's content is per-plugin (its knobIDs are
    // parameter-name-specific), but the file *name* is a fixed convention
    // ("GeneratedPages.xml") -- xtask generates its content at build time
    // (see mkapk-aax's `page_table` module) rather than a plugin
    // hand-maintaining a static XML resource. Confirmed via the real AAX
    // Plug-In Validator that omitting this resource entirely fails
    // test.parameter_traversal.linear ("Failed to load page tables
    // library"), so it isn't optional the way a first pass assumed.
    err = outDescriptor->AddResourceInfo(AAX_eResourceType_PageTable, "GeneratedPages.xml");

    {
        AAX_IPropertyMap* const meterProperties = outDescriptor->NewPropertyMap();
        if (!meterProperties)
            err = AAX_ERROR_NULL_OBJECT;

        err = meterProperties->AddProperty(AAX_eProperty_Meter_Type, AAX_eMeterType_Input);
        err = meterProperties->AddProperty(AAX_eProperty_Meter_Orientation, AAX_eMeterOrientation_Default);
        err = outDescriptor->AddMeterDescription(kAaxGeneric_MeterID[eMeterTap_PreGain], "Input", meterProperties);
    }
    {
        AAX_IPropertyMap* const meterProperties = outDescriptor->NewPropertyMap();
        if (!meterProperties)
            err = AAX_ERROR_NULL_OBJECT;

        err = meterProperties->AddProperty(AAX_eProperty_Meter_Type, AAX_eMeterType_Output);
        err = meterProperties->AddProperty(AAX_eProperty_Meter_Orientation, AAX_eMeterOrientation_Default);
        err = outDescriptor->AddMeterDescription(kAaxGeneric_MeterID[eMeterTap_PostGain], "Output", meterProperties);
    }

    return err;
}

// ***************************************************************************
// ROUTINE:	GetEffectDescriptions
// ***************************************************************************
AAX_Result GetEffectDescriptions(AAX_ICollection* outCollection)
{
    AAX_CheckedResult err;
    AAX_IEffectDescriptor* const plugInDescriptor = outCollection->NewDescriptor();
    if (plugInDescriptor)
    {
        AAX_SWALLOW_MULT(err = DescribeEffect(plugInDescriptor);
                          err = outCollection->AddEffect(reinterpret_cast<const char*>(mkapk_aax_effect_id()),
                                                          plugInDescriptor););
    }
    else
        err = AAX_ERROR_NULL_OBJECT;

    err = outCollection->SetManufacturerName(reinterpret_cast<const char*>(mkapk_aax_manufacturer_name()));
    err = outCollection->AddPackageName(reinterpret_cast<const char*>(mkapk_aax_plugin_name()));
    err = outCollection->SetPackageVersion(1);

    return err;
}
