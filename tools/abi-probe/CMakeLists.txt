cmake_minimum_required(VERSION 3.20)
project(mo_rime_abi_probe LANGUAGES C)

if(NOT DEFINED RIME_INCLUDE_DIR)
  message(FATAL_ERROR "Set RIME_INCLUDE_DIR to the directory containing the official rime_api.h")
endif()

if(NOT EXISTS "${RIME_INCLUDE_DIR}/rime_api.h")
  message(FATAL_ERROR "${RIME_INCLUDE_DIR}/rime_api.h does not exist")
endif()

add_executable(mo_rime_abi_probe probe.c)
target_include_directories(mo_rime_abi_probe PRIVATE "${RIME_INCLUDE_DIR}")
set_target_properties(mo_rime_abi_probe PROPERTIES
  C_STANDARD 11
  C_STANDARD_REQUIRED YES
  C_EXTENSIONS NO
)

if(MSVC)
  target_compile_options(mo_rime_abi_probe PRIVATE /W4 /WX)
else()
  target_compile_options(mo_rime_abi_probe PRIVATE -Wall -Wextra -Wpedantic -Werror)
endif()

