# mkapk-standalone

Standalone desktop host for [mkapk](https://github.com/mkaudio-company/mkapk) plugins: real duplex audio I/O via [`cpal`](https://crates.io/crates/cpal) (input + output device selection), real MIDI input via [`midir`](https://crates.io/crates/midir), and a real native window, wiring any `mkapk_host::Processor` + `PluginEditor` pair into a runnable desktop app with no DAW required.

See the [workspace README](https://github.com/mkaudio-company/mkapk#readme) for the full picture of how this fits into building VST3/AU/AAX/Standalone plugins.
