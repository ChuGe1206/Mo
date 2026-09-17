cmake_minimum_required(VERSION 3.25)
project(mo_windows_tip LANGUAGES CXX)
option(MO_LATENCY_TRACE "Development-only input timing metadata" OFF)

if(NOT WIN32 OR NOT MSVC)
    message(FATAL_ERROR "The Phase 0 TSF shell requires MSVC and the Windows SDK.")
endif()

function(mo_apply_compile_policy target)
    if(MO_LATENCY_TRACE)
        target_compile_definitions(${target} PRIVATE MO_LATENCY_TRACE)
    endif()
    target_compile_features(${target} PRIVATE cxx_std_17)
    target_compile_options(${target} PRIVATE /W4 /WX /permissive- /sdl /guard:cf /utf-8)
    target_link_options(${target} PRIVATE /guard:cf)
    target_include_directories(${target} PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/include")
endfunction()

add_library(mo_tip SHARED src/mo_broker_client.cpp src/mo_candidate_window.cpp src/mo_tip.cpp src/mo_tip.def)
mo_apply_compile_policy(mo_tip)
target_link_libraries(mo_tip PRIVATE advapi32 gdi32 ole32 uuid)
set_target_properties(mo_tip PROPERTIES OUTPUT_NAME "mo_tip")

add_executable(mo_tip_registrar src/registrar.cpp)
mo_apply_compile_policy(mo_tip_registrar)
target_link_libraries(mo_tip_registrar PRIVATE advapi32 ole32 uuid)

add_executable(mo_tip_abi_probe probe/abi_probe.cpp)
mo_apply_compile_policy(mo_tip_abi_probe)
target_link_libraries(mo_tip_abi_probe PRIVATE ole32 uuid)

add_executable(mo_tip_ipc_probe probe/ipc_probe.cpp src/mo_broker_client.cpp)
mo_apply_compile_policy(mo_tip_ipc_probe)
