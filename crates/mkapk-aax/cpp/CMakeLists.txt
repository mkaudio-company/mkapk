cmake_minimum_required(VERSION 3.13)

find_package(AAX_SDK REQUIRED)

project(AaxPlugin VERSION 1.0.0)

if(NOT GUI_AAX_RUST_STATICLIB)
	message(FATAL_ERROR "GUI_AAX_RUST_STATICLIB must be set to the path of the "
		"Rust staticlib exporting the mkapk_aax_* bridge functions (built by xtask "
		"before configuring this project)")
endif()

if(NOT AAX_SDK_SOURCE_DIR)
	message(FATAL_ERROR "AAX_SDK_SOURCE_DIR must be set to the AAX SDK's source tree root "
		"(the same directory as AAX_SDK_PATH) -- unlike its headers/libs, "
		"Interfaces/AAX_Exports.cpp (the module's real ACFStartup/ACFRegisterPlugin/etc. "
		"entry points) is not installed by the SDK's own `cmake --install` and must be "
		"compiled directly into every plugin, matching every one of the SDK's own example "
		"Xcode/VS projects (its example CMakeLists.txt files omit this, which appears to be "
		"a gap in the SDK's still-new CMake tooling rather than something optional).")
endif()

# These three describe the specific plugin being bundled (its bundle-level
# metadata, not its DSP/parameters -- see mkapk_aax_bridge.h for that side).
# Every C++ file in this directory is otherwise plugin-agnostic; only these
# defaults need overriding (via -D on the cmake configure command xtask
# runs) for a plugin other than this workspace's own `plugins/gain`.
if(NOT GUI_AAX_OUTPUT_NAME)
	set(GUI_AAX_OUTPUT_NAME "Gain")
endif()
if(NOT GUI_AAX_BUNDLE_IDENTIFIER)
	set(GUI_AAX_BUNDLE_IDENTIFIER "com.mkaudio.aax.gain")
endif()
if(NOT GUI_AAX_COPYRIGHT_STRING)
	set(GUI_AAX_COPYRIGHT_STRING "Copyright 2026 mkaudio")
endif()

if(NOT EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/GeneratedPages.xml")
	message(FATAL_ERROR "GeneratedPages.xml is missing from ${CMAKE_CURRENT_SOURCE_DIR}. "
		"xtask generates it (via `cargo run -p gain-plugin --bin gain-aax-page-table`) "
		"before configuring this project -- see mkapk-aax's `page_table` module and "
		"xtask/src/aax.rs.")
endif()

aax_plugin(TARGET_NAME AaxPlugin
	OUTPUT_NAME ${GUI_AAX_OUTPUT_NAME}
	VERSION ${PROJECT_VERSION}
	COPYRIGHT_STRING ${GUI_AAX_COPYRIGHT_STRING}
	BUNDLE_IDENTIFIER ${GUI_AAX_BUNDLE_IDENTIFIER}

	RESOURCE_FILES
		GeneratedPages.xml
)

target_sources(AaxPlugin
	PRIVATE FILE_SET HEADERS FILES
		AaxPlugin_Alg.h
		AaxPlugin_Describe.h
		AaxPlugin_Parameters.h
		mkapk_aax_bridge.h

	PRIVATE
		AaxPlugin_AlgProc.cpp
		AaxPlugin_Describe.cpp
		AaxPlugin_Parameters.cpp
		${AAX_SDK_SOURCE_DIR}/Interfaces/AAX_Exports.cpp
)

target_include_directories(AaxPlugin PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})

# GUI_AAX_RUST_STATICLIB is the whole plugin crate (its UI code and
# mkapk-mac/mkapk-core dependencies included, not just its Processor), so it
# needs every native library rustc itself would otherwise pass when linking
# that crate's cdylib -- a plain archive doesn't carry that link metadata
# forward automatically. This exact list came from running:
#   cargo rustc -p gain-plugin --features aax --lib --crate-type staticlib \
#     -- --print=native-static-libs
# rather than guessing.
target_link_libraries(AaxPlugin PRIVATE ${GUI_AAX_RUST_STATICLIB})

if(APPLE)
	target_link_libraries(AaxPlugin PRIVATE
		"-framework CoreGraphics"
		"-framework AppKit"
		"-framework QuartzCore"
		"-framework Foundation"
		"-framework CoreText"
		"-framework CoreFoundation"
		objc
		iconv
	)
endif()
