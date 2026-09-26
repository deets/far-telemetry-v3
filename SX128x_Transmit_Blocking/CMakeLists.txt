# Source - https://stackoverflow.com/a/33090791
# Posted by Arne, modified by community. See post 'Timeline' for change history
# Retrieved 2026-09-25, License - CC BY-SA 4.0

cmake_minimum_required(VERSION 3.28)
project(radiolib_test)

###############################################################################
## file globbing ##############################################################
###############################################################################

# these instructions search the directory tree when CMake is
# invoked and put all files that match the pattern in the variables
# `sources` and `data`
set(sources
  SX128x_Transmit_Blocking.cpp
  RadioLib/src/modules/SX128x/SX1281.cpp
  RadioLib/src/modules/SX128x/SX128x.cpp
  RadioLib/src/modules/SX128x/SX1280.cpp
  RadioLib/src/protocols/PhysicalLayer/PhysicalLayer.cpp
  RadioLib/src/hal/Arduino/ArduinoHal.cpp
  RadioLib/src/Hal.cpp
  RadioLib/src/Module.cpp
  RadioLib/src/utils/Utils.cpp
  arduino/spi.cpp
  arduino/serial.cpp
  arduino/arduino.cpp
  )
# you can use set(sources src/main.cpp) etc if you don't want to
# use globbing to find files automatically

###############################################################################
## target definitions #########################################################
###############################################################################

# add the data to the target, so it becomes visible in some IDE
add_executable(radiolib-test ${sources})

target_compile_definitions(radiolib-test PUBLIC RADIOLIB_BUILD_ARDUINO)
target_compile_options(radiolib-test PUBLIC -std=c++23 -Wall)

# this lets me include files relative to the root source directory with a <> pair
target_include_directories(
  radiolib-test PUBLIC
  arduino
  RadioLib/src/modules/SX128x
  RadioLib/src
  )
