#ifndef MODBUS_RTU_MASTER_H
#define MODBUS_RTU_MASTER_H

#include "protocols/modbus/modbusMaster.h"

class ModbusRTUMaster : public ModbusMaster {
public:
    ModbusRTUMaster();
    ~ModbusRTUMaster() override;

    // ComBase interface implementations
    bool init(const std::string& config) override;
    bool start() override;
    bool stop() override;
    bool isRunning() const override;
    std::string getStatus() const override;
    std::string getStatistics() const override;

    // RTU-specific methods
    bool connect(const std::string& serialPort, int baudRate, char parity, int dataBits, int stopBits);
    bool disconnect();
    bool isConnected() const { return connected_; }
    
    // RTU configuration
    bool setSerialPort(const std::string& serialPort);
    bool setBaudRate(int baudRate);
    bool setParity(char parity);
    bool setDataBits(int dataBits);
    bool setStopBits(int stopBits);
    
    const std::string& getSerialPort() const { return serialPort_; }
    int getBaudRate() const { return baudRate_; }
    char getParity() const { return parity_; }
    int getDataBits() const { return dataBits_; }
    int getStopBits() const { return stopBits_; }

private:
    std::string serialPort_;  // Serial port device
    int baudRate_;            // Baud rate
    char parity_;             // Parity (N/E/O)
    int dataBits_;            // Data bits (7/8)
    int stopBits_;            // Stop bits (1/2)
    
    // Statistics
    uint64_t bytesReceived_ = 0;
    uint64_t bytesSent_ = 0;
    uint64_t messagesReceived_ = 0;
    uint64_t messagesSent_ = 0;
    uint64_t errorCount_ = 0;
    uint64_t crcErrorCount_ = 0;  // Specific to RTU
};

#endif // MODBUS_RTU_MASTER_H 