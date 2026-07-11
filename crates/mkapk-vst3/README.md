# mkapk-vst3

Real VST3 plugin entry point for [mkapk](https://github.com/mkaudio-company/mkapk): a generic C++ shim (`cpp/`) deriving from Steinberg's own VST3 SDK helper classes (`AudioEffect`/`EditControllerEx1`), bridged into any `mkapk_host::Processor` + `PluginEditor` pair via the `vst3_entry!` macro — no plugin-specific C++ needed.

## Installing from crates.io vs. building from source

**This crate builds as a view-only stub when installed as a normal crates.io dependency.** The real entry point needs Steinberg's VST3 SDK (MIT-licensed, vendored as pinned git submodules at `thirdparty/{pluginterfaces,base,public.sdk}` in the [source repository](https://github.com/mkaudio-company/mkapk)), which isn't part of the published package — crates.io doesn't preserve git submodule boundaries when packaging, and vendoring the ~23MB SDK subset directly would blow past its 10MB upload limit.

To get a real, loadable `.vst3`, build from a git checkout of the workspace instead:

```bash
git clone https://github.com/mkaudio-company/mkapk
cd mkapk
git submodule update --init crates/mkapk-vst3/thirdparty/pluginterfaces \
  crates/mkapk-vst3/thirdparty/base crates/mkapk-vst3/thirdparty/public.sdk
cargo xtask bundle-vst3
```

Validated 47/47 against Steinberg's own `validator` — see the [workspace README](https://github.com/mkaudio-company/mkapk#readme) for the full architecture and validation details.
