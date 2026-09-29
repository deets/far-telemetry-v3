// -*- mode: c++-mode
#pragma once
#include <string>
#include <stdint.h>
#include "common.h"
#include "Serial.h"

void delay(int ms);
void delayMicroseconds(int us);
unsigned long millis();
unsigned long micros();
using __FlashStringHelper = char *;
using PGM_P = char*;
char pgm_read_byte(const char*);

enum IODecls {
  INPUT,
  OUTPUT,
  LOW,
  HIGH,
  RISING,
  FALLING
};

void pinMode(uint32_t, int);
void digitalWrite(uint32_t, int);
int digitalRead(uint32_t);
void attachInterrupt(int, void (*)(void), uint32_t);
void detachInterrupt(int);
long pulseIn(uint32_t pin, uint32_t state, uint32_t timeout);
void tone(uint32_t pin, unsigned int frequency, uint32_t duration);
void noTone(uint32_t pin);
void yield();
uint32_t digitalPinToInterrupt(uint32_t pin);

void declareBusyPin(int);
void declareIrqPin(int);
void fakeBusy();
void setIrqMask(uint16_t);
uint16_t getIrqMask();
