#include "SPI.h"
#include <cstdint>
#include <iostream>
#include "Arduino.h"
#include <format>
#include <iostream>
#include <sstream>
#include <string_view>

std::string_view to_string( OpCode opcode );
std::string_view to_string( Register opcode );
std::string_view to_string( PacketType opcode );

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


SPISettings::SPISettings(int, SPIBitOrder, SPIMode) {
}

void SPIClass::begin() {
   // b'SX1280 V3B A9B7\x00'
  _registers.insert(
      std::pair(Register::VersionString,
                std::vector<uint8_t>{83, 88, 49, 50, 56, 48, 32, 86, 51, 66, 32,
                                     65, 57, 66, 55, 0}));
  // This is for some reason read before written, so let's set it.
  _registers.insert(
    std::pair(Register::FrequencyErrorCorrection,
                std::vector<uint8_t>{0}));
  // Header contained a CRC, and also this is
  // the highest byte of the frequency error.
  _registers.insert(
    std::pair(Register::HeaderCrc,
              std::vector<uint8_t>{1 << 4}));
  // Frequency error, one by one. My abstraction breaks a bit
  // here as it could be read continously, but this is how radiolib does it.
  _registers.insert(
    std::pair(Register::FeiByte1M,
              std::vector<uint8_t>{0}));
  _registers.insert(
    std::pair(Register::FeiByte0L,
              std::vector<uint8_t>{16}));

}

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
    addTransaction(OpCode::ReadRegister, true);
    break;
  case 0x8F:
    addTransaction(OpCode::SetBufferBaseAddress, true);
    break;
  case 0x8A:
    addTransaction(OpCode::SetPacketType, true);
    break;
  case 0x88:
    addTransaction(OpCode::SetCadParams, true);
    break;
  case 0x96:
    addTransaction(OpCode::SetRegulatorMode, true);
    break;
  case 0x86:
    addTransaction(OpCode::SetRfFrequency, true);
    break;
  case 0x03:
    addTransaction(OpCode::GetPacketType, true);
    break;
  case 0x8B:
    addTransaction(OpCode::SetModulationParams, true);
    break;
  case 0x18:
    addTransaction(OpCode::WriteRegister, true);
    break;
  case 0xC0:
    addTransaction(OpCode::GetStatus, true);
    break;
  case 0x8C:
    addTransaction(OpCode::SetPacketParams, true);
    break;
  case 0x8E:
    addTransaction(OpCode::SetTxParams, true);
    break;
  case 0x1A:
    addTransaction(OpCode::WriteBuffer, true);
    break;
  case 0x8D:
    addTransaction(OpCode::SetDioIrqParams, true);
    break;
  case 0x97:
    addTransaction(OpCode::ClrIrqStatus, true);
    setIrqMask(0);
    break;
  case 0x83:
    addTransaction(OpCode::SetTx, true);
    break;
  case 0x82:
    addTransaction(OpCode::SetRx, true);
    setIrqMask(0x0002);
    break;
  case 0x15:
    addTransaction(OpCode::GetIrqStatus, true);
    break;
  case 0x17:
    addTransaction(OpCode::GetRxBufferStatus, true);
    break;
  case 0x1B:
    addTransaction(OpCode::ReadBuffer, true);
    break;
  case 0x1D:
    addTransaction(OpCode::GetPacketStatus, true);
    break;
  default:
    unknownOpCode(outgoing);
  }
}

void SPIClass::unknownOpCode(uint8_t outgoing) {
  std::printf("unknownOpCode: 0x%02X\n", outgoing);
}

void SPIClass::addTransaction(OpCode opcode, bool is_busy) {
  _transactions.emplace_back(opcode, _registers, _packet_type);
  if (is_busy) {
    fakeBusy();
  }
}

Transaction &SPIClass::current() {
  return *_transactions.rbegin();
}

uint8_t Transaction::transact(uint8_t outgoing) {
    if(open) {
      this->outgoing.push_back(outgoing);
      switch (opcode) {
      case OpCode::ReadRegister:
        return readRegister();
      case OpCode::WriteRegister:
        return writeRegister();
      case OpCode::SetStandby:
      // I don't fully understand this yet
      // but it appears that the RadioLib reads the status from
      // byte 1 of the respone, so let's mimic that.
        return 0x43;
      case OpCode::SetPacketType:
        return setPacketType();
      case OpCode::GetPacketType:
        return getPacketType();
      case OpCode::GetIrqStatus:
        return getIrqStatus();
      case OpCode::GetRxBufferStatus:
        return getRxBufferStatus();
      case OpCode::ReadBuffer:
        return readBuffer();
      case OpCode::GetPacketStatus:
        return getPacketStatus();
      default:
        // Lets assume this is a generally accepted status
        return 0x43;
      }
    } else {
      throw std::runtime_error("Transaction closed");
    }
}

