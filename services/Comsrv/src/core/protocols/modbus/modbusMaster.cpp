#include "protocols/modbus/modbusMaster.h"
#include <iostream>
#include <chrono>
#include <iomanip>
#include <algorithm>
#include <thread>

// Static instance pointer for callbacks
ModbusMaster* ModbusMaster::instance = nullptr;

ModbusMaster::ModbusMaster() 
    : ctx_(nullptr), slaveId_(1), timeout_(1000), debug_(false), connected_(false) {
    // Set instance for callback functions
    instance = this;
}

ModbusMaster::~ModbusMaster() {
    // Free Modbus context if allocated
    if (ctx_) {
        modbus_free(ctx_);
        ctx_ = nullptr;
    }
    
    // Clear instance pointer
    if (instance == this) {
        instance = nullptr;
    }
}

bool ModbusMaster::setSlaveId(int id) {
    if (id < 0 || id > 247) {
        lastError_ = "Invalid slave ID (valid range: 0-247)";
        logger_.logError("modbus", "Invalid slave ID", 
                       "{\"id\":" + std::to_string(id) + "}");
        return false;
    }
    
    slaveId_ = id;
    
    if (ctx_) {
        if (modbus_set_slave(ctx_, id) == -1) {
            lastError_ = modbus_strerror(errno);
            logger_.logError("modbus", "Failed to set slave ID", 
                           "{\"id\":" + std::to_string(id) + 
                           ",\"error\":\"" + lastError_ + "\"}");
            return false;
        }
    }
    
    return true;
}

bool ModbusMaster::setTimeout(int ms) {
    if (ms <= 0) {
        lastError_ = "Invalid timeout value";
        logger_.logError("modbus", "Invalid timeout", 
                       "{\"timeout\":" + std::to_string(ms) + "}");
        return false;
    }
    
    timeout_ = ms;
    
    if (ctx_) {
        struct timeval tv;
        tv.tv_sec = ms / 1000;
        tv.tv_usec = (ms % 1000) * 1000;
        
        if (modbus_set_response_timeout(ctx_, tv.tv_sec, tv.tv_usec) == -1) {
            lastError_ = modbus_strerror(errno);
            logger_.logError("modbus", "Failed to set timeout", 
                           "{\"timeout\":" + std::to_string(ms) + 
                           ",\"error\":\"" + lastError_ + "\"}");
            return false;
        }
    }
    
    return true;
}

bool ModbusMaster::setDebug(bool enable) {
    debug_ = enable;
    
    if (ctx_) {
        modbus_set_debug(ctx_, enable ? TRUE : FALSE);
    }
    
    return true;
}

void ModbusMaster::setResponseTimeout(uint32_t sec, uint32_t usec) {
    if (ctx_) {
        modbus_set_response_timeout(ctx_, sec, usec);
    }
}

void ModbusMaster::setBroadcast(bool broadcast) {
    if (!broadcast) {
        setSlaveId(slaveId_);
    } else {
        setSlaveId(0);  // Broadcast address is 0
    }
}

int ModbusMaster::getSlaveId() const {
    return slaveId_;
}

int ModbusMaster::getLastErrorCode() const {
    return lastErrorCode_;
}

std::string ModbusMaster::getLastError() const {
    return lastError_;
}

// Implementation of Modbus protocol functions

bool ModbusMaster::readCoils(int slaveId, int address, int quantity, std::vector<bool>& values) {
    if (!ctx_ || !connected_) {
        lastError_ = "Not connected";
        logger_.logError("modbus", "Not connected for read_coils", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + "}");
        return false;
    }
    
    // Set slave ID for this transaction
    modbus_set_slave(ctx_, slaveId);
    
    // Prepare buffer for response
    uint8_t* buffer = new uint8_t[quantity];
    memset(buffer, 0, quantity);
    
    // Execute Modbus read coils function
    int result = modbus_read_bits(ctx_, address, quantity, buffer);
    
    // Check for errors
    if (result == -1) {
        lastError_ = modbus_strerror(errno);
        lastErrorCode_ = errno;
        logger_.logError("modbus", "Read coils failed", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + 
                       ",\"quantity\":" + std::to_string(quantity) + 
                       ",\"error\":\"" + lastError_ + "\"}");
        delete[] buffer;
        return false;
    }
    
    // Convert response to vector of booleans
    values.clear();
    values.reserve(quantity);
    for (int i = 0; i < quantity; i++) {
        values.push_back(buffer[i] != 0);
    }
    
    // Clean up
    delete[] buffer;
    
    // Log success
    logger_.logDebug("modbus", "Read coils succeeded", 
                    "{\"slave\":" + std::to_string(slaveId) + 
                    ",\"address\":" + std::to_string(address) + 
                    ",\"quantity\":" + std::to_string(quantity) + "}");
    
    return true;
}

