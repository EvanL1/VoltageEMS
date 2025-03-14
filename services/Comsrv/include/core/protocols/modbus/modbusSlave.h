#ifndef MODBUS_SLAVE_H
#define MODBUS_SLAVE_H

#include "comBase.h"
#include <string>
#include <vector>
#include <memory>
#include <modbus/modbus.h>
#include <hiredis/hiredis.h>
#include <map>
#include <mutex>
#include <thread>
#include <atomic>

// Address range structure used for optimized reading
struct AddressRange {
    int startAddress;  // Starting address
    int quantity;      // Number of registers/coils to read
    int functionCode;  // Modbus function code to use
    std::vector<std::string> pointIds;  // IDs of points in this range
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

// Modbus exception codes
enum class ModbusExceptionCode {
    ILLEGAL_FUNCTION = 0x01,          // Function code not supported
    ILLEGAL_DATA_ADDRESS = 0x02,      // Address outside of valid range
    ILLEGAL_DATA_VALUE = 0x03,        // Value not within expected range
    SLAVE_DEVICE_FAILURE = 0x04,      // Unrecoverable error while processing
    ACKNOWLEDGE = 0x05,               // Specialized for programming commands
    SLAVE_DEVICE_BUSY = 0x06,         // Slave is busy with long-duration command
    MEMORY_PARITY_ERROR = 0x08,       // Memory parity error detected
    GATEWAY_PATH_UNAVAILABLE = 0x0A,  // Gateway specialization
    GATEWAY_TARGET_FAILURE = 0x0B     // Gateway specialization
};

// Register write callback type
using RegisterWriteCallback = std::function<void(int address, uint16_t value)>;
using CoilWriteCallback = std::function<void(int address, bool value)>;

// Modbus slave device class
class ModbusSlave : public ComBase {
public:
    ModbusSlave();
    virtual ~ModbusSlave();
    
    // ComBase interface implementations
    bool init(const std::string& config) override = 0;
    bool start() override;
    bool stop() override;
    bool isRunning() const override;
    std::string getStatus() const override = 0;
    std::string getStatistics() const override = 0;

    // Protocol identification
    ProtocolType getProtocolType() const override { return ProtocolType::MODBUS; }
    DeviceRole getDeviceRole() const override { return DeviceRole::SLAVE; }
    
    // Physical interface accessor
    PhysicalInterfaceType getPhysicalInterfaceType() const { return physicalInterface_; }
    void setPhysicalInterfaceType(PhysicalInterfaceType type) { physicalInterface_ = type; }

    // Register map access methods
    // Coils (read/write) - Function codes 0x01, 0x05, 0x0F
    bool setCoil(int address, bool value);
    bool getCoil(int address, bool& value) const;
    bool setCoils(int startAddress, const std::vector<bool>& values);
    bool getCoils(int startAddress, int quantity, std::vector<bool>& values) const;
    
    // Discrete Inputs (read-only) - Function code 0x02
    bool setDiscreteInput(int address, bool value);
    bool getDiscreteInput(int address, bool& value) const;
    bool setDiscreteInputs(int startAddress, const std::vector<bool>& values);
    bool getDiscreteInputs(int startAddress, int quantity, std::vector<bool>& values) const;
    
    // Holding Registers (read/write) - Function codes 0x03, 0x06, 0x10
    bool setHoldingRegister(int address, uint16_t value);
    bool getHoldingRegister(int address, uint16_t& value) const;
    bool setHoldingRegisters(int startAddress, const std::vector<uint16_t>& values);
    bool getHoldingRegisters(int startAddress, int quantity, std::vector<uint16_t>& values) const;
    
    // Input Registers (read-only) - Function code 0x04
    bool setInputRegister(int address, uint16_t value);
    bool getInputRegister(int address, uint16_t& value) const;
    bool setInputRegisters(int startAddress, const std::vector<uint16_t>& values);
    bool getInputRegisters(int startAddress, int quantity, std::vector<uint16_t>& values) const;
    