uint8_t Transaction::setPacketType() {
  if (count() == 2)
  {
      packet_type = static_cast<PacketType>(outgoing[1]);

  }
  return 0x43;
}

uint8_t Transaction::getPacketType() {
  if (count() == 3) {
    return static_cast<uint8_t>(packet_type);
  }
  return 0x43;
}

uint8_t Transaction::getIrqStatus() {
  // 4 bytes, OpCode, Nop, IrqStatus upper half, lowerhalf
  if (count() == 3) {
    return getIrqMask() >> 8;
  }
  if (count() == 4) {
    uint16_t mask = getIrqMask();
    // clear the mask
    setIrqMask(0);
    return mask & 0xff;
  }
  return 0x43;
}

uint8_t Transaction::getRxBufferStatus() {
  // 4 bytes, OpCode, Nop, payload len, payload offset
  // len
  if (count() == 3) {
    return 16;
  }
  //offset
  if (count() == 4) {
    return 0;
  }
  return 0x43;
}

uint8_t Transaction::readBuffer() {
  // OpCode, offset, nop
  // len
  if (count() < 4) {
    return 0x43;
  }
  return count();
}

uint8_t Transaction::getPacketStatus() {
  // OpCode, nop, 5 status bytes packet dependent
  // rssi
  if (count() == 3) {
    return 20;
  }
  // SNR
  if (count() == 4) {
    return 10;
  }
  if (count() > 4) {
    return 0;
  }
  return 0x43;
}

uint8_t Transaction::readRegister() {
    // register reads write 4 bytes,
    // opcode, high, low, and a nop
    // before they start returning the actual value.
  switch (count()) {
  case 1:
    return 0x43;
  case 2:
    return 0;
  case 3: {
    Register reg = static_cast<Register>(outgoing[1] << 8 | outgoing[2]);
    register_to_work_on = reg;
    return 0;
  }
  case 4:
    return 0;
  default:
    if (!register_to_work_on) {
      throw std::runtime_error("No register");
    }
    if (!registers.contains(*register_to_work_on)) {
      throw std::runtime_error(std::format("Unknown register: {}", to_string(*register_to_work_on)));
    }
    return registers[*register_to_work_on][count() - 5];
  }
  return 0;
}

uint8_t Transaction::writeRegister() {
  // register reads write 3 bytes,
  // opcode, high, low
  switch (count()) {
  case 1:
  case 2:
    break;
  case 3: {
    Register reg = static_cast<Register>(outgoing[1] << 8 | outgoing[2]);
    register_to_work_on = reg;
  }
  }
  // I assume we always return the status
  return 0x43;
}

void Transaction::end() {
  open = false;
  switch (opcode) {
  case OpCode::WriteRegister:
    storeRegister();
    break;
  default:
    break;
  }
}

void Transaction::storeRegister() {
  if (!register_to_work_on) {
    throw std::runtime_error("No register set to work on");
  }
  if (outgoing.size() <= 3) {
    throw std::runtime_error("No register values to store");
  }
  std::vector<uint8_t> value{outgoing.begin() + 3, outgoing.end()};
  registers.insert(std::pair(*register_to_work_on, value));
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
    return "<unknown OpCode>";
}

