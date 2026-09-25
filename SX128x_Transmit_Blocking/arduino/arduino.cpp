#include <sstream>

#include "Arduino.h"

void delay(int) {}

std::string to_string(int v, int base) {
  std::stringstream ss;
  ss << v;
  return ss.str();
}

String::String(int v, int base) : std::string(to_string(v, base)) {
}

String operator+(const char * l, const String & r) {
  std::stringstream ss;
  ss << l << r;
  const auto s = ss.str();
  return String(s.begin(), s.end());
}


char pgm_read_byte(const char *p) {
  return *p;
}


void pinMode(uint32_t, int) {}
void digitalWrite(uint32_t, int) {}
int digitalRead(uint32_t) { return 0; }
void attachInterrupt(int, void (*)(void), uint32_t) {}
void detachInterrupt(int) {}
long pulseIn(uint32_t pin, uint32_t state, uint32_t timeout) {
  return -1;
}
void tone(uint32_t pin, unsigned int frequency, uint32_t duration) {}
void noTone(uint32_t pin) {}
void yield() {}
uint32_t digitalPinToInterrupt(uint32_t pin) { return pin; }

unsigned long millis() { return 0;}
unsigned long micros() { return 0;}
void delayMicroseconds(int us) {}
