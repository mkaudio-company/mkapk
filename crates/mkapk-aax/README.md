# mkapk-aax

Real AAX plugin entry point for [mkapk](https://github.com/mkaudio-company/mkapk): a generic, plugin-agnostic C++ shim (`cpp/`) built against Avid's AAX SDK, bridged into any `mkapk_host::Processor` via the `aax_entry!` macro and a page-table generator (`page_table` module) — no plugin-specific C++ needed.

## Installing from crates.io vs. building from source

**This crate builds as a view-only stub without the AAX SDK.** Avid's AAX SDK isn't vendored here and isn't fetched automatically — it's dual-licensed (a commercial agreement with Avid, or GPL v3; see [developer.avid.com/aax](https://developer.avid.com/aax)) and must be obtained separately. Set `AAX_SDK_PATH` at build time to enable the real entry point:

```bash
git clone https://github.com/mkaudio-company/mkapk
cd mkapk
AAX_SDK_PATH=/path/to/your/aax-sdk cargo xtask bundle-aax
```

Validated 6/6 against Avid's own `AAXValidator.framework` — see the [workspace README](https://github.com/mkaudio-company/mkapk#readme) for the full architecture and validation details.