bool ModbusMaster::readDiscreteInputs(int slaveId, int address, int quantity, std::vector<bool>& values) {
    if (!ctx_ || !connected_) {
        lastError_ = "Not connected";
        logger_.logError("modbus", "Not connected for read_discrete_inputs", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + "}");
        return false;
    }
    
    // Set slave ID for this transaction
    modbus_set_slave(ctx_, slaveId);
    
    // Prepare buffer for response
    uint8_t* buffer = new uint8_t[quantity];
    memset(buffer, 0, quantity);
    
    // Execute Modbus read discrete inputs function
    int result = modbus_read_input_bits(ctx_, address, quantity, buffer);
    
    // Check for errors
    if (result == -1) {
        lastError_ = modbus_strerror(errno);
        lastErrorCode_ = errno;
        logger_.logError("modbus", "Read discrete inputs failed", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + 
                       ",\"quantity\":" + std::to_string(quantity) + 
                       ",\"error\":\"" + lastError_ + "\"}");
        delete[] buffer;
        return false;
    }
    
    // Convert response to vector of booleans
    values.clear();
    values.reserve(quantity);
    for (int i = 0; i < quantity; i++) {
        values.push_back(buffer[i] != 0);
    }
    
    // Clean up
    delete[] buffer;
    
    // Log success
    logger_.logDebug("modbus", "Read discrete inputs succeeded", 
                    "{\"slave\":" + std::to_string(slaveId) + 
                    ",\"address\":" + std::to_string(address) + 
                    ",\"quantity\":" + std::to_string(quantity) + "}");
    
    return true;
}

bool ModbusMaster::readHoldingRegisters(int slaveId, int address, int quantity, std::vector<uint16_t>& values) {
    if (!ctx_ || !connected_) {
        lastError_ = "Not connected";
        logger_.logError("modbus", "Not connected for read_holding_registers", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + "}");
        return false;
    }
    
    // Set slave ID for this transaction
    modbus_set_slave(ctx_, slaveId);
    
    // Prepare buffer for response
    uint16_t* buffer = new uint16_t[quantity];
    memset(buffer, 0, quantity * sizeof(uint16_t));
    
    // Execute Modbus read holding registers function
    int result = modbus_read_registers(ctx_, address, quantity, buffer);
    
    // Check for errors
    if (result == -1) {
        lastError_ = modbus_strerror(errno);
        lastErrorCode_ = errno;
        logger_.logError("modbus", "Read holding registers failed", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + 
                       ",\"quantity\":" + std::to_string(quantity) + 
                       ",\"error\":\"" + lastError_ + "\"}");
        delete[] buffer;
        return false;
    }
    
    // Convert response to vector of uint16_t
    values.clear();
    values.reserve(quantity);
    for (int i = 0; i < quantity; i++) {
        values.push_back(buffer[i]);
    }
    
    // Clean up
    delete[] buffer;
    
    // Log success
    logger_.logDebug("modbus", "Read holding registers succeeded", 
                    "{\"slave\":" + std::to_string(slaveId) + 
                    ",\"address\":" + std::to_string(address) + 
                    ",\"quantity\":" + std::to_string(quantity) + "}");
    
    return true;
}

