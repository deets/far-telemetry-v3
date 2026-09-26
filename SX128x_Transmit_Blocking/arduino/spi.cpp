#include "SPI.h"
#include <iostream>

SPIClass SPI;


SPISettings::SPISettings(int, SPIBitOrder, SPIMode) {}

void SPIClass::begin() {}

void SPIClass::beginTransaction(const SPISettings) {
  _transfer_after_transaction = true;
}

uint8_t SPIClass::transfer(uint8_t outgoing) {
  if (_transfer_after_transaction) {
    transferAfterTransaction(outgoing);
    _transfer_after_transaction = false;
  } else {
    current().append(outgoing);
  }
  return 0;
}
void SPIClass::endTransaction() {
  std::cout << std::format("{}", current().opcode) << "\n";
}

void SPIClass::end() {}

void SPIClass::transferAfterTransaction(uint8_t outgoing) {
  switch (outgoing) {
  case 0:
    addTransaction(OpCode::NoOp);
    break;
  default:
    unknownOpCode(outgoing);
  }
}

void SPIClass::unknownOpCode(uint8_t outgoing) {
  std::printf("unknownOpCode: 0x%02X\n", outgoing);
}

void SPIClass::addTransaction(OpCode opcode) {
  _transactions.emplace_back(opcode);
}

Transaction &SPIClass::current() {
  return *_transactions.rbegin();
}

extern SPIClass SPI;

std::string_view to_string( OpCode opcode ) {
    switch (opcode) {
    case OpCode::NoOp:
      return "NoOp";
    case OpCode::GetStatus:
      return "GetStatus";
    case OpCode::SetSleep:
      return "SetSleep";
    case OpCode::SetStandby:
      return "SetStandby";
    case OpCode::SetFs:
      return "SetFs";
    case OpCode::SetTx:
      return "SetTx";
    case OpCode::SetRx:
      return "SetRx";
    case OpCode::SetRxDutyCycle:
      return "SetRxDutyCycle";
    case OpCode::SetCad:
      return "SetCad";
    case OpCode::SetTxContinuousWave:
      return "SetTxContinuousWave";
    case OpCode::SetTxContinuousPreamble:
      return "SetTxContinuousPreamble";
    case OpCode::SetAutoFS:
      return "SetAutoFS";
    case OpCode::SetAutoTx:
      return "SetAutoTx";
    case OpCode::WriteRegister:
      return "WriteRegister";
    case OpCode::ReadRegister:
      return "ReadRegister";
    case OpCode::WriteBuffer:
      return "WriteBuffer";
    case OpCode::ReadBuffer:
      return "ReadBuffer";
    case OpCode::SetDioIrqParams:
      return "SetDioIrqParams";
    case OpCode::GetIrqStatus:
      return "GetIrqStatus";
    case OpCode::ClrIrqStatus:
      return "ClrIrqStatus";
    case OpCode::SetPacketType:
      return "SetPacketType";
    case OpCode::GetPacketType:
      return "GetPacketType";
    case OpCode::SetRfFrequency:
      return "SetRfFrequency";
    case OpCode::SetTxParams:
      return "SetTxParams";
    case OpCode::SetCadParams:
      return "SetCadParams";
    case OpCode::SetBufferBaseAddress:
      return "SetBufferBaseAddress";
    case OpCode::SetModulationParams:
      return "SetModulationParams";
    case OpCode::SetPacketParams:
      return "SetPacketParams";
    case OpCode::GetRxBufferStatus:
      return "GetRxBufferStatus";
    case OpCode::GetPacketStatus:
      return "GetPacketStatus";
    case OpCode::GetRssiInst:
      return "GetRssiInst";
    case OpCode::SetLongPreamble:
      return "SetLongPreamble";
    case OpCode::SetRegulatorMode:
      return "SetRegulatorMode";
    case OpCode::SetSaveContext:
      return "SetSaveContext";
    case OpCode::SetUartSpeed:
      return "SetUartSpeed";
    case OpCode::SetRangingRole:
      return "SetRangingRole";
    case OpCode::SetAdvancedRanging:
      return "SetAdvancedRanging";
        break;
    }
    return "<unknown>";
}
