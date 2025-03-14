#ifndef MODBUS_MASTER_H
#define MODBUS_MASTER_H

#include "core/comBase.h"
#include <string>
#include <vector>
#include <map>
#include <mutex>
#include <atomic>
#include <thread>
#include <functional>

/**
 * @brief Modbus function codes
 */
enum class ModbusFunction {
    READ_COILS = 0x01,
    READ_DISCRETE_INPUTS = 0x02,
    READ_HOLDING_REGISTERS = 0x03,
    READ_INPUT_REGISTERS = 0x04,
    WRITE_SINGLE_COIL = 0x05,
    WRITE_SINGLE_REGISTER = 0x06,
    WRITE_MULTIPLE_COILS = 0x0F,
    WRITE_MULTIPLE_REGISTERS = 0x10
};

/**
 * @brief Modbus data types
 */
enum class ModbusDataType {
    COIL,
    DISCRETE_INPUT,
    HOLDING_REGISTER,
    INPUT_REGISTER
};

/**
 * @brief Modbus register data types
 */
enum class ModbusRegisterType {
    UINT16,
    INT16,
    UINT32,
    INT32,
    FLOAT32,
    FLOAT64
};

/**
 * @brief Modbus endianness
 */
enum class ModbusEndian {
    BIG_ENDIAN,
    LITTLE_ENDIAN,
    BIG_ENDIAN_BYTE_SWAP,
    LITTLE_ENDIAN_BYTE_SWAP
};

/**
 * @brief Base class for Modbus master devices
 */
class ModbusMaster : public ComBase {
public:
    /**
     * @brief Constructor
     * 
     * @param name Name of the Modbus master
     */
    ModbusMaster(const std::string& name);
    
    /**
     * @brief Destructor
     */
    virtual ~ModbusMaster();
    
    /**
     * @brief Read coils from a slave device
     * 
     * @param slaveId Slave ID
     * @param startAddress Start address
     * @param count Number of coils to read
     * @param[out] values Vector to store the read values
     * @return true if successful, false otherwise
     */
    virtual bool readCoils(uint8_t slaveId, uint16_t startAddress, uint16_t count, std::vector<bool>& values) = 0;
    
    /**
     * @brief Read discrete inputs from a slave device
     * 
     * @param slaveId Slave ID
     * @param startAddress Start address
     * @param count Number of discrete inputs to read
     * @param[out] values Vector to store the read values
     * @return true if successful, false otherwise
     */
    virtual bool readDiscreteInputs(uint8_t slaveId, uint16_t startAddress, uint16_t count, std::vector<bool>& values) = 0;
    
    /**
     * @brief Read holding registers from a slave device
     * 
     * @param slaveId Slave ID
     * @param startAddress Start address
     * @param count Number of registers to read
     * @param[out] values Vector to store the read values
     * @return true if successful, false otherwise
     */
    virtual bool readHoldingRegisters(uint8_t slaveId, uint16_t startAddress, uint16_t count, std::vector<uint16_t>& values) = 0;
    
    /**
     * @brief Read input registers from a slave device
     * 
     * @param slaveId Slave ID
     * @param startAddress Start address
     * @param count Number of registers to read
     * @param[out] values Vector to store the read values
     * @return true if successful, false otherwise
     */
    virtual bool readInputRegisters(uint8_t slaveId, uint16_t startAddress, uint16_t count, std::vector<uint16_t>& values) = 0;
    
    /**
     * @brief Write single coil to a slave device
     * 
     * @param slaveId Slave ID
     * @param address Coil address
     * @param value Value to write
     * @return true if successful, false otherwise
     */
    virtual bool writeSingleCoil(uint8_t slaveId, uint16_t address, bool value) = 0;
    
    /**
     * @brief Write single register to a slave device
     * 
     * @param slaveId Slave ID
     * @param address Register address
     * @param value Value to write
     * @return true if successful, false otherwise
     */
    virtual bool writeSingleRegister(uint8_t slaveId, uint16_t address, uint16_t value) = 0;
    