bool ModbusMaster::readInputRegisters(int slaveId, int address, int quantity, std::vector<uint16_t>& values) {
    if (!ctx_ || !connected_) {
        lastError_ = "Not connected";
        logger_.logError("modbus", "Not connected for read_input_registers", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + "}");
        return false;
    }
    
    // Set slave ID for this transaction
    modbus_set_slave(ctx_, slaveId);
    
    // Prepare buffer for response
    uint16_t* buffer = new uint16_t[quantity];
    memset(buffer, 0, quantity * sizeof(uint16_t));
    
    // Execute Modbus read input registers function
    int result = modbus_read_input_registers(ctx_, address, quantity, buffer);
    
    // Check for errors
    if (result == -1) {
        lastError_ = modbus_strerror(errno);
        lastErrorCode_ = errno;
        logger_.logError("modbus", "Read input registers failed", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + 
                       ",\"quantity\":" + std::to_string(quantity) + 
                       ",\"error\":\"" + lastError_ + "\"}");
        delete[] buffer;
        return false;
    }
    
    // Convert response to vector of uint16_t
    values.clear();
    values.reserve(quantity);
    for (int i = 0; i < quantity; i++) {
        values.push_back(buffer[i]);
    }
    
    // Clean up
    delete[] buffer;
    
    // Log success
    logger_.logDebug("modbus", "Read input registers succeeded", 
                    "{\"slave\":" + std::to_string(slaveId) + 
                    ",\"address\":" + std::to_string(address) + 
                    ",\"quantity\":" + std::to_string(quantity) + "}");
    
    return true;
}

bool ModbusMaster::writeSingleCoil(int slaveId, int address, bool value) {
    if (!ctx_ || !connected_) {
        lastError_ = "Not connected";
        logger_.logError("modbus", "Not connected for write_single_coil", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + "}");
        return false;
    }
    
    // Set slave ID for this transaction
    modbus_set_slave(ctx_, slaveId);
    
    // Execute Modbus write single coil function
    int result = modbus_write_bit(ctx_, address, value ? TRUE : FALSE);
    
    // Check for errors
    if (result == -1) {
        lastError_ = modbus_strerror(errno);
        lastErrorCode_ = errno;
        logger_.logError("modbus", "Write single coil failed", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + 
                       ",\"value\":" + (value ? "true" : "false") + 
                       ",\"error\":\"" + lastError_ + "\"}");
        return false;
    }
    
    // Log success
    logger_.logDebug("modbus", "Write single coil succeeded", 
                    "{\"slave\":" + std::to_string(slaveId) + 
                    ",\"address\":" + std::to_string(address) + 
                    ",\"value\":" + (value ? "true" : "false") + "}");
    
    return true;
}

bool ModbusMaster::writeSingleRegister(int slaveId, int address, uint16_t value) {
    if (!ctx_ || !connected_) {
        lastError_ = "Not connected";
        logger_.logError("modbus", "Not connected for write_single_register", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + "}");
        return false;
    }
    
    // Set slave ID for this transaction
    modbus_set_slave(ctx_, slaveId);
    
    // Execute Modbus write single register function
    int result = modbus_write_register(ctx_, address, value);
    
    // Check for errors
    if (result == -1) {
        lastError_ = modbus_strerror(errno);
        lastErrorCode_ = errno;
        logger_.logError("modbus", "Write single register failed", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + 
                       ",\"value\":" + std::to_string(value) + 
                       ",\"error\":\"" + lastError_ + "\"}");
        return false;
    }
    
    // Log success
    logger_.logDebug("modbus", "Write single register succeeded", 
                    "{\"slave\":" + std::to_string(slaveId) + 
                    ",\"address\":" + std::to_string(address) + 
                    ",\"value\":" + std::to_string(value) + "}");
    
    return true;
}

