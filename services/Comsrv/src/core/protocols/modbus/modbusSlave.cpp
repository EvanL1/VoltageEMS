#include "protocols/modbus/modbusSlave.h"
#include "protocols/modbus/modbusTCPSlave.h"
#include "protocols/modbus/modbusRTUSlave.h"
#include <iostream>
#include <chrono>
#include <iomanip>
#include <algorithm>
#include <thread>
#include <cstring>

// Static instance pointer for callbacks
ModbusSlave* ModbusSlave::instance = nullptr;

ModbusSlave::ModbusSlave() 
    : ctx_(nullptr), 
      mapping_(nullptr),
      slaveId_(1), 
      timeout_(1000), 
      debug_(false), 
      connected_(false),
      running_(false) {
    // Set instance for callback functions
    instance = this;
}

ModbusSlave::~ModbusSlave() {
    // Stop listening thread if running
    stopListening();
    
    // Free Modbus mapping if allocated
    if (mapping_) {
        modbus_mapping_free(mapping_);
        mapping_ = nullptr;
    }
    
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

bool ModbusSlave::start() {
    if (running_) {
        return true;  // Already running
    }
    
    // Start the listening thread
    if (!startListening()) {
        return false;
    }
    
    return ComBase::start();
}

bool ModbusSlave::stop() {
    if (!running_) {
        return true;  // Already stopped
    }
    
    // Stop the listening thread
    stopListening();
    
    return ComBase::stop();
}

bool ModbusSlave::isRunning() const {
    return running_ && connected_;
}

bool ModbusSlave::startListening() {
    if (running_) {
        return true;  // Already running
    }
    
    if (!connected_) {
        lastError_ = "Cannot start listening: not connected";
        logger_.logError("modbus_slave", "Start listening failed", lastError_);
        return false;
    }
    
    // Set up running flag and start thread
    running_ = true;
    listeningThread_ = std::thread(&ModbusSlave::listenThreadFunc, this);
    
    logger_.logInfo("modbus_slave", "Started listening", 
                  "{\"slaveId\":" + std::to_string(slaveId_) + "}");
    
    return true;
}

bool ModbusSlave::stopListening() {
    if (!running_) {
        return true;  // Already stopped
    }
    
    // Signal thread to stop
    running_ = false;
    
    // Wait for thread to finish
    if (listeningThread_.joinable()) {
        listeningThread_.join();
    }
    
    logger_.logInfo("modbus_slave", "Stopped listening", 
                  "{\"slaveId\":" + std::to_string(slaveId_) + "}");
    
    return true;
}

bool ModbusSlave::setupModbusMapping(int nbCoils, int nbDiscreteInputs, int nbHoldingRegisters, int nbInputRegisters) {
    // Free existing mapping if any
    if (mapping_) {
        modbus_mapping_free(mapping_);
        mapping_ = nullptr;
    }
    
    // Create new mapping
    mapping_ = modbus_mapping_new(nbCoils, nbDiscreteInputs, nbHoldingRegisters, nbInputRegisters);
    
    if (mapping_ == nullptr) {
        lastError_ = modbus_strerror(errno);
        logger_.logError("modbus_slave", "Failed to create mapping", 
                       "{\"error\":\"" + lastError_ + "\"}");
        return false;
    }
    
    // Initialize register maps from the Modbus mapping
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    // Clear existing maps
    coils_.clear();
    discreteInputs_.clear();
    holdingRegisters_.clear();
    inputRegisters_.clear();
    
    // Initialize maps with values from modbus mapping
    for (int i = 0; i < nbCoils; i++) {
        coils_[i] = mapping_->tab_bits[i] != 0;
    }
    
    for (int i = 0; i < nbDiscreteInputs; i++) {
        discreteInputs_[i] = mapping_->tab_input_bits[i] != 0;
    }
    
    for (int i = 0; i < nbHoldingRegisters; i++) {
        holdingRegisters_[i] = mapping_->tab_registers[i];
    }
    
    for (int i = 0; i < nbInputRegisters; i++) {
        inputRegisters_[i] = mapping_->tab_input_registers[i];
    }
    
    logger_.logInfo("modbus_slave", "Modbus mapping created", 
                  "{\"coils\":" + std::to_string(nbCoils) + 
                  ",\"discrete_inputs\":" + std::to_string(nbDiscreteInputs) + 
                  ",\"holding_registers\":" + std::to_string(nbHoldingRegisters) + 
                  ",\"input_registers\":" + std::to_string(nbInputRegisters) + "}");
    
    return true;
}

bool ModbusSlave::setSlaveId(int id) {
    if (id < 0 || id > 247) {
        lastError_ = "Invalid slave ID (valid range: 0-247)";
        logger_.logError("modbus_slave", "Invalid slave ID", 
                       "{\"id\":" + std::to_string(id) + "}");
        return false;
    }
    
    slaveId_ = id;
    
    if (ctx_) {
        if (modbus_set_slave(ctx_, id) == -1) {
            lastError_ = modbus_strerror(errno);
            logger_.logError("modbus_slave", "Failed to set slave ID", 
                           "{\"id\":" + std::to_string(id) + 
                           ",\"error\":\"" + lastError_ + "\"}");
            return false;
        }
    }
    
    return true;
}

bool ModbusSlave::setTimeout(int ms) {
    if (ms <= 0) {
        lastError_ = "Invalid timeout value";
        logger_.logError("modbus_slave", "Invalid timeout", 
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
            logger_.logError("modbus_slave", "Failed to set timeout", 
                           "{\"timeout\":" + std::to_string(ms) + 
                           ",\"error\":\"" + lastError_ + "\"}");
            return false;
        }
    }
    
    return true;
}

bool ModbusSlave::setDebug(bool enable) {
    debug_ = enable;
    
    if (ctx_) {
        modbus_set_debug(ctx_, enable ? TRUE : FALSE);
    }
    
    return true;
}

void ModbusSlave::setResponseTimeout(uint32_t sec, uint32_t usec) {
    if (ctx_) {
        modbus_set_response_timeout(ctx_, sec, usec);
    }
}

int ModbusSlave::getSlaveId() const {
    return slaveId_;
}

int ModbusSlave::getLastErrorCode() const {
    return lastErrorCode_;
}

std::string ModbusSlave::getLastError() const {
    return lastError_;
}

void ModbusSlave::setHoldingRegisterCallback(RegisterWriteCallback callback) {
    holdingRegisterCallback_ = callback;
}

void ModbusSlave::setCoilCallback(CoilWriteCallback callback) {
    coilCallback_ = callback;
}

// Coil (output) access methods
bool ModbusSlave::setCoil(int address, bool value) {
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    // Update our internal map
    coils_[address] = value;
    
    // Update the modbus mapping if available
    if (mapping_ && address >= 0 && address < mapping_->nb_bits) {
        mapping_->tab_bits[address] = value ? 1 : 0;
    } else if (mapping_) {
        // Address is outside the allocated range for the modbus mapping
        logger_.logWarning("modbus_slave", "Coil address out of range", 
                         "{\"address\":" + std::to_string(address) + 
                         ",\"max\":" + std::to_string(mapping_->nb_bits - 1) + "}");
    }
    
    return true;
}

bool ModbusSlave::getCoil(int address, bool& value) const {
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    auto it = coils_.find(address);
    if (it != coils_.end()) {
        value = it->second;
        return true;
    }
    
    // If not in our map but in modbus mapping range, get from there
    if (mapping_ && address >= 0 && address < mapping_->nb_bits) {
        value = mapping_->tab_bits[address] != 0;
        return true;
    }
    
    // Not found
    return false;
}

bool ModbusSlave::setCoils(int startAddress, const std::vector<bool>& values) {
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    // Update our internal map
    for (size_t i = 0; i < values.size(); i++) {
        coils_[startAddress + i] = values[i];
    }
    
    // Update the modbus mapping if available
    if (mapping_) {
        for (size_t i = 0; i < values.size(); i++) {
            int address = startAddress + i;
            if (address >= 0 && address < mapping_->nb_bits) {
                mapping_->tab_bits[address] = values[i] ? 1 : 0;
            } else {
                // Address is outside the allocated range for the modbus mapping
                logger_.logWarning("modbus_slave", "Coil address out of range", 
                                 "{\"address\":" + std::to_string(address) + 
                                 ",\"max\":" + std::to_string(mapping_->nb_bits - 1) + "}");
            }
        }
    }
    
    return true;
}

bool ModbusSlave::getCoils(int startAddress, int quantity, std::vector<bool>& values) const {
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    values.clear();
    values.resize(quantity, false);
    
    bool allFound = true;
    
    for (int i = 0; i < quantity; i++) {
        int address = startAddress + i;
        auto it = coils_.find(address);
        
        if (it != coils_.end()) {
            values[i] = it->second;
        } else if (mapping_ && address >= 0 && address < mapping_->nb_bits) {
            values[i] = mapping_->tab_bits[address] != 0;
        } else {
            allFound = false;
        }
    }
    
    return allFound;
}

// Discrete Input access methods
bool ModbusSlave::setDiscreteInput(int address, bool value) {
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    // Update our internal map
    discreteInputs_[address] = value;
    
    // Update the modbus mapping if available
    if (mapping_ && address >= 0 && address < mapping_->nb_input_bits) {
        mapping_->tab_input_bits[address] = value ? 1 : 0;
    } else if (mapping_) {
        // Address is outside the allocated range for the modbus mapping
        logger_.logWarning("modbus_slave", "Discrete input address out of range", 
                         "{\"address\":" + std::to_string(address) + 
                         ",\"max\":" + std::to_string(mapping_->nb_input_bits - 1) + "}");
    }
    
    return true;
}

bool ModbusSlave::getDiscreteInput(int address, bool& value) const {
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    auto it = discreteInputs_.find(address);
    if (it != discreteInputs_.end()) {
        value = it->second;
        return true;
    }
    
    // If not in our map but in modbus mapping range, get from there
    if (mapping_ && address >= 0 && address < mapping_->nb_input_bits) {
        value = mapping_->tab_input_bits[address] != 0;
        return true;
    }
    
    // Not found
    return false;
}

bool ModbusSlave::setDiscreteInputs(int startAddress, const std::vector<bool>& values) {
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    // Update our internal map
    for (size_t i = 0; i < values.size(); i++) {
        discreteInputs_[startAddress + i] = values[i];
    }
    
    // Update the modbus mapping if available
    if (mapping_) {
        for (size_t i = 0; i < values.size(); i++) {
            int address = startAddress + i;
            if (address >= 0 && address < mapping_->nb_input_bits) {
                mapping_->tab_input_bits[address] = values[i] ? 1 : 0;
            } else {
                // Address is outside the allocated range for the modbus mapping
                logger_.logWarning("modbus_slave", "Discrete input address out of range", 
                                 "{\"address\":" + std::to_string(address) + 
                                 ",\"max\":" + std::to_string(mapping_->nb_input_bits - 1) + "}");
            }
        }
    }
    
    return true;
}

bool ModbusSlave::getDiscreteInputs(int startAddress, int quantity, std::vector<bool>& values) const {
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    values.clear();
    values.resize(quantity, false);
    
    bool allFound = true;
    
    for (int i = 0; i < quantity; i++) {
        int address = startAddress + i;
        auto it = discreteInputs_.find(address);
        
        if (it != discreteInputs_.end()) {
            values[i] = it->second;
        } else if (mapping_ && address >= 0 && address < mapping_->nb_input_bits) {
            values[i] = mapping_->tab_input_bits[address] != 0;
        } else {
            allFound = false;
        }
    }
    
    return allFound;
}

// Holding Register access methods
bool ModbusSlave::setHoldingRegister(int address, uint16_t value) {
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    // Update our internal map
    holdingRegisters_[address] = value;
    
    // Update the modbus mapping if available
    if (mapping_ && address >= 0 && address < mapping_->nb_registers) {
        mapping_->tab_registers[address] = value;
    } else if (mapping_) {
        // Address is outside the allocated range for the modbus mapping
        logger_.logWarning("modbus_slave", "Holding register address out of range", 
                         "{\"address\":" + std::to_string(address) + 
                         ",\"max\":" + std::to_string(mapping_->nb_registers - 1) + "}");
    }
    
    return true;
}

bool ModbusSlave::getHoldingRegister(int address, uint16_t& value) const {
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    auto it = holdingRegisters_.find(address);
    if (it != holdingRegisters_.end()) {
        value = it->second;
        return true;
    }
    
    // If not in our map but in modbus mapping range, get from there
    if (mapping_ && address >= 0 && address < mapping_->nb_registers) {
        value = mapping_->tab_registers[address];
        return true;
    }
    
    // Not found
    return false;
}

bool ModbusSlave::setHoldingRegisters(int startAddress, const std::vector<uint16_t>& values) {
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    // Update our internal map
    for (size_t i = 0; i < values.size(); i++) {
        holdingRegisters_[startAddress + i] = values[i];
    }
    
    // Update the modbus mapping if available
    if (mapping_) {
        for (size_t i = 0; i < values.size(); i++) {
            int address = startAddress + i;
            if (address >= 0 && address < mapping_->nb_registers) {
                mapping_->tab_registers[address] = values[i];
            } else {
                // Address is outside the allocated range for the modbus mapping
                logger_.logWarning("modbus_slave", "Holding register address out of range", 
                                 "{\"address\":" + std::to_string(address) + 
                                 ",\"max\":" + std::to_string(mapping_->nb_registers - 1) + "}");
            }
        }
    }
    
    return true;
}

bool ModbusSlave::getHoldingRegisters(int startAddress, int quantity, std::vector<uint16_t>& values) const {
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    values.clear();
    values.resize(quantity, 0);
    
    bool allFound = true;
    
    for (int i = 0; i < quantity; i++) {
        int address = startAddress + i;
        auto it = holdingRegisters_.find(address);
        
        if (it != holdingRegisters_.end()) {
            values[i] = it->second;
        } else if (mapping_ && address >= 0 && address < mapping_->nb_registers) {
            values[i] = mapping_->tab_registers[address];
        } else {
            allFound = false;
        }
    }
    
    return allFound;
}

// Input Register access methods
bool ModbusSlave::setInputRegister(int address, uint16_t value) {
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    // Update our internal map
    inputRegisters_[address] = value;
    
    // Update the modbus mapping if available
    if (mapping_ && address >= 0 && address < mapping_->nb_input_registers) {
        mapping_->tab_input_registers[address] = value;
    } else if (mapping_) {
        // Address is outside the allocated range for the modbus mapping
        logger_.logWarning("modbus_slave", "Input register address out of range", 
                         "{\"address\":" + std::to_string(address) + 
                         ",\"max\":" + std::to_string(mapping_->nb_input_registers - 1) + "}");
    }
    
    return true;
}

bool ModbusSlave::getInputRegister(int address, uint16_t& value) const {
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    auto it = inputRegisters_.find(address);
    if (it != inputRegisters_.end()) {
        value = it->second;
        return true;
    }
    
    // If not in our map but in modbus mapping range, get from there
    if (mapping_ && address >= 0 && address < mapping_->nb_input_registers) {
        value = mapping_->tab_input_registers[address];
        return true;
    }
    
    // Not found
    return false;
}

bool ModbusSlave::setInputRegisters(int startAddress, const std::vector<uint16_t>& values) {
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    // Update our internal map
    for (size_t i = 0; i < values.size(); i++) {
        inputRegisters_[startAddress + i] = values[i];
    }
    
    // Update the modbus mapping if available
    if (mapping_) {
        for (size_t i = 0; i < values.size(); i++) {
            int address = startAddress + i;
            if (address >= 0 && address < mapping_->nb_input_registers) {
                mapping_->tab_input_registers[address] = values[i];
            } else {
                // Address is outside the allocated range for the modbus mapping
                logger_.logWarning("modbus_slave", "Input register address out of range", 
                                 "{\"address\":" + std::to_string(address) + 
                                 ",\"max\":" + std::to_string(mapping_->nb_input_registers - 1) + "}");
            }
        }
    }
    
    return true;
}

bool ModbusSlave::getInputRegisters(int startAddress, int quantity, std::vector<uint16_t>& values) const {
    std::lock_guard<std::mutex> lock(registerMutex_);
    
    values.clear();
    values.resize(quantity, 0);
    
    bool allFound = true;
    
    for (int i = 0; i < quantity; i++) {
        int address = startAddress + i;
        auto it = inputRegisters_.find(address);
        
        if (it != inputRegisters_.end()) {
            values[i] = it->second;
        } else if (mapping_ && address >= 0 && address < mapping_->nb_input_registers) {
            values[i] = mapping_->tab_input_registers[address];
        } else {
            allFound = false;
        }
    }
    
    return allFound;
}

// Static methods for callbacks
std::string ModbusSlave::formatMessage(const uint8_t* data, int len) {
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

void ModbusSlave::modbusPreSendCallback(modbus_t* ctx, uint8_t* req, int req_len) {
    if (instance && instance->logCallback_) {
        std::string message = "TX: " + formatMessage(req, req_len);
        instance->logCallback_(message);
    }
}

void ModbusSlave::modbusPostRecvCallback(modbus_t* ctx, uint8_t* rsp, int rsp_len) {
    if (instance && instance->logCallback_) {
        std::string message = "RX: " + formatMessage(rsp, rsp_len);
        instance->logCallback_(message);
    }
}

// Modbus exception handling
void ModbusSlave::buildExceptionResponse(uint8_t functionCode, ModbusExceptionCode exceptionCode, uint8_t* response, int* responseLength) {
    // Set the error bit in the function code (MSB)
    response[0] = functionCode | 0x80;
    // Set the exception code
    response[1] = static_cast<uint8_t>(exceptionCode);
    // Set the response length
    *responseLength = 2;
    
    // Update exception statistics
    exceptionsSent_++;
    
    logger_.logWarning("modbus_slave", "Sending exception", 
                     "{\"function\":\"" + std::to_string(functionCode) + 
                     "\",\"exception\":\"" + std::to_string(static_cast<int>(exceptionCode)) + "\"}");
}

// Function code processing methods
void ModbusSlave::processReadCoils(uint8_t* request, int offset, int* requestLength) {
    // Extract parameters from request
    int address = (request[offset + 1] << 8) | request[offset + 2];
    int quantity = (request[offset + 3] << 8) | request[offset + 4];
    
    // Validate quantity
    if (quantity < 1 || quantity > 2000) {
        buildExceptionResponse(request[offset], ModbusExceptionCode::ILLEGAL_DATA_VALUE, request, requestLength);
        return;
    }
    
    // Get coil values
    std::vector<bool> values;
    bool success = getCoils(address, quantity, values);
    
    if (!success) {
        buildExceptionResponse(request[offset], ModbusExceptionCode::ILLEGAL_DATA_ADDRESS, request, requestLength);
        return;
    }
    
    // Calculate byte count for response
    int byteCount = (quantity + 7) / 8;
    
    // Build response
    request[offset + 1] = byteCount;
    
    // Pack bits into bytes
    for (int i = 0; i < byteCount; i++) {
        uint8_t byte = 0;
        for (int j = 0; j < 8; j++) {
            int bitIndex = i * 8 + j;
            if (bitIndex < quantity && values[bitIndex]) {
                byte |= (1 << j);
            }
        }
        request[offset + 2 + i] = byte;
    }
    
    // Set response length
    *requestLength = offset + 2 + byteCount;
}

void ModbusSlave::processReadDiscreteInputs(uint8_t* request, int offset, int* requestLength) {
    // Extract parameters from request
    int address = (request[offset + 1] << 8) | request[offset + 2];
    int quantity = (request[offset + 3] << 8) | request[offset + 4];
    
    // Validate quantity
    if (quantity < 1 || quantity > 2000) {
        buildExceptionResponse(request[offset], ModbusExceptionCode::ILLEGAL_DATA_VALUE, request, requestLength);
        return;
    }
    
    // Get discrete input values
    std::vector<bool> values;
    bool success = getDiscreteInputs(address, quantity, values);
    
    if (!success) {
        buildExceptionResponse(request[offset], ModbusExceptionCode::ILLEGAL_DATA_ADDRESS, request, requestLength);
        return;
    }
    
    // Calculate byte count for response
    int byteCount = (quantity + 7) / 8;
    
    // Build response
    request[offset + 1] = byteCount;
    
    // Pack bits into bytes
    for (int i = 0; i < byteCount; i++) {
        uint8_t byte = 0;
        for (int j = 0; j < 8; j++) {
            int bitIndex = i * 8 + j;
            if (bitIndex < quantity && values[bitIndex]) {
                byte |= (1 << j);
            }
        }
        request[offset + 2 + i] = byte;
    }
    
    // Set response length
    *requestLength = offset + 2 + byteCount;
}

void ModbusSlave::processReadHoldingRegisters(uint8_t* request, int offset, int* requestLength) {
    // Extract parameters from request
    int address = (request[offset + 1] << 8) | request[offset + 2];
    int quantity = (request[offset + 3] << 8) | request[offset + 4];
    
    // Validate quantity
    if (quantity < 1 || quantity > 125) {
        buildExceptionResponse(request[offset], ModbusExceptionCode::ILLEGAL_DATA_VALUE, request, requestLength);
        return;
    }
    
    // Get holding register values
    std::vector<uint16_t> values;
    bool success = getHoldingRegisters(address, quantity, values);
    
    if (!success) {
        buildExceptionResponse(request[offset], ModbusExceptionCode::ILLEGAL_DATA_ADDRESS, request, requestLength);
        return;
    }
    
    // Calculate byte count for response
    int byteCount = quantity * 2;
    
    // Build response
    request[offset + 1] = byteCount;
    
    // Pack registers into bytes
    for (int i = 0; i < quantity; i++) {
        request[offset + 2 + i * 2] = (values[i] >> 8) & 0xFF;
        request[offset + 3 + i * 2] = values[i] & 0xFF;
    }
    
    // Set response length
    *requestLength = offset + 2 + byteCount;
}

void ModbusSlave::processReadInputRegisters(uint8_t* request, int offset, int* requestLength) {
    // Extract parameters from request
    int address = (request[offset + 1] << 8) | request[offset + 2];
    int quantity = (request[offset + 3] << 8) | request[offset + 4];
    
    // Validate quantity
    if (quantity < 1 || quantity > 125) {
        buildExceptionResponse(request[offset], ModbusExceptionCode::ILLEGAL_DATA_VALUE, request, requestLength);
        return;
    }
    
    // Get input register values
    std::vector<uint16_t> values;
    bool success = getInputRegisters(address, quantity, values);
    
    if (!success) {
        buildExceptionResponse(request[offset], ModbusExceptionCode::ILLEGAL_DATA_ADDRESS, request, requestLength);
        return;
    }
    
    // Calculate byte count for response
    int byteCount = quantity * 2;
    
    // Build response
    request[offset + 1] = byteCount;
    
    // Pack registers into bytes
    for (int i = 0; i < quantity; i++) {
        request[offset + 2 + i * 2] = (values[i] >> 8) & 0xFF;
        request[offset + 3 + i * 2] = values[i] & 0xFF;
    }
    
    // Set response length
    *requestLength = offset + 2 + byteCount;
}

void ModbusSlave::processWriteSingleCoil(uint8_t* request, int offset, int* requestLength) {
    // Extract parameters from request
    int address = (request[offset + 1] << 8) | request[offset + 2];
    int value = (request[offset + 3] << 8) | request[offset + 4];
    
    // Validate value (only 0x0000 or 0xFF00 are valid)
    if (value != 0x0000 && value != 0xFF00) {
        buildExceptionResponse(request[offset], ModbusExceptionCode::ILLEGAL_DATA_VALUE, request, requestLength);
        return;
    }
    
    // Set coil value
    bool success = setCoil(address, value == 0xFF00);
    
    if (!success) {
        buildExceptionResponse(request[offset], ModbusExceptionCode::ILLEGAL_DATA_ADDRESS, request, requestLength);
        return;
    }
    
    // Notify via callback if registered
    if (coilCallback_) {
        coilCallback_(address, value == 0xFF00);
    }
    
    // Echo the request as the response
    *requestLength = offset + 5;
}

void ModbusSlave::processWriteSingleRegister(uint8_t* request, int offset, int* requestLength) {
    // Extract parameters from request
    int address = (request[offset + 1] << 8) | request[offset + 2];
    uint16_t value = (request[offset + 3] << 8) | request[offset + 4];
    
    // Set holding register value
    bool success = setHoldingRegister(address, value);
    
    if (!success) {
        buildExceptionResponse(request[offset], ModbusExceptionCode::ILLEGAL_DATA_ADDRESS, request, requestLength);
        return;
    }
    
    // Notify via callback if registered
    if (holdingRegisterCallback_) {
        holdingRegisterCallback_(address, value);
    }
    
    // Echo the request as the response
    *requestLength = offset + 5;
}

void ModbusSlave::processWriteMultipleCoils(uint8_t* request, int offset, int* requestLength) {
    // Extract parameters from request
    int address = (request[offset + 1] << 8) | request[offset + 2];
    int quantity = (request[offset + 3] << 8) | request[offset + 4];
    int byteCount = request[offset + 5];
    
    // Validate quantity
    if (quantity < 1 || quantity > 1968 || byteCount != (quantity + 7) / 8) {
        buildExceptionResponse(request[offset], ModbusExceptionCode::ILLEGAL_DATA_VALUE, request, requestLength);
        return;
    }
    
    // Extract coil values from request
    std::vector<bool> values;
    values.reserve(quantity);
    
    for (int i = 0; i < quantity; i++) {
        int byteIndex = i / 8;
        int bitIndex = i % 8;
        bool value = (request[offset + 6 + byteIndex] & (1 << bitIndex)) != 0;
        values.push_back(value);
    }
    
    // Set coil values
    bool success = setCoils(address, values);
    
    if (!success) {
        buildExceptionResponse(request[offset], ModbusExceptionCode::ILLEGAL_DATA_ADDRESS, request, requestLength);
        return;
    }
    
    // Notify via callback if registered
    if (coilCallback_) {
        for (int i = 0; i < quantity; i++) {
            coilCallback_(address + i, values[i]);
        }
    }
    
    // Build response (just the header with quantity)
    // request[offset] is already the function code
    // request[offset+1] and request[offset+2] are already the address
    // request[offset+3] and request[offset+4] are already the quantity
    
    // Set response length
    *requestLength = offset + 5;
}

void ModbusSlave::processWriteMultipleRegisters(uint8_t* request, int offset, int* requestLength) {
    // Extract parameters from request
    int address = (request[offset + 1] << 8) | request[offset + 2];
    int quantity = (request[offset + 3] << 8) | request[offset + 4];
    int byteCount = request[offset + 5];
    
    // Validate quantity
    if (quantity < 1 || quantity > 123 || byteCount != quantity * 2) {
        buildExceptionResponse(request[offset], ModbusExceptionCode::ILLEGAL_DATA_VALUE, request, requestLength);
        return;
    }
    
    // Extract register values from request
    std::vector<uint16_t> values;
    values.reserve(quantity);
    
    for (int i = 0; i < quantity; i++) {
        uint16_t value = (request[offset + 6 + i * 2] << 8) | request[offset + 7 + i * 2];
        values.push_back(value);
    }
    
    // Set holding register values
    bool success = setHoldingRegisters(address, values);
    
    if (!success) {
        buildExceptionResponse(request[offset], ModbusExceptionCode::ILLEGAL_DATA_ADDRESS, request, requestLength);
        return;
    }
    
    // Notify via callback if registered
    if (holdingRegisterCallback_) {
        for (int i = 0; i < quantity; i++) {
            holdingRegisterCallback_(address + i, values[i]);
        }
    }
    
    // Build response (just the header with quantity)
    // request[offset] is already the function code
    // request[offset+1] and request[offset+2] are already the address
    // request[offset+3] and request[offset+4] are already the quantity
    
    // Set response length
    *requestLength = offset + 5;
}

// Override the base class's channelThreadFunc for Modbus-specific behavior
void ModbusSlave::channelThreadFunc(int channelIndex) {
    logger_.logDebug("modbus", "Modbus slave channel thread started", 
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
    if (physicalInterface_ == PhysicalInterfaceType::NETWORK) {
        if (std::holds_alternative<ModbusTCPConfig>(channelConfig.protocolConfig)) {
            const auto& config = std::get<ModbusTCPConfig>(channelConfig.protocolConfig);
            // Apply TCP specific configuration
            slaveId_ = config.slaveId;
        }
    } else if (physicalInterface_ == PhysicalInterfaceType::SERIAL) {
        if (std::holds_alternative<ModbusRTUConfig>(channelConfig.protocolConfig)) {
            const auto& config = std::get<ModbusRTUConfig>(channelConfig.protocolConfig);
            // Apply RTU specific configuration
            slaveId_ = config.slaveId;
        }
    }
    
    // Get data points and initialize register maps
    int maxCoilAddr = -1;
    int maxDiscreteInputAddr = -1;
    int maxHoldingRegAddr = -1;
    int maxInputRegAddr = -1;
    
    for (const auto& [pointId, config] : channelConfig.points) {
        if (std::holds_alternative<ModbusPointConfig>(config.pointConfig_)) {
            const auto& modbus = std::get<ModbusPointConfig>(config.pointConfig_);
            
            switch (config.pointType_) {
                case PointType::DI:
                    if (modbus.functionCode == static_cast<int>(ModbusFunctionCode::READ_COILS)) {
                        maxCoilAddr = std::max(maxCoilAddr, modbus.address);
                    } else if (modbus.functionCode == static_cast<int>(ModbusFunctionCode::READ_DISCRETE_INPUTS)) {
                        maxDiscreteInputAddr = std::max(maxDiscreteInputAddr, modbus.address);
                    }
                    break;
                    
                case PointType::DO:
                    maxCoilAddr = std::max(maxCoilAddr, modbus.address);
                    break;
                    
                case PointType::AI:
                    if (modbus.functionCode == static_cast<int>(ModbusFunctionCode::READ_INPUT_REGISTERS)) {
                        maxInputRegAddr = std::max(maxInputRegAddr, modbus.address);
                    } else if (modbus.functionCode == static_cast<int>(ModbusFunctionCode::READ_HOLDING_REGISTERS)) {
                        maxHoldingRegAddr = std::max(maxHoldingRegAddr, modbus.address);
                    }
                    break;
                    
                case PointType::AO:
                    maxHoldingRegAddr = std::max(maxHoldingRegAddr, modbus.address);
                    break;
                    
                default:
                    break;
            }
        }
    }
    
    // Calculate sizes for Modbus mapping
    int nbCoils = maxCoilAddr >= 0 ? maxCoilAddr + 1 : 0;
    int nbDiscreteInputs = maxDiscreteInputAddr >= 0 ? maxDiscreteInputAddr + 1 : 0;
    int nbHoldingRegisters = maxHoldingRegAddr >= 0 ? maxHoldingRegAddr + 1 : 0;
    int nbInputRegisters = maxInputRegAddr >= 0 ? maxInputRegAddr + 1 : 0;
    
    // Set up Modbus mapping
    if (!setupModbusMapping(nbCoils, nbDiscreteInputs, nbHoldingRegisters, nbInputRegisters)) {
        logger_.logError("modbus", "Failed to set up Modbus mapping", 
                       "{\"error\":\"" + lastError_ + "\"}");
        return;
    }
    
    // Initialize register values from data points
    for (const auto& [pointId, config] : channelConfig.points) {
        if (std::holds_alternative<ModbusPointConfig>(config.pointConfig_)) {
            const auto& modbus = std::get<ModbusPointConfig>(config.pointConfig_);
            
            // Get the current value of the point from Redis
            DataPointValue value = getDataFromRedis(pointId);
            
            switch (config.pointType_) {
                case PointType::DI:
                    if (modbus.functionCode == static_cast<int>(ModbusFunctionCode::READ_COILS)) {
                        setCoil(modbus.address, value.value != 0);
                    } else if (modbus.functionCode == static_cast<int>(ModbusFunctionCode::READ_DISCRETE_INPUTS)) {
                        setDiscreteInput(modbus.address, value.value != 0);
                    }
                    break;
                    
                case PointType::DO:
                    setCoil(modbus.address, value.value != 0);
                    break;
                    
                case PointType::AI:
                    if (modbus.functionCode == static_cast<int>(ModbusFunctionCode::READ_INPUT_REGISTERS)) {
                        setInputRegister(modbus.address, static_cast<uint16_t>(value.value));
                    } else if (modbus.functionCode == static_cast<int>(ModbusFunctionCode::READ_HOLDING_REGISTERS)) {
                        setHoldingRegister(modbus.address, static_cast<uint16_t>(value.value));
                    }
                    break;
                    
                case PointType::AO:
                    setHoldingRegister(modbus.address, static_cast<uint16_t>(value.value));
                    break;
                    
                default:
                    break;
            }
        }
    }
    
    // Set up callbacks for updating Redis on register changes
    setHoldingRegisterCallback([this, &channelConfig](int address, uint16_t value) {
        // Find the point that corresponds to this register
        for (const auto& [pointId, config] : channelConfig.points) {
            if (std::holds_alternative<ModbusPointConfig>(config.pointConfig_)) {
                const auto& modbus = std::get<ModbusPointConfig>(config.pointConfig_);
                
                if ((config.pointType_ == PointType::AO || config.pointType_ == PointType::AI) && 
                    modbus.functionCode == static_cast<int>(ModbusFunctionCode::READ_HOLDING_REGISTERS) &&
                    modbus.address == address) {
                    
                    // Update Redis with the new value
                    DataPointValue pointValue;
                    pointValue.id = pointId;
                    pointValue.value = static_cast<double>(value);
                    pointValue.timestamp = std::chrono::system_clock::now();
                    pointValue.isValid = true;
                    
                    writeDataToRedis(pointValue);
                    break;
                }
            }
        }
    });
    
    setCoilCallback([this, &channelConfig](int address, bool value) {
        // Find the point that corresponds to this coil
        for (const auto& [pointId, config] : channelConfig.points) {
            if (std::holds_alternative<ModbusPointConfig>(config.pointConfig_)) {
                const auto& modbus = std::get<ModbusPointConfig>(config.pointConfig_);
                
                if ((config.pointType_ == PointType::DO || config.pointType_ == PointType::DI) && 
                    modbus.functionCode == static_cast<int>(ModbusFunctionCode::READ_COILS) &&
                    modbus.address == address) {
                    
                    // Update Redis with the new value
                    DataPointValue pointValue;
                    pointValue.id = pointId;
                    pointValue.value = value ? 1.0 : 0.0;
                    pointValue.timestamp = std::chrono::system_clock::now();
                    pointValue.isValid = true;
                    
                    writeDataToRedis(pointValue);
                    break;
                }
            }
        }
    });
    
    // Main loop - for a slave device, we just need to keep the thread alive
    // Actual request handling is done in the listening thread
    while (channelRunning_[channelIndex]) {
        try {
            // First check if we're connected
            if (!connected_) {
                logger_.logWarning("modbus", "Slave not connected, waiting...", 
                                 "{\"index\":" + std::to_string(channelIndex) + "}");
                
                // Sleep before retry
                std::this_thread::sleep_for(std::chrono::seconds(1));
                continue;
            }
            
            // Sleep to avoid high CPU usage
            std::this_thread::sleep_for(std::chrono::milliseconds(100));
            
        } catch (const std::exception& e) {
            logger_.logError("modbus", "Exception in slave channel thread", 
                           "{\"index\":" + std::to_string(channelIndex) + 
                           ",\"error\":\"" + e.what() + "\"}");
            
            // Sleep longer after error
            std::this_thread::sleep_for(std::chrono::seconds(1));
        }
    }
    
    logger_.logDebug("modbus", "Slave channel thread exiting", 
                    "{\"index\":" + std::to_string(channelIndex) + "}");
}

// Factory function implementation
std::unique_ptr<ModbusSlave> createModbusSlave(PhysicalInterfaceType interfaceType) {
    std::unique_ptr<ModbusSlave> slave = nullptr;
    
    if (interfaceType == PhysicalInterfaceType::NETWORK) {
        // Create a TCP slave
        slave = std::make_unique<ModbusTCPSlave>();
    } else if (interfaceType == PhysicalInterfaceType::SERIAL) {
        // Create an RTU slave
        slave = std::make_unique<ModbusRTUSlave>();
    } else {
        // Unsupported interface type
        return nullptr;
    }
    
    return slave;
} 