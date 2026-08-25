cmake_minimum_required(VERSION 3.15)
project(httplora_radio)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

add_library(radio_hal STATIC
    radio_hal.cpp
    mock_serial.cpp
    serial_posix.cpp
)

target_include_directories(radio_hal PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