bool ModbusMaster::writeMultipleCoils(int slaveId, int address, const std::vector<bool>& values) {
    if (!ctx_ || !connected_) {
        lastError_ = "Not connected";
        logger_.logError("modbus", "Not connected for write_multiple_coils", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + "}");
        return false;
    }
    
    // Set slave ID for this transaction
    modbus_set_slave(ctx_, slaveId);
    
    // Prepare buffer from vector of booleans
    int quantity = static_cast<int>(values.size());
    uint8_t* buffer = new uint8_t[quantity];
    for (int i = 0; i < quantity; i++) {
        buffer[i] = values[i] ? TRUE : FALSE;
    }
    
    // Execute Modbus write multiple coils function
    int result = modbus_write_bits(ctx_, address, quantity, buffer);
    
    // Check for errors
    if (result == -1) {
        lastError_ = modbus_strerror(errno);
        lastErrorCode_ = errno;
        logger_.logError("modbus", "Write multiple coils failed", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + 
                       ",\"quantity\":" + std::to_string(quantity) + 
                       ",\"error\":\"" + lastError_ + "\"}");
        delete[] buffer;
        return false;
    }
    
    // Clean up
    delete[] buffer;
    
    // Log success
    logger_.logDebug("modbus", "Write multiple coils succeeded", 
                    "{\"slave\":" + std::to_string(slaveId) + 
                    ",\"address\":" + std::to_string(address) + 
                    ",\"quantity\":" + std::to_string(quantity) + "}");
    
    return true;
}

bool ModbusMaster::writeMultipleRegisters(int slaveId, int address, const std::vector<uint16_t>& values) {
    if (!ctx_ || !connected_) {
        lastError_ = "Not connected";
        logger_.logError("modbus", "Not connected for write_multiple_registers", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + "}");
        return false;
    }
    
    // Set slave ID for this transaction
    modbus_set_slave(ctx_, slaveId);
    
    // Get quantity and check if valid
    int quantity = static_cast<int>(values.size());
    if (quantity <= 0) {
        lastError_ = "Invalid quantity for write_multiple_registers";
        logger_.logError("modbus", "Invalid quantity", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + 
                       ",\"quantity\":" + std::to_string(quantity) + "}");
        return false;
    }
    
    // Execute Modbus write multiple registers function
    int result = modbus_write_registers(ctx_, address, quantity, values.data());
    
    // Check for errors
    if (result == -1) {
        lastError_ = modbus_strerror(errno);
        lastErrorCode_ = errno;
        logger_.logError("modbus", "Write multiple registers failed", 
                       "{\"slave\":" + std::to_string(slaveId) + 
                       ",\"address\":" + std::to_string(address) + 
                       ",\"quantity\":" + std::to_string(quantity) + 
                       ",\"error\":\"" + lastError_ + "\"}");
        return false;
    }
    
    // Log success
    logger_.logDebug("modbus", "Write multiple registers succeeded", 
                    "{\"slave\":" + std::to_string(slaveId) + 
                    ",\"address\":" + std::to_string(address) + 
                    ",\"quantity\":" + std::to_string(quantity) + "}");
    
    return true;
}

// Static methods for callbacks
std::string ModbusMaster::formatMessage(const uint8_t* data, int len) {
    std::stringstream ss;
    for (int i = 0; i < len; i++) {
        ss << std::hex << std::setw(2) << std::setfill('0') 
           << static_cast<int>(data[i]);
        if (i < len - 1) {
            ss << " ";
        }
    }
    return ss.str();
}

void ModbusMaster::modbusPreSendCallback(modbus_t* ctx, uint8_t* req, int req_len) {
    if (instance && instance->logCallback_) {
        std::string message = "TX: " + formatMessage(req, req_len);
        instance->logCallback_(message);
    }
}

void ModbusMaster::modbusPostRecvCallback(modbus_t* ctx, uint8_t* rsp, int rsp_len) {
    if (instance && instance->logCallback_) {
        std::string message = "RX: " + formatMessage(rsp, rsp_len);
        instance->logCallback_(message);
    }
}

