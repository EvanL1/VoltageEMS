#ifndef MODBUS_MASTER_H
#define MODBUS_MASTER_H

#include "comBase.h"
#include <string>
#include <vector>
#include <memory>
#include <modbus/modbus.h>
#include <hiredis/hiredis.h>

// Communication mode enumeration
enum class ComMode {
    TCP,
    RTU
};

// Modbus function codes enumeration
enum class ModbusFunctionCode {
    READ_COILS = 0x01,                // Read Coils
    READ_DISCRETE_INPUTS = 0x02,      // Read Discrete Inputs
    READ_HOLDING_REGISTERS = 0x03,    // Read Holding Registers
    READ_INPUT_REGISTERS = 0x04,      // Read Input Registers
    WRITE_SINGLE_COIL = 0x05,         // Write Single Coil
    WRITE_SINGLE_REGISTER = 0x06,     // Write Single Register
    WRITE_MULTIPLE_COILS = 0x0F,      // Write Multiple Coils
    WRITE_MULTIPLE_REGISTERS = 0x10,  // Write Multiple Registers
};

// Address range structure used for optimized reading
struct AddressRange {
    int startAddress;  // Starting address
    int quantity;      // Number of registers/coils to read
    int functionCode;  // Modbus function code to use
    std::vector<std::string> pointIds;  // IDs of points in this range
};

// Modbus master class
class ModbusMaster : public ComBase {
public:
    ModbusMaster();
    virtual ~ModbusMaster();
    
    // ComBase interface implementations
    bool init(const std::string& config) override = 0;
    std::string getStatus() const override = 0;
    std::string getStatistics() const override = 0;

    // Protocol identification
    ProtocolType getProtocolType() const override { return ProtocolType::MODBUS; }
    DeviceRole getDeviceRole() const override { return DeviceRole::MASTER; }
    
    // Physical interface accessor
    PhysicalInterfaceType getPhysicalInterfaceType() const { return physicalInterface_; }
    void setPhysicalInterfaceType(PhysicalInterfaceType type) { physicalInterface_ = type; }

    // Common Modbus functions
    // Reads coils from the specified slave device -> 0x01
    bool readCoils(int slaveId, int address, int quantity, std::vector<bool>& values);

    // Reads discrete inputs from the specified slave device -> 0x02
    bool readDiscreteInputs(int slaveId, int address, int quantity, std::vector<bool>& values);

    // Reads holding registers from the specified slave device -> 0x03
    bool readHoldingRegisters(int slaveId, int address, int quantity, std::vector<uint16_t>& values);

    // Reads input registers from the specified slave device -> 0x04
    bool readInputRegisters(int slaveId, int address, int quantity, std::vector<uint16_t>& values);
    
    // Writes a single coil to the specified slave device -> 0x05
    bool writeSingleCoil(int slaveId, int address, bool value);

    // Writes a single register to the specified slave device -> 0x06
    bool writeSingleRegister(int slaveId, int address, uint16_t value);

    // Writes multiple coils to the specified slave device -> 0x0F
    bool writeMultipleCoils(int slaveId, int address, const std::vector<bool>& values);

    // Writes multiple registers to the specified slave device -> 0x10
    bool writeMultipleRegisters(int slaveId, int address, const std::vector<uint16_t>& values);

    // Modbus specific settings
    // Sets the slave ID for communication
    bool setSlaveId(int id);

    // Sets the timeout for communication
    bool setTimeout(int ms);

    // Enables or disables debug mode
    bool setDebug(bool enable);

    // Configuration interface
    // Sets the response timeout for communication
    void setResponseTimeout(uint32_t sec, uint32_t usec);

    // Sets whether to enable broadcast mode
    void setBroadcast(bool broadcast);

    // Gets the current slave ID
    int getSlaveId() const;
    
    // Error handling
    // Gets the last error code
    int getLastErrorCode() const;

    // Gets the last error message
    std::string getLastError() const;

protected:
    // Override base class's channelThreadFunc for Modbus-specific behavior
    void channelThreadFunc(int channelIndex) override;

    modbus_t* ctx_;                    // Modbus context
    int slaveId_;                      // Slave ID
    int timeout_;                      // Timeout duration
    bool debug_;                       // Debug mode status
    bool connected_;                   // Connection status
    std::string lastError_;            // Last error message
    int lastErrorCode_ = 0;            // Last error code
    // Log callback related
    using LogCallback = std::function<void(const std::string& message)>;
    LogCallback logCallback_;
    static std::string formatMessage(const uint8_t* data, int len);
    static void modbusPreSendCallback(modbus_t* ctx, uint8_t* req, int req_len);
    static void modbusPostRecvCallback(modbus_t* ctx, uint8_t* rsp, int rsp_len);
    static ModbusMaster* instance;      // Instance pointer for callbacks

    int maxRead_ = 120;  // Default maximum read quantity

    // Functions related to segmented polling
    std::vector<AddressRange> analyzeAddressRanges(
        const std::map<std::string, DataPointConfig>& points, int maxRead);
    
    // Gets the size of the data point based on its type
    int getPointSize(DataType type);

    // Reads all points
    void readAllPoints();

    // Reads points by type
    void readPointsByType(const std::map<std::string, DataPointConfig>& points, 
                         PointType type);

    // Processes the data for a given address range
    void processRangeData(const AddressRange& range, 
                         const std::vector<uint16_t>& values,
                         const std::map<std::string, DataPointConfig>& points);
};

// Factory function for creating Modbus masters based on physical interface type
std::unique_ptr<ModbusMaster> createModbusMaster(PhysicalInterfaceType interfaceType);

#endif // MODBUS_MASTER_H 