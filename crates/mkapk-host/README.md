# mkapk-host

Host-agnostic plugin traits for [mkapk](https://github.com/mkaudio-company/mkapk): `Processor` (the DSP contract every plugin format wraps) and `PluginEditor`/`EditorHost` (the UI contract), plus a lock-free UI/audio parameter gateway (`LockFreeParameterGateway`) and a lock-free `PeakMeter`.

`#![deny(unsafe_code)]` — every format-specific entry point (`mkapk-vst3`, `mkapk-au`, `mkapk-aax`, `mkapk-standalone`) bridges into these traits without this crate itself touching any plugin ABI.

See the [workspace README](https://github.com/mkaudio-company/mkapk#readme) for the full picture of how this fits into building VST3/AU/AAX/Standalone plugins.
