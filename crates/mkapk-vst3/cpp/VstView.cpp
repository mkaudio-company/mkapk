#include "VstView.h"

#include "VstBridge.h"

#include <cstring>

using namespace Steinberg;
using namespace Steinberg::Vst;

void VstEditorView::beginEditThunk(void* context, uint32_t paramId) {
    auto* view = static_cast<VstEditorView*>(context);
    if (auto* handler = view->getController()->getComponentHandler()) {
        handler->beginEdit(paramId);
    }
}

void VstEditorView::performEditThunk(void* context, uint32_t paramId, double value) {
    auto* view = static_cast<VstEditorView*>(context);
    if (auto* handler = view->getController()->getComponentHandler()) {
        handler->performEdit(paramId, value);
    }
}

void VstEditorView::endEditThunk(void* context, uint32_t paramId) {
    auto* view = static_cast<VstEditorView*>(context);
    if (auto* handler = view->getController()->getComponentHandler()) {
        handler->endEdit(paramId);
    }
}

void VstEditorView::resizeViewThunk(void* context, float width, float height) {
    auto* view = static_cast<VstEditorView*>(context);
    ViewRect rect(0, 0, static_cast<int32>(width), static_cast<int32>(height));
    if (view->plugFrame) {
        view->plugFrame->resizeView(view, &rect);
    }
}

VstEditorView::VstEditorView(VstController* controller) : EditorView(controller) {
    editorHandle = mkapk_vst3_editor_create();
    mkapk_vst3_editor_set_host_context(editorHandle, this, &beginEditThunk, &performEditThunk,
                                        &endEditThunk, &resizeViewThunk);

    float width = 400.0f;
    float height = 300.0f;
    mkapk_vst3_editor_get_size(editorHandle, &width, &height);
    setRect(ViewRect(0, 0, static_cast<int32>(width), static_cast<int32>(height)));
}

VstEditorView::~VstEditorView() {
    if (editorHandle) {
        mkapk_vst3_editor_destroy(editorHandle);
    }
}

tresult PLUGIN_API VstEditorView::isPlatformTypeSupported(FIDString type) {
    if (!type) {
        return kInvalidArgument;
    }
#if SMTG_OS_WINDOWS
    if (strcmp(type, kPlatformTypeHWND) == 0) {
        return kResultTrue;
    }
#elif SMTG_OS_MACOS
    if (strcmp(type, kPlatformTypeNSView) == 0) {
        return kResultTrue;
    }
#endif
    return kResultFalse;
}

tresult PLUGIN_API VstEditorView::onSize(ViewRect* newSize) {
    tresult result = EditorView::onSize(newSize);
    if (newSize) {
        const float width = static_cast<float>(newSize->right - newSize->left);
        const float height = static_cast<float>(newSize->bottom - newSize->top);
        mkapk_vst3_editor_resize(editorHandle, width, height);
    }
    return result;
}

void VstEditorView::attachedToParent() {
    mkapk_vst3_editor_open(editorHandle, systemWindow);
}

void VstEditorView::removedFromParent() {
    mkapk_vst3_editor_close(editorHandle);
}
