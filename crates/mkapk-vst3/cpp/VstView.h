/// Generic `EditorView` (VST3's `CPluginView`-based helper) implementation:
/// bridges `attached`/`removed`/`onSize`/host callbacks into the
/// `mkapk_vst3_editor_*` bridge functions (see `VstBridge.h`), with no
/// plugin-specific code. `getSize` is handled by `CPluginView`'s own
/// default implementation (tracks `rect`, seeded from the editor's real
/// size in the constructor), so this class doesn't override it.
#pragma once
#ifndef MKAPK_VST3_VIEW_H
#define MKAPK_VST3_VIEW_H

#include "VstController.h"

#include "public.sdk/source/vst/vsteditcontroller.h"

class VstEditorView : public Steinberg::Vst::EditorView {
public:
    explicit VstEditorView(VstController* controller);
    ~VstEditorView() override;

    Steinberg::tresult PLUGIN_API isPlatformTypeSupported(Steinberg::FIDString type) SMTG_OVERRIDE;
    Steinberg::tresult PLUGIN_API onSize(Steinberg::ViewRect* newSize) SMTG_OVERRIDE;
    Steinberg::tresult PLUGIN_API canResize() SMTG_OVERRIDE { return Steinberg::kResultTrue; }
    void attachedToParent() SMTG_OVERRIDE;
    void removedFromParent() SMTG_OVERRIDE;

private:
    // Host-callback thunks registered with the bridge (see
    // `mkapk_vst3_editor_set_host_context`) -- member functions (not free
    // functions) so they can reach `plugFrame` (`CPluginView`'s protected
    // member) to forward `resizeView` calls.
    static void beginEditThunk(void* context, uint32_t paramId);
    static void performEditThunk(void* context, uint32_t paramId, double value);
    static void endEditThunk(void* context, uint32_t paramId);
    static void resizeViewThunk(void* context, float width, float height);

    void* editorHandle = nullptr;
};

#endif  // MKAPK_VST3_VIEW_H
