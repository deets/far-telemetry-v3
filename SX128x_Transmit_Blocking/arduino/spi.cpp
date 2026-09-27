#include "SPI.h"
#include <iostream>
#include "Arduino.h"

std::string_view to_string( OpCode opcode );

template<>
struct std::formatter<OpCode> : std::formatter<std::string_view> {
    auto format( OpCode opcode, auto& context ) const {
        return std::formatter<std::string_view>::format( to_string( opcode ), context );
    }
};





template<>
class std::formatter<Transaction> {
public:
  constexpr auto parse( auto& context ) {
    return context.begin();
  }

  auto format( const Transaction& transaction, auto& context ) const {
    return std::format_to( context.out(), "OpCode: {} -> {}", transaction.opcode, transaction.outgoing );
  }
};

template<>
class std::formatter<std::vector<uint8_t>> {
public:
  constexpr auto parse( auto& context ) {
    return context.begin();
  }

  auto format(const std::vector<uint8_t> &bytes, auto &context) const {
    std::stringstream ss;
    ss << "[";
    for (const auto &c : bytes) {
      ss << std::format("{:02X}", c) << ", ";
    }
    ss << "]";
    return std::format_to( context.out(), "{}", ss.str() );
  }
};


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
  }
  return current().transact(outgoing);
}
void SPIClass::endTransaction() {
  std::cout << std::format("{}", current()) << "\n";
  current().end();
}

void SPIClass::end() {
}

void SPIClass::transferAfterTransaction(uint8_t outgoing) {
  switch (outgoing) {
  case 0:
    addTransaction(OpCode::NoOp, false);
    break;
  case 0x80:
    addTransaction(OpCode::SetStandby, true);
    break;
  case 0x19:
    addTransaction(OpCode::ReadRegister, false);
  default:
    unknownOpCode(outgoing);
  }
}

void SPIClass::unknownOpCode(uint8_t outgoing) {
  std::printf("unknownOpCode: 0x%02X\n", outgoing);
}

void SPIClass::addTransaction(OpCode opcode, bool is_busy) {
  _transactions.emplace_back(opcode);
  if (is_busy) {
    fakeBusy();
  }
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