// Override the base class's channelThreadFunc for Modbus-specific behavior
void ModbusMaster::channelThreadFunc(int channelIndex) {
    logger_.logDebug("modbus", "Modbus master channel thread started", 
                    "{\"index\":" + std::to_string(channelIndex) + "}");
    
    // Get channel config
    ChannelConfig channelConfig;
    {
        std::lock_guard<std::mutex> lock(channelsMutex_);
        auto it = channels_.find(channelIndex);
        if (it == channels_.end()) {
            logger_.logError("modbus", "Channel not found in thread", 
                           "{\"index\":" + std::to_string(channelIndex) + "}");
            return;
        }
        channelConfig = it->second;
    }
    
    // Extract Modbus specific configuration based on physical interface type
    int slaveId = 1; // Default slave ID
    
    if (physicalInterface_ == PhysicalInterfaceType::NETWORK) {
        if (std::holds_alternative<ModbusTCPConfig>(channelConfig.protocolConfig)) {
            const auto& config = std::get<ModbusTCPConfig>(channelConfig.protocolConfig);
            slaveId = config.slaveId;
        }
    } else if (physicalInterface_ == PhysicalInterfaceType::SERIAL) {
        if (std::holds_alternative<ModbusRTUConfig>(channelConfig.protocolConfig)) {
            const auto& config = std::get<ModbusRTUConfig>(channelConfig.protocolConfig);
            slaveId = config.slaveId;
        }
    }
    
    // Thread main loop - for a master device, we poll the slave devices
    while (channelRunning_[channelIndex]) {
        try {
            // First check if we're connected
            if (!connected_) {
                logger_.logWarning("modbus", "Master not connected, retrying...", 
                                 "{\"index\":" + std::to_string(channelIndex) + "}");
                
                // Sleep before retry
                std::this_thread::sleep_for(std::chrono::seconds(1));
                continue;
            }
            
            // Get all points for this channel
            std::map<std::string, DataPointConfig> channelPoints;
            {
                std::lock_guard<std::mutex> lock(dataPointsMutex_);
                channelPoints = channelConfig.points;
            }
            
            // Analyze address ranges for optimized reading
            std::vector<AddressRange> ranges = analyzeAddressRanges(channelPoints, maxRead_);
            
            // Process each range
            for (const auto& range : ranges) {
                if (!channelRunning_[channelIndex]) break; // Check if we should exit
                
                std::vector<uint16_t> values;
                bool success = false;
                
                // Read appropriate register type based on function code
                switch (range.functionCode) {
                    case static_cast<int>(ModbusFunctionCode::READ_COILS): {
                        std::vector<bool> boolValues;
                        success = readCoils(slaveId, range.startAddress, range.quantity, boolValues);
                        if (success) {
                            // Convert bool values to uint16_t (0 or 1)
                            values.reserve(boolValues.size());
                            for (bool val : boolValues) {
                                values.push_back(val ? 1 : 0);
                            }
                        }
                        break;
                    }
                    case static_cast<int>(ModbusFunctionCode::READ_DISCRETE_INPUTS): {
                        std::vector<bool> boolValues;
                        success = readDiscreteInputs(slaveId, range.startAddress, range.quantity, boolValues);
                        if (success) {
                            // Convert bool values to uint16_t (0 or 1)
                            values.reserve(boolValues.size());
                            for (bool val : boolValues) {
                                values.push_back(val ? 1 : 0);
                            }
                        }
                        break;
                    }
                    case static_cast<int>(ModbusFunctionCode::READ_HOLDING_REGISTERS):
                        success = readHoldingRegisters(slaveId, range.startAddress, range.quantity, values);
                        break;
                    case static_cast<int>(ModbusFunctionCode::READ_INPUT_REGISTERS):
                        success = readInputRegisters(slaveId, range.startAddress, range.quantity, values);
                        break;
                    default:
                        logger_.logError("modbus", "Unsupported function code", 
                                       "{\"code\":" + std::to_string(range.functionCode) + "}");
                        continue;
                }
                
                if (success) {
                    // Process the data and update points
                    processRangeData(range, values, channelPoints);
                } else {
                    logger_.logError("modbus", "Failed to read range", 
                                   "{\"address\":" + std::to_string(range.startAddress) + 
                                   ",\"quantity\":" + std::to_string(range.quantity) + 
                                   ",\"error\":\"" + lastError_ + "\"}");
                }
                
                // Small delay between reads to avoid flooding the device
                std::this_thread::sleep_for(std::chrono::milliseconds(50));
            }
            
            // Sleep at end of cycle
            std::this_thread::sleep_for(std::chrono::milliseconds(channelConfig.pollRate));
            
        } catch (const std::exception& e) {
            logger_.logError("modbus", "Exception in master channel thread", 
                           "{\"index\":" + std::to_string(channelIndex) + 
                           ",\"error\":\"" + e.what() + "\"}");
            
            // Sleep longer after error
            std::this_thread::sleep_for(std::chrono::seconds(1));
        }
    }
    
    logger_.logDebug("modbus", "Master channel thread exiting", 
                    "{\"index\":" + std::to_string(channelIndex) + "}");
}

