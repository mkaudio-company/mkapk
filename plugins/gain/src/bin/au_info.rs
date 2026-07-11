//! Prints this plugin's Audio Unit component type (the 4-character
//! `aufx`/`aumf`/`aumu` code) to stdout, for `xtask bundle-au` to capture
//! before writing the `AudioComponents` `Info.plist` entry. AU hosts only
//! ever route real MIDI to a Music Effect (`aumf`) or Music Device
//! (`aumu`) component, never a plain Effect (`aufx`) -- so this can't be a
//! fixed constant the way the other format bundlers are, since it depends
//! on `GainProcessor::accepts_midi`/`plugin_kind`, which only exist as
//! runtime methods on the concrete processor.
use mkapk_host::{PluginKind, Processor as _};

fn main() {
    let processor = gain_plugin::GainProcessor::new();
    let component_type = if !processor.accepts_midi() {
        "aufx"
    } else if processor.plugin_kind() == PluginKind::Instrument {
        "aumu"
    } else {
        "aumf"
    };
    print!("{component_type}");
}
