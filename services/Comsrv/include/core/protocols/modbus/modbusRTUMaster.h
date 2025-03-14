#ifndef MODBUS_RTU_MASTER_H
#define MODBUS_RTU_MASTER_H

#include "core/protocols/modbus/modbusMaster.h"
#include <string>
#include <vector>
#include <mutex>
#include <atomic>
#include <thread>
#include <queue>
#include <condition_variable>

/**
 * @brief Serial port parity
 */
enum class SerialParity {
    NONE,
    ODD,
    EVEN
};

/**
 * @brief Modbus RTU Master class
 * 
 * This class implements the Modbus RTU protocol over serial port.
 */
class ModbusRTUMaster : public ModbusMaster {
public:
    /**
     * @brief Constructor
     * 
     * @param portName Serial port name
     * @param baudRate Baud rate
     * @param dataBits Data bits (5-8)
     * @param parity Parity
     * @param stopBits Stop bits (1 or 2)
     * @param timeout Timeout in milliseconds
     */
    ModbusRTUMaster(const std::string& portName, int baudRate = 9600, int dataBits = 8, 
                   SerialParity parity = SerialParity::NONE, int stopBits = 1, int timeout = 1000);
    
    /**
     * @brief Destructor
     */
    virtual ~ModbusRTUMaster();
    
    /**
     * @brief Start the communication
     * 
     * @return true if started successfully, false otherwise
     */
    bool start() override;
    
    /**
     * @brief Stop the communication
     * 
     * @return true if stopped successfully, false otherwise
     */
    bool stop() override;
    
    /**
     * @brief Read coils from a slave device
     * 
     * @param slaveId Slave ID
     * @param startAddress Start address
     * @param count Number of coils to read
     * @param[out] values Vector to store the read values
     * @return true if successful, false otherwise
     */
    bool readCoils(uint8_t slaveId, uint16_t startAddress, uint16_t count, std::vector<bool>& values) override;
    
    /**
     * @brief Read discrete inputs from a slave device
     * 
     * @param slaveId Slave ID
     * @param startAddress Start address
     * @param count Number of discrete inputs to read
     * @param[out] values Vector to store the read values
     * @return true if successful, false otherwise
     */
    bool readDiscreteInputs(uint8_t slaveId, uint16_t startAddress, uint16_t count, std::vector<bool>& values) override;
    
    /**
     * @brief Read holding registers from a slave device
     * 
     * @param slaveId Slave ID
     * @param startAddress Start address
     * @param count Number of registers to read
     * @param[out] values Vector to store the read values
     * @return true if successful, false otherwise
     */
    bool readHoldingRegisters(uint8_t slaveId, uint16_t startAddress, uint16_t count, std::vector<uint16_t>& values) override;
    
    /**
     * @brief Read input registers from a slave device
     * 
     * @param slaveId Slave ID
     * @param startAddress Start address
     * @param count Number of registers to read
     * @param[out] values Vector to store the read values
     * @return true if successful, false otherwise
     */
    bool readInputRegisters(uint8_t slaveId, uint16_t startAddress, uint16_t count, std::vector<uint16_t>& values) override;
    
    /**
     * @brief Write single coil to a slave device
     * 
     * @param slaveId Slave ID
     * @param address Coil address
     * @param value Value to write
     * @return true if successful, false otherwise
     */
    bool writeSingleCoil(uint8_t slaveId, uint16_t address, bool value) override;
    
    /**
     * @brief Write single register to a slave device
     * 
     * @param slaveId Slave ID
     * @param address Register address
     * @param value Value to write
     * @return true if successful, false otherwise
     */
    bool writeSingleRegister(uint8_t slaveId, uint16_t address, uint16_t value) override;
    
    /**
     * @brief Write multiple coils to a slave device
     * 
     * @param slaveId Slave ID
     * @param startAddress Start address
     * @param values Values to write
     * @return true if successful, false otherwise
     */
    bool writeMultipleCoils(uint8_t slaveId, uint16_t startAddress, const std::vector<bool>& values) override;
    
    /**
     * @brief Write multiple registers to a slave device
     * 
     * @param slaveId Slave ID
     * @param startAddress Start address
     * @param values Values to write
     * @return true if successful, false otherwise
     */
    bool writeMultipleRegisters(uint8_t slaveId, uint16_t startAddress, const std::vector<uint16_t>& values) override;
    
private:
    /**
     * @brief Modbus request
     */
    struct ModbusRequest {
        uint8_t slaveId;
        ModbusFunction function;
        std::vector<uint8_t> data;
        std::vector<uint8_t> response;
        bool completed;
        bool success;
    };
    
    /**
     * @brief Send a Modbus request
     * 
     * @param request Modbus request
     * @return true if successful, false otherwise
     */
    bool sendRequest(ModbusRequest& request);
    
    /**
     * @brief Calculate CRC
     * 
     * @param data Data to calculate CRC for
     * @param length Length of data
     * @return Calculated CRC
     */
    uint16_t calculateCRC(const uint8_t* data, size_t length);
    
    /**
     * @brief Serial communication thread
     */
    void serialThread();
    
    std::string m_portName;
    int m_baudRate;
    int m_dataBits;
    SerialParity m_parity;
    int m_stopBits;
    int m_timeout;
    
    int m_serialPort;
    std::atomic<bool> m_serialRunning;
    std::thread m_serialThread;
    
    std::mutex m_requestMutex;
    std::queue<ModbusRequest> m_requestQueue;
    std::condition_variable m_requestCondition;
    
    std::mutex m_responseMutex;
    ModbusRequest* m_currentRequest;
    std::condition_variable m_responseCondition;
};

#endif // MODBUS_RTU_MASTER_H 