// Data point size calculation
int ModbusMaster::getPointSize(DataType type) {
    switch (type) {
        case DataType::BOOL:
        case DataType::INT16:
        case DataType::UINT16:
            return 1; // 1 register (16 bits)
            
        case DataType::INT32:
        case DataType::UINT32:
        case DataType::FLOAT32:
            return 2; // 2 registers (32 bits)
            
        default:
            return 1; // Default to 1 register
    }
}

// Address range analysis for optimized reading
std::vector<AddressRange> ModbusMaster::analyzeAddressRanges(
    const std::map<std::string, DataPointConfig>& points, int maxRead) {
    
    std::vector<AddressRange> ranges;
    
    // Check if there are any points
    if (points.empty()) {
        return ranges;
    }
    
    // Sort all points by address
    struct PointAddress {
        std::string id;
        int address;
        int size;
        int functionCode;
    };
    
    std::vector<PointAddress> pointAddresses;
    
    for (const auto& point : points) {
        const auto& config = point.second;
        
        // Skip if not a Modbus point
        if (!std::holds_alternative<ModbusPointConfig>(config.pointConfig_)) {
            continue;
        }
        
        const auto& modbus = std::get<ModbusPointConfig>(config.pointConfig_);
        
        PointAddress pa;
        pa.id = point.first;
        pa.address = modbus.address;
        pa.size = getPointSize(config.datatype_);
        pa.functionCode = modbus.functionCode;
        
        pointAddresses.push_back(pa);
    }
    
    // Sort by function code first, then by address
    std::sort(pointAddresses.begin(), pointAddresses.end(), 
             [](const PointAddress& a, const PointAddress& b) {
                 if (a.functionCode != b.functionCode) {
                     return a.functionCode < b.functionCode;
                 }
                 return a.address < b.address;
             });
    
    // If no points after filtering, return empty ranges
    if (pointAddresses.empty()) {
        return ranges;
    }
    
    // Group points into ranges
    int currentFunctionCode = pointAddresses[0].functionCode;
    int currentStartAddress = pointAddresses[0].address;
    int currentEndAddress = currentStartAddress + pointAddresses[0].size - 1;
    std::vector<std::string> currentPointIds = { pointAddresses[0].id };
    
    for (size_t i = 1; i < pointAddresses.size(); i++) {
        const auto& point = pointAddresses[i];
        
        // If function code changes, start a new range
        if (point.functionCode != currentFunctionCode) {
            // Add the current range
            AddressRange range;
            range.startAddress = currentStartAddress;
            range.quantity = currentEndAddress - currentStartAddress + 1;
            range.functionCode = currentFunctionCode;
            range.pointIds = currentPointIds;
            ranges.push_back(range);
            
            // Start a new range
            currentFunctionCode = point.functionCode;
            currentStartAddress = point.address;
            currentEndAddress = point.address + point.size - 1;
            currentPointIds = { point.id };
            continue;
        }
        
        // Calculate gap between current range and this point
        int gap = point.address - (currentEndAddress + 1);
        
        // If gap is too large or adding this point would exceed maxRead, start a new range
        if (gap > 10 || (point.address + point.size - currentStartAddress) > maxRead) {
            // Add the current range
            AddressRange range;
            range.startAddress = currentStartAddress;
            range.quantity = currentEndAddress - currentStartAddress + 1;
            range.functionCode = currentFunctionCode;
            range.pointIds = currentPointIds;
            ranges.push_back(range);
            
            // Start a new range
            currentStartAddress = point.address;
            currentEndAddress = point.address + point.size - 1;
            currentPointIds = { point.id };
        } else {
            // Extend the current range
            currentEndAddress = std::max(currentEndAddress, point.address + point.size - 1);
            currentPointIds.push_back(point.id);
        }
    }
    
    // Add the last range
    if (!currentPointIds.empty()) {
        AddressRange range;
        range.startAddress = currentStartAddress;
        range.quantity = currentEndAddress - currentStartAddress + 1;
        range.functionCode = currentFunctionCode;
        range.pointIds = currentPointIds;
        ranges.push_back(range);
    }
    
    return ranges;
}