    // Register callback registration for notifications on register changes
    void setHoldingRegisterCallback(RegisterWriteCallback callback);
    void setCoilCallback(CoilWriteCallback callback);
    
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

    // Gets the current slave ID
    int getSlaveId() const;
    
    // Error handling
    // Gets the last error code
    int getLastErrorCode() const;

    // Gets the last error message
    std::string getLastError() const;
    
    // Starts the Modbus mapping for registers
    bool setupModbusMapping(int nbCoils, int nbDiscreteInputs, int nbHoldingRegisters, int nbInputRegisters);

protected:
    // Start a thread for handling Modbus requests
    bool startListening();
    
    // Stop the listening thread
    bool stopListening();
    
    // Listening thread function
    virtual void listenThreadFunc() = 0;
    
    // Override base class's channelThreadFunc for Modbus-specific behavior
    void channelThreadFunc(int channelIndex) override;
    
    // Process specific Modbus function codes
    void processReadCoils(uint8_t* request, int offset, int* requestLength);
    void processReadDiscreteInputs(uint8_t* request, int offset, int* requestLength);
    void processReadHoldingRegisters(uint8_t* request, int offset, int* requestLength);
    void processReadInputRegisters(uint8_t* request, int offset, int* requestLength);
    void processWriteSingleCoil(uint8_t* request, int offset, int* requestLength);
    void processWriteSingleRegister(uint8_t* request, int offset, int* requestLength);
    void processWriteMultipleCoils(uint8_t* request, int offset, int* requestLength);
    void processWriteMultipleRegisters(uint8_t* request, int offset, int* requestLength);
    
    // Build Modbus exception response
    void buildExceptionResponse(uint8_t functionCode, ModbusExceptionCode exceptionCode, uint8_t* response, int* responseLength);

    modbus_t* ctx_;                  // Modbus context
    modbus_mapping_t* mapping_;      // Modbus data mapping
    int slaveId_;                    // Slave ID
    int timeout_;                    // Timeout duration
    bool debug_;                     // Debug mode status
    bool connected_;                 // Connection status
    std::string lastError_;          // Last error message
    int lastErrorCode_ = 0;          // Last error code
    PhysicalInterfaceType physicalInterface_ = PhysicalInterfaceType::NETWORK;  // Default to network

    // Threading
    std::thread listeningThread_;    // Thread for listening to incoming requests
    std::atomic<bool> running_;      // Flag to indicate if the listening thread is running
    
    // Register access synchronization
    mutable std::mutex registerMutex_;
    
    // Custom register storage for advanced handling beyond the basic modbus mapping
    std::map<int, bool> coils_;
    std::map<int, bool> discreteInputs_;
    std::map<int, uint16_t> holdingRegisters_;
    std::map<int, uint16_t> inputRegisters_;
    
    // Register change callbacks
    RegisterWriteCallback holdingRegisterCallback_;
    CoilWriteCallback coilCallback_;
    
    // Statistics
    uint64_t requestsReceived_ = 0;
    uint64_t responsesRejected_ = 0;
    uint64_t responsesWritten_ = 0;
    uint64_t exceptionsSent_ = 0;

    // Log callback related
    using LogCallback = std::function<void(const std::string& message)>;
    LogCallback logCallback_;
    static std::string formatMessage(const uint8_t* data, int len);
    static void modbusPreSendCallback(modbus_t* ctx, uint8_t* req, int req_len);
    static void modbusPostRecvCallback(modbus_t* ctx, uint8_t* rsp, int rsp_len);
    static ModbusSlave* instance;      // Instance pointer for callbacks

    int maxRead_ = 120;  // Default maximum read quantity
};

// Factory function for creating Modbus slaves based on physical interface type
std::unique_ptr<ModbusSlave> createModbusSlave(PhysicalInterfaceType interfaceType);

#endif // MODBUS_SLAVE_H