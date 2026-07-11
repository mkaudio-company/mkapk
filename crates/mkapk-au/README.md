# mkapk-au

Real AUv2 plugin entry point for [mkapk](https://github.com/mkaudio-company/mkapk): a hand-written `AudioComponentPlugInInterface` dispatch (via `au-sys`) bridged into any `mkapk_host::Processor` + `PluginEditor` pair via the `au_entry!` macro, plus an `AUCocoaUIBase`-ready Cocoa UI — no plugin-specific dispatch code needed.

macOS only. Unlike `mkapk-vst3`/`mkapk-aax`, this needs no separate SDK — `AudioToolbox` ships with the OS — so the real entry point is available unconditionally, including when this crate is installed as a normal crates.io dependency.

Validated against Apple's own `auval` — see the [workspace README](https://github.com/mkaudio-company/mkapk#readme) for the full architecture and validation details.