// Process data for a range of addresses
void ModbusMaster::processRangeData(
    const AddressRange& range, 
    const std::vector<uint16_t>& values, 
    const std::map<std::string, DataPointConfig>& points) {
    
    // Check if we have enough data
    if (values.size() < range.quantity) {
        logger_.logError("modbus", "Insufficient data for range", 
                       "{\"address\":" + std::to_string(range.startAddress) + 
                       ",\"quantity\":" + std::to_string(range.quantity) + 
                       ",\"received\":" + std::to_string(values.size()) + "}");
        return;
    }
    
    // Process each point in the range
    for (const auto& pointId : range.pointIds) {
        // Find the point configuration
        auto pointIt = points.find(pointId);
        if (pointIt == points.end()) {
            logger_.logWarning("modbus", "Point not found in configuration", 
                             "{\"id\":\"" + pointId + "\"}");
            continue;
        }
        
        const auto& config = pointIt->second;
        
        // Check if it's a Modbus point
        if (!std::holds_alternative<ModbusPointConfig>(config.pointConfig_)) {
            logger_.logWarning("modbus", "Not a Modbus point", 
                             "{\"id\":\"" + pointId + "\"}");
            continue;
        }
        
        const auto& modbus = std::get<ModbusPointConfig>(config.pointConfig_);
        
        // Calculate offset in the values array
        int offset = modbus.address - range.startAddress;
        
        // Check if the offset is valid
        if (offset < 0 || offset >= static_cast<int>(values.size())) {
            logger_.logError("modbus", "Invalid offset for point", 
                           "{\"id\":\"" + pointId + 
                           "\",\"address\":" + std::to_string(modbus.address) + 
                           ",\"rangeStart\":" + std::to_string(range.startAddress) + 
                           ",\"offset\":" + std::to_string(offset) + "}");
            continue;
        }
        
        // Get the size of the data point
        int pointSize = getPointSize(config.datatype_);
        
        // Check if we have enough data for this point
        if (offset + pointSize > static_cast<int>(values.size())) {
            logger_.logError("modbus", "Insufficient data for point", 
                           "{\"id\":\"" + pointId + 
                           "\",\"address\":" + std::to_string(modbus.address) + 
                           ",\"size\":" + std::to_string(pointSize) + 
                           ",\"available\":" + std::to_string(values.size() - offset) + "}");
            continue;
        }
        
        // Extract the relevant data for this point
        std::vector<uint16_t> pointData(values.begin() + offset, values.begin() + offset + pointSize);
        
        // Parse the data and create a value
        DataPointValue value = parseData(pointId, pointData);
        
        // Update the point data in Redis if successful
        if (value.isValid) {
            writeDataToRedis(value);
            
            // Call appropriate callback based on point type
            switch (config.pointType_) {
                case PointType::DI:
                    processDIData(pointId, pointData);
                    break;
                    
                case PointType::AI:
                    processAIData(pointId, pointData);
                    break;
                    
                default:
                    // Other point types not expected for read operations
                    break;
            }
        } else {
            logger_.logWarning("modbus", "Invalid data for point", 
                             "{\"id\":\"" + pointId + 
                             "\",\"value\":" + std::to_string(value.value) + "}");
        }
    }
}

// Factory function implementation
std::unique_ptr<ModbusMaster> createModbusMaster(PhysicalInterfaceType interfaceType) {
    // This will be implemented when ModbusTCPMaster and ModbusRTUMaster are created
    return nullptr;
} 