std::string_view to_string( Register reg ) {
    switch (reg) {
    case Register::FirmwareVersions:
      return "FirmwareVersions";
    case Register::VersionString:
      return "VersionString";
    case Register::RxGain:
      return "RxGain";
    case Register::ManualGainSetting:
      return "ManualGainSetting";
    case Register::LnaGainValue:
      return "LnaGainValue";
    case Register::LnaGainControl:
      return "LnaGainControl";
    case Register::SynchPeakAttenuation:
      return "SynchPeakAttenuation";
    case Register::PayloadLength:
      return "PayloadLength";
    case Register::LoRaHeaderMode:
      return "LoRaHeaderMode";
    case Register::RangingRequestAddressByte3:
      return "RangingRequestAddressByte3";
    case Register::RangingRequestAddressByte2:
      return "RangingRequestAddressByte2";
    case Register::RangingRequestAddressByte1:
      return "RangingRequestAddressByte1";
    case Register::RangingRequestAddressByte0:
      return "RangingRequestAddressByte0";
    case Register::RangingDeviceAddressByte3:
      return "RangingDeviceAddressByte3";
    case Register::RangingDeviceAddressByte2:
      return "RangingDeviceAddressByte2";
    case Register::RangingDeviceAddressByte1:
      return "RangingDeviceAddressByte1";
    case Register::RangingDeviceAddressByte0:
      return "RangingDeviceAddressByte0";
    case Register::RangingFilterWindowSize:
      return "RangingFilterWindowSize";
    case Register::ResetRangingFilter:
      return "ResetRangingFilter";
    case Register::RangingResultMux:
      return "RangingResultMux";
    case Register::SfAdditionalConfiguration:
      return "SfAdditionalConfiguration";
    case Register::RangingCalibrationByte2:
      return "RangingCalibrationByte2";
    case Register::RangingCalibrationByte1:
      return "RangingCalibrationByte1";
    case Register::RangingCalibrationByte0:
      return "RangingCalibrationByte0";
    case Register::RangingIdCheckLength:
      return "RangingIdCheckLength";
    case Register::FrequencyErrorCorrection:
      return "FrequencyErrorCorrection";
    case Register::CadDetPeak:
      return "CadDetPeak";
    case Register::LoRaSyncWord1:
      return "LoRaSyncWord1";
    case Register::LoRaSyncWord0:
      return "LoRaSyncWord0";
    case Register::HeaderCrc:
      return "HeaderCrc";
    case Register::CodingRate:
      return "CodingRate";
    case Register::FeiByte1M:
      return "FeiByte1M";
    case Register::FeiByte0L:
      return "FeiByte0L";
    case Register::RangingResultByte2:
      return "RangingResultByte2";
    case Register::RangingResultByte1:
      return "RangingResultByte1";
    case Register::RangingResultByte0:
      return "RangingResultByte0";
    case Register::RangingRssi:
      return "RangingRssi";
    case Register::FreezeRangingResult:
      return "FreezeRangingResult";
    case Register::PacketPreambleSettings:
      return "PacketPreambleSettings";
    case Register::WhiteningInitialValue:
      return "WhiteningInitialValue";
    case Register::CrcPolynomialDefinitionMsb:
      return "CrcPolynomialDefinitionMsb";
    case Register::CrcPolynomialDefinitionLsb:
      return "CrcPolynomialDefinitionLsb";
    case Register::CrcPolynomialSeedByte1:
      return "CrcPolynomialSeedByte1";
    case Register::CrcPolynomialSeedByte0:
      return "CrcPolynomialSeedByte0";
    case Register::SyncAddressControl:
      return "SyncAddressControl";
    case Register::SyncAddress1Byte4:
      return "SyncAddress1Byte4";
    case Register::SyncAddress1Byte3:
      return "SyncAddress1Byte3";
    case Register::SyncAddress1Byte2:
      return "SyncAddress1Byte2";
    case Register::SyncAddress1Byte1:
      return "SyncAddress1Byte1";
    case Register::SyncAddress1Byte0:
      return "SyncAddress1Byte0";
    case Register::SyncAddress2Byte4:
      return "SyncAddress2Byte4";
    case Register::SyncAddress2Byte3:
      return "SyncAddress2Byte3";
    case Register::SyncAddress2Byte2:
      return "SyncAddress2Byte2";
    case Register::SyncAddress2Byte1:
      return "SyncAddress2Byte1";
    case Register::SyncAddress2Byte0:
      return "SyncAddress2Byte0";
    case Register::SyncAddress3Byte4:
      return "SyncAddress3Byte4";
    case Register::SyncAddress3Byte3:
      return "SyncAddress3Byte3";
    case Register::SyncAddress3Byte2:
      return "SyncAddress3Byte2";
    case Register::SyncAddress3Byte1:
      return "SyncAddress3Byte1";
    case Register::SyncAddress3Byte0:
      return "SyncAddress3Byte0";
        break;
    }
    return "<unknown Register>";
}

std::string_view to_string(PacketType packettype) {
  switch (packettype) {
  case PacketType::Gfsk:
    return "Gfsk";
  case PacketType::LoRa:
    return "LoRa";
  case PacketType::Ranging:
    return "Ranging";
  case PacketType::Flrc:
    return "Flrc";
  case PacketType::Ble:
    return "Ble";
  }
  return "<unknown PacketType>";
}
