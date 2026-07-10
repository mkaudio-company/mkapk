//! A small, blocking device-picker window (macOS only) shown before the
//! main plugin window opens, letting the user choose which audio input and
//! output device to use instead of always taking cpal's defaults.
//!
//! This runs mkgraphic's own `App`/`Window` for the picker, in the same
//! process and on the same shared `NSApplication` as the plugin's own
//! window (created afterward via `gui-test-host`). `mkgraphic::host::App`
//! has no supported way to stop its run loop from inside a button/dropdown
//! callback, so `stop_current_app` reaches the same shared `NSApplication`
//! singleton directly via `objc2-app-kit` instead (the same call
//! `mkgraphic::host::MacOSApp::stop` makes internally) -- this does not
//! require holding a reference to mkgraphic's own `App` value.
#![cfg(target_os = "macos")]

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait};
use mkgraphic::prelude::*;
use objc2_app_kit::NSApplication;
use objc2_foundation::MainThreadMarker;

fn stop_current_app() {
    if let Some(mtm) = MainThreadMarker::new() {
        NSApplication::sharedApplication(mtm).stop(None);
    }
}

/// The devices chosen from the picker. `input` is `None` when the user
/// picked "None (silence)" -- a deliberate choice to run without live
/// capture, distinct from "no picker ran at all" (see `build_audio_stream`,
/// which only falls back to a host default when it never got an explicit
/// choice to begin with).
pub struct SelectedDevices {
    pub input: Option<cpal::Device>,
    pub output: cpal::Device,
}

/// Shows a device-picker window and blocks until the user confirms a
/// choice. Returns `None` if there are no output devices, or if this isn't
/// running on the main thread.
pub fn pick_devices() -> Option<SelectedDevices> {
    let host = cpal::default_host();
    let output_devices: Vec<cpal::Device> = host.output_devices().ok()?.collect();
    if output_devices.is_empty() {
        return None;
    }
    let input_devices: Vec<cpal::Device> = host
        .input_devices()
        .map(|devices| devices.collect())
        .unwrap_or_default();

    let output_names: Vec<String> = output_devices
        .iter()
        .enumerate()
        .map(|(i, d)| d.name().unwrap_or_else(|_| format!("Output device {i}")))
        .collect();
    let default_output_index = host
        .default_output_device()
        .and_then(|default| default.name().ok())
        .and_then(|default_name| output_names.iter().position(|n| *n == default_name))
        .unwrap_or(0);

    // "None (silence)" is always the first input option: some machines have
    // no capture device, or the user may not want live input. Picking it is
    // a deliberate opt-out, distinct from just leaving the dropdown alone
    // (which defaults to the host's actual default input device below, when
    // one exists).
    let mut input_names: Vec<String> = vec!["None (silence)".to_string()];
    input_names.extend(
        input_devices
            .iter()
            .enumerate()
            .map(|(i, d)| d.name().unwrap_or_else(|_| format!("Input device {i}"))),
    );
    let default_input_index = host
        .default_input_device()
        .and_then(|default| default.name().ok())
        .and_then(|default_name| input_names.iter().position(|n| *n == default_name))
        .unwrap_or(0);

    let selected_output = Arc::new(Mutex::new(default_output_index));
    let selected_output_for_dropdown = selected_output.clone();
    let selected_input = Arc::new(Mutex::new(default_input_index));
    let selected_input_for_dropdown = selected_input.clone();

    let mut app = App::new();
    let mut window = Window::new("Select Audio I/O", Extent::new(420.0, 240.0));

    let output_name_refs: Vec<&str> = output_names.iter().map(String::as_str).collect();
    let output_picker = dropdown()
        .items(output_name_refs)
        .placeholder(output_names[default_output_index].clone())
        .on_select(move |index| {
            *selected_output_for_dropdown.lock().unwrap() = index;
        });

    let input_name_refs: Vec<&str> = input_names.iter().map(String::as_str).collect();
    let input_picker = dropdown()
        .items(input_name_refs)
        .placeholder(input_names[default_input_index].clone())
        .on_select(move |index| {
            *selected_input_for_dropdown.lock().unwrap() = index;
        });

    let content = vtile![
        label("Choose the audio input and output device, then click Start."),
        label("Input"),
        input_picker,
        label("Output"),
        output_picker,
        button("Start").on_click(stop_current_app),
    ];

    window.set_content(share(content));
    window.show();
    app.run();

    let output_index = *selected_output.lock().unwrap();
    let input_index = *selected_input.lock().unwrap();

    let output = output_devices.into_iter().nth(output_index)?;
    let input = if input_index == 0 {
        None
    } else {
        input_devices.into_iter().nth(input_index - 1)
    };

    Some(SelectedDevices { input, output })
}
