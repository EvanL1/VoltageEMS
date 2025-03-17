#include "core/protocols/modbus/modbusMaster.h"
#include <iostream>
#include <chrono>

ModbusMaster::ModbusMaster(const std::string& name)
    : ComBase(name), m_nextPollingId(0), m_pollingRunning(false)
{
    std::cout << "ModbusMaster initialized: " << name << std::endl;
}

ModbusMaster::~ModbusMaster()
{
    if (m_pollingRunning) {
        m_pollingRunning = false;
        if (m_pollingThread.joinable()) {
            m_pollingThread.join();
        }
    }
    std::cout << "ModbusMaster destroyed: " << getName() << std::endl;
}

int ModbusMaster::addPolling(uint8_t slaveId, uint16_t address, ModbusDataType type, uint16_t count, 
                           ModbusRegisterType registerType, ModbusEndian endian, 
                           uint32_t interval, std::function<void(const std::vector<uint16_t>&)> callback)
{
    std::lock_guard<std::mutex> lock(m_pollingMutex);
    
    int pollingId = m_nextPollingId++;
    
    PollingInfo info = {
        slaveId,
        address,
        count,
        type,
        registerType,
        endian,
        interval,
        callback,
        std::chrono::steady_clock::now()
    };
    
    m_pollingInfo[pollingId] = info;
    
    // Start polling thread if not running
    if (!m_pollingRunning) {
        m_pollingRunning = true;
        m_pollingThread = std::thread(&ModbusMaster::pollingThread, this);
    }
    
    return pollingId;
}

void ModbusMaster::removePolling(int pollingId)
{
    std::lock_guard<std::mutex> lock(m_pollingMutex);
    
    auto it = m_pollingInfo.find(pollingId);
    if (it != m_pollingInfo.end()) {
        m_pollingInfo.erase(it);
    }
    
    // Stop polling thread if no more polling
    if (m_pollingInfo.empty() && m_pollingRunning) {
        m_pollingRunning = false;
        if (m_pollingThread.joinable()) {
            m_pollingThread.join();
        }
    }
}

void ModbusMaster::pollingThread()
{
    std::cout << "Polling thread started for: " << getName() << std::endl;
    
    while (m_pollingRunning) {
        auto now = std::chrono::steady_clock::now();
        
        {
            std::lock_guard<std::mutex> lock(m_pollingMutex);
            
            for (auto& [id, info] : m_pollingInfo) {
                auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(now - info.lastPoll).count();
                
                if (elapsed >= info.interval) {
                    // Time to poll
                    std::vector<uint16_t> values;
                    bool success = false;
                    
                    switch (info.type) {
                        case ModbusDataType::COIL: {
                            std::vector<bool> boolValues;
                            success = readCoils(info.slaveId, info.address, info.count, boolValues);
                            if (success) {
                                values.resize(boolValues.size());
                                for (size_t i = 0; i < boolValues.size(); ++i) {
                                    values[i] = boolValues[i] ? 1 : 0;
                                }
                            }
                            break;
                        }
                        case ModbusDataType::DISCRETE_INPUT: {
                            std::vector<bool> boolValues;
                            success = readDiscreteInputs(info.slaveId, info.address, info.count, boolValues);
                            if (success) {
                                values.resize(boolValues.size());
                                for (size_t i = 0; i < boolValues.size(); ++i) {
                                    values[i] = boolValues[i] ? 1 : 0;
                                }
                            }
                            break;
                        }
                        case ModbusDataType::HOLDING_REGISTER:
                            success = readHoldingRegisters(info.slaveId, info.address, info.count, values);
                            break;
                        case ModbusDataType::INPUT_REGISTER:
                            success = readInputRegisters(info.slaveId, info.address, info.count, values);
                            break;
                    }
                    
                    if (success) {
                        // Call callback
                        info.callback(values);
                    }
                    
                    // Update last poll time
                    info.lastPoll = now;
                }
            }
        }
        
        // Sleep for a short time to avoid busy waiting
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    
    std::cout << "Polling thread stopped for: " << getName() << std::endl;
} 