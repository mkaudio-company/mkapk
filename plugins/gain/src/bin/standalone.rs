//! Standalone desktop build of the gain plugin: real audio device output
//! and a real window, both driving the exact same `GainProcessor`/`GainEditor`
//! used by the VST3/AU/AAX builds.
use std::sync::Arc;

use gain_plugin::{GainEditor, GainProcessor};
use mkapk_host::{LockFreeParameterGateway, PeakMeter};
use mkapk_standalone::StandaloneConfig;

fn main() {
    let gateway = Arc::new(LockFreeParameterGateway::default());
    let meter = PeakMeter::new();
    let processor = GainProcessor::new();
    let editor = GainEditor::new(gateway.clone(), meter.clone());

    mkapk_standalone::run(
        processor,
        gateway,
        meter,
        editor,
        StandaloneConfig::default(),
    );
}
