// -*- mode: c++-mode
#pragma once
#include "common.h"

class SerialClass {
public:
  void begin(int baud);

  void print(const char *);
  void println(const char *);
  void println(int, int base = DEC);
  void println(const String &);
  void print(const float);
};


extern SerialClass Serial;
