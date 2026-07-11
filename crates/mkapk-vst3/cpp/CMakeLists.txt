cmake_minimum_required(VERSION 3.13)

project(Vst3Plugin VERSION 1.0.0)

if(NOT GUI_VST3_RUST_STATICLIB)
	message(FATAL_ERROR "GUI_VST3_RUST_STATICLIB must be set to the path of the "
		"Rust staticlib exporting the mkapk_vst3_* bridge functions (built by xtask "
		"before configuring this project)")
endif()

if(NOT VST3_SDK_ROOT)
	set(VST3_SDK_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../thirdparty")
endif()

if(NOT GUI_VST3_OUTPUT_NAME)
	set(GUI_VST3_OUTPUT_NAME "Gain")
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# `xtask` wraps this target's output in the actual `.vst3` bundle shape
# (Info.plist/PkgInfo/codesign), the same way it already does for the AU
# cdylib -- unlike `mkapk-aax`'s CMake project, this one doesn't assemble a
# bundle itself, since we don't vendor the VST3 SDK's own `cmake/` helper
# submodule (skipped: it's SDK build tooling, not part of the MIT-licensed
# `pluginterfaces`/`base`/`public.sdk` subset this crate actually needs).
add_library(Vst3Plugin SHARED
	${CMAKE_CURRENT_SOURCE_DIR}/VstFactory.cpp
	${CMAKE_CURRENT_SOURCE_DIR}/VstProcessor.cpp
	${CMAKE_CURRENT_SOURCE_DIR}/VstController.cpp
	${CMAKE_CURRENT_SOURCE_DIR}/VstView.cpp

	${VST3_SDK_ROOT}/pluginterfaces/base/funknown.cpp
	${VST3_SDK_ROOT}/pluginterfaces/base/coreiids.cpp
	${VST3_SDK_ROOT}/pluginterfaces/base/conststringtable.cpp
	${VST3_SDK_ROOT}/pluginterfaces/base/ustring.cpp

	${VST3_SDK_ROOT}/public.sdk/source/vst/vstcomponentbase.cpp
	${VST3_SDK_ROOT}/public.sdk/source/vst/vstcomponent.cpp
	${VST3_SDK_ROOT}/public.sdk/source/vst/vstaudioeffect.cpp
	${VST3_SDK_ROOT}/public.sdk/source/vst/vsteditcontroller.cpp
	${VST3_SDK_ROOT}/public.sdk/source/vst/vstbus.cpp
	${VST3_SDK_ROOT}/public.sdk/source/vst/vstparameters.cpp
	${VST3_SDK_ROOT}/public.sdk/source/vst/vstinitiids.cpp

	${VST3_SDK_ROOT}/public.sdk/source/main/pluginfactory.cpp

	${VST3_SDK_ROOT}/public.sdk/source/common/pluginview.cpp
	${VST3_SDK_ROOT}/public.sdk/source/common/commonstringconvert.cpp
	${VST3_SDK_ROOT}/public.sdk/source/common/commoniids.cpp

	${VST3_SDK_ROOT}/base/source/fobject.cpp
	${VST3_SDK_ROOT}/base/source/updatehandler.cpp
	${VST3_SDK_ROOT}/base/source/fstring.cpp
	${VST3_SDK_ROOT}/base/source/timer.cpp
	${VST3_SDK_ROOT}/base/source/fdebug.cpp
	${VST3_SDK_ROOT}/base/source/fdynlib.cpp
	${VST3_SDK_ROOT}/base/source/fbuffer.cpp
	${VST3_SDK_ROOT}/base/source/fstreamer.cpp
	${VST3_SDK_ROOT}/base/source/baseiids.cpp
	${VST3_SDK_ROOT}/base/thread/source/flock.cpp
)

if(APPLE)
	target_sources(Vst3Plugin PRIVATE ${VST3_SDK_ROOT}/public.sdk/source/main/macmain.cpp)
elseif(WIN32)
	target_sources(Vst3Plugin PRIVATE ${VST3_SDK_ROOT}/public.sdk/source/main/dllmain.cpp)
else()
	target_sources(Vst3Plugin PRIVATE ${VST3_SDK_ROOT}/public.sdk/source/main/linuxmain.cpp)
endif()

target_include_directories(Vst3Plugin PRIVATE ${VST3_SDK_ROOT} ${CMAKE_CURRENT_SOURCE_DIR})
target_compile_definitions(Vst3Plugin PRIVATE RELEASE=1)

set_target_properties(Vst3Plugin PROPERTIES
	OUTPUT_NAME "${GUI_VST3_OUTPUT_NAME}"
	PREFIX ""
)

# `GUI_VST3_RUST_STATICLIB` is the whole plugin crate (its UI code and
# mkapk-mac/mkapk-core dependencies included, not just its Processor), so it
# needs every native library rustc itself would otherwise pass when linking
# that crate's cdylib -- a plain archive doesn't carry that link metadata
# forward automatically. Same convention as `mkapk-aax/cpp/CMakeLists.txt`.
target_link_libraries(Vst3Plugin PRIVATE ${GUI_VST3_RUST_STATICLIB})

if(APPLE)
	target_link_libraries(Vst3Plugin PRIVATE
		"-framework CoreFoundation"
		"-framework CoreGraphics"
		"-framework AppKit"
		"-framework QuartzCore"
		"-framework Foundation"
		"-framework CoreText"
		objc
		iconv
	)
endif()