    /**
     * @brief Write multiple coils to a slave device
     * 
     * @param slaveId Slave ID
     * @param startAddress Start address
     * @param values Values to write
     * @return true if successful, false otherwise
     */
    virtual bool writeMultipleCoils(uint8_t slaveId, uint16_t startAddress, const std::vector<bool>& values) = 0;
    
    /**
     * @brief Write multiple registers to a slave device
     * 
     * @param slaveId Slave ID
     * @param startAddress Start address
     * @param values Values to write
     * @return true if successful, false otherwise
     */
    virtual bool writeMultipleRegisters(uint8_t slaveId, uint16_t startAddress, const std::vector<uint16_t>& values) = 0;
    
    /**
     * @brief Read a value with a specific data type
     * 
     * @param slaveId Slave ID
     * @param address Register address
     * @param type Register type
     * @param registerType Register data type
     * @param endian Endianness
     * @param[out] value Value to store the read value
     * @return true if successful, false otherwise
     */
    template<typename T>
    bool readValue(uint8_t slaveId, uint16_t address, ModbusDataType type, ModbusRegisterType registerType, 
                  ModbusEndian endian, T& value);
    
    /**
     * @brief Write a value with a specific data type
     * 
     * @param slaveId Slave ID
     * @param address Register address
     * @param type Register type
     * @param registerType Register data type
     * @param endian Endianness
     * @param value Value to write
     * @return true if successful, false otherwise
     */
    template<typename T>
    bool writeValue(uint8_t slaveId, uint16_t address, ModbusDataType type, ModbusRegisterType registerType, 
                   ModbusEndian endian, T value);
    
    /**
     * @brief Set a polling function for a specific register
     * 
     * @param slaveId Slave ID
     * @param address Register address
     * @param type Register type
     * @param interval Polling interval in milliseconds
     * @param callback Callback function when value changes
     * @return Polling ID for removing the polling
     */
    int addPolling(uint8_t slaveId, uint16_t address, ModbusDataType type, uint16_t count, 
                  ModbusRegisterType registerType, ModbusEndian endian, 
                  uint32_t interval, std::function<void(const std::vector<uint16_t>&)> callback);
    
    /**
     * @brief Remove a polling
     * 
     * @param pollingId Polling ID returned by addPolling
     */
    void removePolling(int pollingId);
    
protected:
    /**
     * @brief Convert raw register values to a specific data type
     * 
     * @param registers Raw register values
     * @param registerType Register data type
     * @param endian Endianness
     * @param[out] value Value to store the converted value
     * @return true if successful, false otherwise
     */
    template<typename T>
    bool convertRegisters(const std::vector<uint16_t>& registers, ModbusRegisterType registerType, 
                         ModbusEndian endian, T& value);
    
    /**
     * @brief Convert a specific data type to raw register values
     * 
     * @param value Value to convert
     * @param registerType Register data type
     * @param endian Endianness
     * @param[out] registers Vector to store the converted register values
     * @return true if successful, false otherwise
     */
    template<typename T>
    bool convertToRegisters(T value, ModbusRegisterType registerType, 
                           ModbusEndian endian, std::vector<uint16_t>& registers);
    
    /**
     * @brief Polling thread function
     */
    void pollingThread();
    
    /**
     * @brief Struct for polling information
     */
    struct PollingInfo {
        uint8_t slaveId;
        uint16_t address;
        uint16_t count;
        ModbusDataType type;
        ModbusRegisterType registerType;
        ModbusEndian endian;
        uint32_t interval;
        std::function<void(const std::vector<uint16_t>&)> callback;
        std::chrono::steady_clock::time_point lastPoll;
    };
    
    std::map<int, PollingInfo> m_pollingInfo;
    int m_nextPollingId;
    std::thread m_pollingThread;
    std::atomic<bool> m_pollingRunning;
    std::mutex m_pollingMutex;
};

// Template implementations will be in a separate header file

#endif // MODBUS_MASTER_H 