// -*- mode: c++-mode
#pragma once
#include <stdint.h>

enum SPIBitOrder { MSBFIRST };
enum SPIMode { SPI_MODE0 };

class SPISettings {
public:
  SPISettings(int, SPIBitOrder, SPIMode);
};

class SPIClass {
public:
  void begin();
  void beginTransaction(const SPISettings);
  uint8_t transfer(uint8_t);
  void endTransaction();
  void end();
};


extern SPIClass SPI;
