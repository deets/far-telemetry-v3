// -*- mode: c++-mode
#pragma once
#include <cstddef>
#include <cstdint>
#include <stdint.h>
#include <vector>
#include <format>
#include <iostream>
#include <sstream>
#include <string_view>

enum SPIBitOrder { MSBFIRST };
enum SPIMode { SPI_MODE0 };

enum class OpCode {
  NoOp = 0,
    // Operational Modes Functions
    /// Returns current status of the transceiver
    GetStatus = 0xC0,
    /// Sets transceiver to Sleep mode
    SetSleep = 0x84,
    /// Sets transceiver to Standby mode
    SetStandby = 0x80,
    /// Sets transceiver to Frequency Synthesis (FS) mode
    SetFs = 0xC1,
    /// Sets transceiver to Transmit (Tx) mode
    SetTx = 0x83,
    /// Sets transceiver to Receive (Rx) mode
    SetRx = 0x82,
    /// Sets transceiver to Rx Duty Cycle mode
    SetRxDutyCycle = 0x94,
    /// Sets transceiver to Channel Activity Detection (CAD) mode
    SetCad = 0xC5,
    /// Sets transceiver to Continuous Wave transmission mode
    SetTxContinuousWave = 0xD1,
    /// Sets transceiver to Continuous Preamble transmission mode
    SetTxContinuousPreamble = 0xD2,
    /// Sets auto Frequency Synthesis mode
    SetAutoFS = 0x9E,
    /// Sets auto Transmit mode with specified time
    SetAutoTx = 0x98,

    // Register and Buffer Access Functions
    /// Writes data into radio register memory
    WriteRegister = 0x18,
    /// Reads data from radio register memory
    ReadRegister = 0x19,
    /// Writes data into radio data buffer memory
    WriteBuffer = 0x1A,
    /// Reads data from radio data buffer memory
    ReadBuffer = 0x1B,

    // DIO and Interrupt Control Functions
    /// Configures DIO interrupt masks
    SetDioIrqParams = 0x8D,
    /// Returns current interrupt status
    GetIrqStatus = 0x15,
    /// Clears selected interrupt flags
    ClrIrqStatus = 0x97,

    // RF Modulation and Packet Functions
    /// Sets modem packet type (GFSK, LoRa, Ranging, FLRC, BLE)
    SetPacketType = 0x8A,
    /// Returns current modem packet type
    GetPacketType = 0x03,
    /// Sets RF frequency
    SetRfFrequency = 0x86,
    /// Sets transmit power and ramp time
    SetTxParams = 0x8E,
    /// Sets CAD symbol number
    SetCadParams = 0x88,
    /// Sets base addresses for Tx and Rx data buffer
    SetBufferBaseAddress = 0x8F,
    /// Sets modulation parameters
    SetModulationParams = 0x8B,
    /// Sets packet parameters
    SetPacketParams = 0x8C,
    /// Returns payload length and Rx buffer offset
    GetRxBufferStatus = 0x17,
    /// Returns packet status information
    GetPacketStatus = 0x1D,
    /// Returns instantaneous RSSI
    GetRssiInst = 0x1F,
    /// Sets long preamble mode
    SetLongPreamble = 0x9B,

    // Miscellaneous and Advanced Functions
    /// Sets power regulation mode (LDO or DC-DC)
    SetRegulatorMode = 0x96,
    /// Saves context in Sleep mode
    SetSaveContext = 0xD5,
    /// Sets UART interface speed
    SetUartSpeed = 0x9D,
    /// Sets ranging role (Slave or Master)
    SetRangingRole = 0xA3,
    /// Enables or disables advanced ranging
    SetAdvancedRanging = 0x9A,
};

struct Transaction {
  OpCode opcode;
  bool open = true;
  size_t count = 0;
  std::vector<uint8_t> outgoing;

  uint8_t transact(uint8_t outgoing) {
    if(open) {
      this->outgoing.push_back(outgoing);
      if (count == 0) {
        // At least for SetStandby the result is 0x43,
        // I'll use that for all now
        return 0x43;
      } else {
        return 0;
      }
    } else {
      throw std::runtime_error("Transaction closed");
    }
  }

  void end() { open = false; };

};

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

private:
  void transferAfterTransaction(uint8_t);
  void unknownOpCode(uint8_t);

  bool _transfer_after_transaction = false;

  Transaction &current();
  void addTransaction(OpCode, bool is_busy);
  std::vector<Transaction> _transactions;
};


extern SPIClass SPI;

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
