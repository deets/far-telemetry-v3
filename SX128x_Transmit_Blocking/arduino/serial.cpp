#include "Serial.h"
#include <format>
#include <iostream>
#include <sstream>
#include <string_view>

SerialClass Serial;

void SerialClass::begin(int) {}


void SerialClass::print(const char * message) {
  std::cout << message;
}

void SerialClass::println(const char * message) {
  std::cout << message << "\n";
}

void SerialClass::print(const float v) {
  std::cout << v;
}

void SerialClass::println(const String& message) {
  std::cout << message << "\n";
}

void SerialClass::println(int value, int base) {
  switch (base) {
  case 10:
    std::cout << std::format("{}", value) << "\n";
    break;
  case 16:
    std::cout << std::format("{:X}", value) << "\n";
    break;
  }
}
