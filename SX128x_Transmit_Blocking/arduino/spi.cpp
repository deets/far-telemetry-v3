#include "SPI.h"

SPIClass SPI;


SPISettings::SPISettings(int, SPIBitOrder, SPIMode) {}
void SPIClass::begin() {}
void SPIClass::beginTransaction(const SPISettings) {}
uint8_t SPIClass::transfer(uint8_t) {
  return 0;
}
void SPIClass::endTransaction() {}
void SPIClass::end() {}


extern SPIClass SPI;
