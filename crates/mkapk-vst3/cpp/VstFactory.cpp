/// Standard `CPluginFactory` registration, replicating what
/// `public.sdk`'s own `BEGIN_FACTORY`/`DEF_VST3_CLASS`/`END_FACTORY` macros
/// expand to -- written out by hand rather than using those macros because
/// they assume compile-time literal `TUID`s, whereas every value here
/// (including both class IDs) comes from the bridge at plugin-load time
/// (see `VstBridge.h`), populated per-plugin by `mkapk_vst3::vst3_entry!`.
#include "VstBridge.h"
#include "VstController.h"
#include "VstProcessor.h"

#include "public.sdk/source/main/pluginfactory.h"

#include <cstring>

using namespace Steinberg;
using namespace Steinberg::Vst;

// Required by `public.sdk`'s platform main files (`macmain.cpp`/
// `dllmain.cpp`/`linuxmain.cpp`) -- called once when the module is
// loaded/unloaded. Nothing in this bridge needs process-wide setup beyond
// what each `VstProcessor`/`VstController` instance already does in its own
// `initialize`/`terminate`.
bool InitModule() {
    return true;
}

bool DeinitModule() {
    return true;
}

SMTG_EXPORT_SYMBOL IPluginFactory* PLUGIN_API GetPluginFactory() {
    if (gPluginFactory) {
        gPluginFactory->addRef();
        return gPluginFactory;
    }

    static PFactoryInfo factoryInfo(mkapk_vst3_vendor_name(), mkapk_vst3_url(),
                                     mkapk_vst3_email(), PFactoryInfo::kUnicode);
    gPluginFactory = new CPluginFactory(factoryInfo);

    TUID processorCid;
    std::memcpy(processorCid, mkapk_vst3_processor_cid(), sizeof(TUID));
    TUID controllerCid;
    std::memcpy(controllerCid, mkapk_vst3_controller_cid(), sizeof(TUID));

    static PClassInfo2 processorClass(processorCid, PClassInfo::kManyInstances,
                                       kVstAudioEffectClass, mkapk_vst3_plugin_name(), 0, "Fx",
                                       nullptr, mkapk_vst3_version(), kVstVersionString);
    gPluginFactory->registerClass(&processorClass, VstProcessor::createInstance);

    static PClassInfo2 controllerClass(controllerCid, PClassInfo::kManyInstances,
                                        kVstComponentControllerClass, mkapk_vst3_plugin_name(), 0,
                                        "", nullptr, mkapk_vst3_version(), kVstVersionString);
    gPluginFactory->registerClass(&controllerClass, VstController::createInstance);

    return gPluginFactory;
}
