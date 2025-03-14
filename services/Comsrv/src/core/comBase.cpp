#include "comBase.h"
#include <iostream>
#include <chrono>
#include <iomanip>
#include <sstream>
#include <cstring>
#include <mutex>
#include <thread>

namespace Communication {

ComBase::ComBase() {}

ComBase::~ComBase() {
    // Stop all running channel threads
    stop();
    
    disconnectFromRedis();
}

bool ComBase::connectToRedis(const std::string& host, int port) {
    redisCtx_ = redisConnect(host.c_str(), port);
    if (redisCtx_ == nullptr || redisCtx_->err) {
        if (redisCtx_) {
            logger_.logError("Redis connection error: " + std::string(redisCtx_->errstr));
            redisFree(redisCtx_);
            redisCtx_ = nullptr;
        } else {
            logger_.logError("redis", "Redis connection error", "Can't allocate redis context");
        }
        return false;
    }
    return true;
}

bool ComBase::writeToRedis(const std::string& key, const std::string& value) {
    if (!redisCtx_) {
        logger_.logError("redis", "Redis context is not connected");
        return false;
    }
    
    redisReply* reply = (redisReply*)redisCommand(redisCtx_, "SET %s %s", 
                                                 key.c_str(), value.c_str());
    if (!reply) {
        logger_.logError("Redis command error: " + std::string(redisCtx_->errstr));
        return false;
    }
    
    freeReplyObject(reply);
    return true;
}

bool ComBase::disconnectFromRedis() {
    if (redisCtx_) {
        redisFree(redisCtx_);
        redisCtx_ = nullptr;
    }
    return true;
}

void ComBase::addDataPoint(const std::string& id, const DataPointConfig& config) {
    dataPoints_[id] = config;
}

void ComBase::removeDataPoint(const std::string& id) {
    dataPoints_.erase(id);
}

bool ComBase::readDI(const std::string& id, DIValue& value) {
    auto it = dataPoints_.find(id);
    if (it == dataPoints_.end() || it->second.pointType_ != PointType::DI) {
        logger_.logError("point", "Invalid DI point", "{\"id\":\"" + id + "\"}");
        return false;
    }
    
    value.id = id;
    value.timestamp = getCurrentTimestamp();
    value.isValid = false;
    value.state = DIState::INVALID;
    value.quality = "Not implemented";
    
    return true;
}

bool ComBase::readAllDI(std::vector<DIValue>& values) {
    values.clear();
    for (const auto& point : dataPoints_) {
        if (point.second.pointType_ == PointType::DI) {
            DIValue value;
            if (readDI(point.first, value)) {
                values.push_back(value);
            }
        }
    }
    return !values.empty();
}

bool ComBase::readAI(const std::string& id, AIValue& value) {
    auto it = dataPoints_.find(id);
    if (it == dataPoints_.end() || it->second.pointType_ != PointType::AI) {
        logger_.logError("Invalid AI point: " + id);
        return false;
    }
    
    value.id = id;
    value.timestamp = getCurrentTimestamp();
    value.isValid = false;
    value.value = 0.0;
    value.unit = it->second.unit;
    value.quality = "Not implemented";
    
    return true;
}

bool ComBase::readAllAI(std::vector<AIValue>& values) {
    values.clear();
    for (const auto& point : dataPoints_) {
        if (point.second.pointType_ == PointType::AI) {
            AIValue value;
            if (readAI(point.first, value)) {
                values.push_back(value);
            }
        }
    }
    return !values.empty();
}

bool ComBase::executeDO(const std::string& id, DOCommand command) {
    auto it = dataPoints_.find(id);
    if (it == dataPoints_.end() || it->second.pointType_ != PointType::DO) {
        logger_.logError("point", "Invalid DO point", "{\"id\":\"" + id + "\"}");
        return false;
    }

    if (!validateDOCommand(id, command)) {
        return false;
    }

    logger_.logInfo("command", "Executing DO command", 
                   "{\"id\":\"" + id + "\",\"command\":" + 
                   std::to_string(static_cast<int>(command.command)) + "}");

    if (doCallback_) {
        doCallback_(id, true);
    }

    return true;
}

bool ComBase::cancelDO(const std::string& id) {
    auto it = dataPoints_.find(id);
    if (it == dataPoints_.end() || it->second.pointType_ != PointType::DO) {
        logger_.logError("Invalid DO point: " + id);
        return false;
    }
    
    DOCommand cancelCmd;
    cancelCmd.id = id;
    cancelCmd.command = DOState::CANCEL;
    cancelCmd.timestamp = getCurrentTimestamp();
    cancelCmd.needConfirm = false;
    cancelCmd.timeout = 0;
    
    return executeDO(id, cancelCmd);
}

bool ComBase::executeAO(const std::string& id, double value) {
    auto it = dataPoints_.find(id);
    if (it == dataPoints_.end() || it->second.pointType_ != PointType::AO) {
        logger_.logError("Invalid AO point: " + id);
        return false;
    }
    
    if (!validateAOValue(id, value)) {
        return false;
    }
    
    // 记录命令执行
    logger_.logInfo("Executing AO command: " + id + ", value: " + 
                   std::to_string(value));
    
    if (aoCallback_) {
        aoCallback_(id, true);
    }
    
    return true;
}

bool ComBase::cancelAO(const std::string& id) {
    auto it = dataPoints_.find(id);
    if (it == dataPoints_.end() || it->second.pointType_ != PointType::AO) {
        logger_.logError("Invalid AO point: " + id);
        return false;
    }
    
    return executeAO(id, 0.0);
}

bool ComBase::createChannel(const ChannelConfig& config) {
    if (channels_.find(config.index) != channels_.end()) {
        logger_.logError("channel", "Channel already exists", 
                        "{\"index\":" + std::to_string(config.index) + "}");
        return false;
    }
    
    channels_[config.index] = config;
    logger_.logInfo("channel", "Channel created", 
                   "{\"index\":" + std::to_string(config.index) + 
                   ",\"name\":\"" + config.name + "\"}");
    return true;
}

bool ComBase::removeChannel(int index) {
    return channels_.erase(index) > 0;
}

bool ComBase::isChannelActive(int index) const {
    auto it = channels_.find(index);
    return it != channels_.end() && running_;
}

std::string ComBase::getChannelStatus(int index) const {
    auto it = channels_.find(index);
    if (it == channels_.end()) {
        return "Channel not found";
    }
    
    std::stringstream ss;
    ss << "Channel " << index << ": "
       << (isChannelActive(index) ? "Active" : "Inactive")
       << ", Protocol: " << static_cast<int>(it->second.protocolType)
       << ", Points: " << it->second.points.size();
    
    return ss.str();
}

std::vector<PointTableItem> ComBase::getChannelPoints(int index) const {
    std::vector<PointTableItem> result;
    auto it = channels_.find(index);
    if (it == channels_.end()) {
        return result;
    }
    
    for (const auto& point : it->second.points) {
        PointTableItem item;
        item.id = point.first;
        item.type = point.second.pointType_;
        item.dataType = point.second.datatype_;
        item.byteOrder = point.second.byteOrder_;
        item.description = point.second.description;
        
        // Get address from specific protocol configuration
        if (std::holds_alternative<ModbusPointConfig>(point.second.pointConfig_)) {
            item.address = std::get<ModbusPointConfig>(point.second.pointConfig_).address;
        }
        
        result.push_back(item);
    }
    
    return result;
}

std::string ComBase::getCurrentTimestamp() {
    auto now = std::chrono::system_clock::now();
    auto now_c = std::chrono::system_clock::to_time_t(now);
    auto now_ms = std::chrono::duration_cast<std::chrono::milliseconds>(
        now.time_since_epoch()) % 1000;
    
    std::stringstream ss;
    ss << std::put_time(std::localtime(&now_c), "%Y-%m-%d %H:%M:%S")
       << '.' << std::setfill('0') << std::setw(3) << now_ms.count();
    
    return ss.str();
}

bool ComBase::validateChannel(int index) const {
    return channels_.find(index) != channels_.end();
}

bool ComBase::validatePoint(int channelIndex, const std::string& pointId) const {
    auto channelIt = channels_.find(channelIndex);
    if (channelIt == channels_.end()) {
        return false;
    }
    
    return channelIt->second.points.find(pointId) != channelIt->second.points.end();
}

void ComBase::processChannelData(int channelIndex, const std::vector<uint8_t>& data) {
    auto channelIt = channels_.find(channelIndex);
    if (channelIt == channels_.end()) {
        logger_.logError("Channel " + std::to_string(channelIndex) + " not found");
        return;
    }
    
    // TODO: Implement channel data processing
}

bool ComBase::writeChannelData(int channelIndex, const std::string& pointId,
                             const std::vector<uint8_t>& data) {
    if (!validateChannel(channelIndex) || !validatePoint(channelIndex, pointId)) {
        return false;
    }
    
    // TODO: Implement channel data writing
    return true;
}

DataPointValue ComBase::parseData(const std::string& id, const std::vector<uint16_t>& rawData) {
    DataPointValue result;
    result.id = id;
    result.isValid = false;
    result.timestamp = getCurrentTimestamp();

    auto it = dataPoints_.find(id);
    if (it == dataPoints_.end()) {
        return result;
    }

    const DataPointConfig& config = it->second;
    result.unit = config.unit;

    // 检查数据长度
    size_t requiredLength = (config.datatype_ == DataType::INT32 || 
                           config.datatype_ == DataType::UINT32 || 
                           config.datatype_ == DataType::FLOAT32) ? 2 : 1;
    if (rawData.size() < requiredLength) {
        return result;
    }

    // 解析数据
    double value = parseValue(rawData, config);

    // 应用系数和偏移
    value = value * config.scale + config.offset;

    // 验证数据
    result.isValid = validateValue(value, config);
    result.value = value;

    return result;
}

double ComBase::parseValue(const std::vector<uint16_t>& rawData, const DataPointConfig& config) {
    switch (config.datatype_) {
        case DataType::INT16:
            return static_cast<int16_t>(rawData[0]);
        
        case DataType::UINT16:
            return rawData[0];
        
        case DataType::INT32:
            return combine32Bit(rawData[0], rawData[1], config.byteOrder_);
        
        case DataType::UINT32:
            return static_cast<uint32_t>(combine32Bit(rawData[0], rawData[1], config.byteOrder_));
        
        case DataType::FLOAT32:
            return combineFloat(rawData[0], rawData[1], config.byteOrder_);
        
        case DataType::BOOL:
            return (rawData[0] != 0) ? 1.0 : 0.0;
        
        default:
            return 0.0;
    }
}

int32_t ComBase::combine32Bit(uint16_t high, uint16_t low, ByteOrder order) {
    uint32_t result = 0;
    
    switch (order) {
        case ByteOrder::ABCD:
            result = (static_cast<uint32_t>(high) << 16) | low;
            break;
        
        case ByteOrder::CDAB:
            result = (static_cast<uint32_t>(low) << 16) | high;
            break;
        
        case ByteOrder::BADC:
            result = (static_cast<uint32_t>((high >> 8) | (high << 8)) << 16) |
                    (low >> 8) | (low << 8);
            break;
        
        case ByteOrder::DCBA:
            result = (static_cast<uint32_t>(low & 0xFF) << 24) |
                    (static_cast<uint32_t>(low & 0xFF00) << 8) |
                    (static_cast<uint32_t>(high & 0xFF) << 8) |
                    (static_cast<uint32_t>(high & 0xFF00) >> 8);
            break;
        
        default:
            result = (static_cast<uint32_t>(high) << 16) | low;
    }
    
    return static_cast<int32_t>(result);
}

float ComBase::combineFloat(uint16_t high, uint16_t low, ByteOrder order) {
    uint32_t combined = combine32Bit(high, low, order);
    float result;
    std::memcpy(&result, &combined, sizeof(float));
    return result;
}

bool ComBase::validateValue(double value, const DataPointConfig& config) {
    if (!config.isValid) {
        return false;
    }
    
    // Check if value is within range limits when min and max are different
    if (config.min != config.max) {
        if (value < config.min || value > config.max) {
            return false;
        }
    }
    
    return true;
}

bool ComBase::writeDataToRedis(const DataPointValue& value) {
    if (!redisCtx_) {
        return false;
    }

    // Create JSON formatted data
    std::stringstream ss;
    ss << "{\"value\":" << value.value
       << ",\"unit\":\"" << value.unit
       << "\",\"timestamp\":\"" << value.timestamp
       << "\",\"valid\":" << (value.isValid ? "true" : "false")
       << "}";

    std::string key = "data:" + value.id;
    return writeToRedis(key, ss.str());
}

std::vector<DataPointConfig> ComBase::getDataPointsByType(PointType type) const {
    std::vector<DataPointConfig> result;
    for (const auto& pair : dataPoints_) {
        if (pair.second.pointType_ == type) {
            result.push_back(pair.second);
        }
    }
    return result;
}

void ComBase::processDIData(const std::string& id, const std::vector<uint16_t>& data) {
    DIValue value;
    value.id = id;
    value.timestamp = getCurrentTimestamp();
    
    if (data.empty()) {
        value.state = DIState::INVALID;
        value.isValid = false;
        value.quality = "Data not available";
        logger_.logWarning("point", "DI data not available", 
                          "{\"id\":\"" + id + "\"}");
    } else {
        value.state = data[0] ? DIState::ON : DIState::OFF;
        value.isValid = true;
        value.quality = "Good";
        logger_.logDebug("point", "DI state updated", 
                        "{\"id\":\"" + id + "\",\"state\":" + 
                        std::to_string(static_cast<int>(value.state)) + "}");
    }

    if (diCallback_) {
        diCallback_(value);
    }
}

void ComBase::processAIData(const std::string& id, const std::vector<uint16_t>& data) {
    auto it = dataPoints_.find(id);
    if (it == dataPoints_.end()) {
        logger_.logError("point", "AI point not found", 
                        "{\"id\":\"" + id + "\"}");
        return;
    }

    const DataPointConfig& config = it->second;
    AIValue value;
    value.id = id;
    value.unit = config.unit;
    value.timestamp = getCurrentTimestamp();

    if (data.empty()) {
        value.isValid = false;
        value.value = 0.0;
        value.quality = "Data not available";
        logger_.logWarning("point", "AI data not available", 
                          "{\"id\":\"" + id + "\"}");
    } else {
        DataPointValue parsed = parseData(id, data);
        value.value = parsed.value;
        value.isValid = parsed.isValid;
        value.quality = value.isValid ? "Good" : "Out of range";
        
        if (value.isValid) {
            logger_.logDebug("point", "AI value updated", 
                           "{\"id\":\"" + id + "\",\"value\":" + 
                           std::to_string(value.value) + ",\"unit\":\"" + 
                           value.unit + "\"}");
        } else {
            logger_.logWarning("point", "AI value out of range", 
                             "{\"id\":\"" + id + "\",\"value\":" + 
                             std::to_string(value.value) + ",\"unit\":\"" + 
                             value.unit + "\"}");
        }
    }

    if (aiCallback_) {
        aiCallback_(value);
    }
}

bool ComBase::validateDOCommand(const std::string& id, DOCommand command) {
    auto it = dataPoints_.find(id);
    if (it == dataPoints_.end()) {
        logger_.logError(id, "DO point not found");
        return false;
    }

    switch (command.command) {
        case DOState::OPEN:
        case DOState::CLOSE:
        case DOState::CANCEL:
            return true;
        default:
            logger_.logError(id, "Invalid DO command");
            return false;
    }
}

bool ComBase::validateAOValue(const std::string& id, double value) {
    auto it = dataPoints_.find(id);
    if (it == dataPoints_.end()) {
        logger_.logError(id, "AO point not found");
        return false;
    }

    return validateValue(value, it->second);
}

std::string ComBase::formatRedisKey(const std::string& id, PointType type) {
    std::string prefix;
    switch (type) {
        case PointType::DI: prefix = "di:"; break;
        case PointType::AI: prefix = "ai:"; break;
        case PointType::DO: prefix = "do:"; break;
        case PointType::AO: prefix = "ao:"; break;
    }
    return prefix + id;
}

bool ComBase::startChannel(int channelIndex) {
    std::lock_guard<std::mutex> lock(channelsMutex_);
    
    // Check if channel exists
    auto it = channels_.find(channelIndex);
    if (it == channels_.end()) {
        logger_.logError("channel", "Channel not found", 
                       "{\"index\":" + std::to_string(channelIndex) + "}");
        return false;
    }
    
    // Check if channel is already running
    auto runIt = channelRunning_.find(channelIndex);
    if (runIt != channelRunning_.end() && runIt->second) {
        // Channel is already running
        logger_.logInfo("channel", "Channel already running", 
                      "{\"index\":" + std::to_string(channelIndex) + "}");
        return true;
    }
    
    // Set running flag and start thread
    channelRunning_[channelIndex] = true;
    
    try {
        // Remove old thread if exists
        auto threadIt = channelThreads_.find(channelIndex);
        if (threadIt != channelThreads_.end() && threadIt->second.joinable()) {
            // This should not happen if everything is working properly
            logger_.logWarning("channel", "Existing thread being joined", 
                             "{\"index\":" + std::to_string(channelIndex) + "}");
            threadIt->second.join();
            channelThreads_.erase(threadIt);
        }
        
        // Create new thread
        channelThreads_[channelIndex] = std::thread(&ComBase::channelThreadFunc, this, channelIndex);
        
        logger_.logInfo("channel", "Channel thread started", 
                       "{\"index\":" + std::to_string(channelIndex) + 
                       ",\"name\":\"" + it->second.name + "\"}");
        return true;
    } catch (const std::exception& e) {
        logger_.logError("channel", "Failed to start channel thread", 
                        "{\"index\":" + std::to_string(channelIndex) + 
                        ",\"error\":\"" + e.what() + "\"}");
        channelRunning_[channelIndex] = false;
        return false;
    }
}

bool ComBase::stopChannel(int channelIndex) {
    std::unique_lock<std::mutex> lock(channelsMutex_);
    
    // Check if channel exists and is running
    auto runIt = channelRunning_.find(channelIndex);
    if (runIt == channelRunning_.end() || !runIt->second) {
        // Channel is not running
        logger_.logInfo("channel", "Channel not running", 
                      "{\"index\":" + std::to_string(channelIndex) + "}");
        return true;
    }
    
    // Set running flag to false to stop thread
    runIt->second = false;
    
    // Get thread handle
    auto threadIt = channelThreads_.find(channelIndex);
    if (threadIt != channelThreads_.end() && threadIt->second.joinable()) {
        // Release lock during join to avoid deadlock
        lock.unlock();
        
        // Wait for thread to finish
        threadIt->second.join();
        
        // Reacquire lock and erase thread
        lock.lock();
        channelThreads_.erase(threadIt);
        
        logger_.logInfo("channel", "Channel thread stopped", 
                       "{\"index\":" + std::to_string(channelIndex) + "}");
        return true;
    }
    
    return true;
}

bool ComBase::isChannelRunning(int channelIndex) const {
    std::lock_guard<std::mutex> lock(channelsMutex_);
    
    auto runIt = channelRunning_.find(channelIndex);
    return (runIt != channelRunning_.end() && runIt->second);
}

void ComBase::channelThreadFunc(int channelIndex) {
    logger_.logDebug("channel", "Channel thread started", 
                    "{\"index\":" + std::to_string(channelIndex) + "}");
    
    // Get channel config
    ChannelConfig channelConfig;
    {
        std::lock_guard<std::mutex> lock(channelsMutex_);
        auto it = channels_.find(channelIndex);
        if (it == channels_.end()) {
            logger_.logError("channel", "Channel not found in thread", 
                           "{\"index\":" + std::to_string(channelIndex) + "}");
            return;
        }
        channelConfig = it->second;
    }
    
    // Thread main loop
    while (channelRunning_[channelIndex]) {
        try {
            // Read data from device
            // This is a placeholder - actual implementation depends on protocol
            
            // Process channel data
            std::vector<uint8_t> dummyData;  // Replace with actual data
            processChannelData(channelIndex, dummyData);
            
            // Sleep to avoid high CPU usage
            std::this_thread::sleep_for(std::chrono::milliseconds(100));
        } catch (const std::exception& e) {
            logger_.logError("channel", "Exception in channel thread", 
                           "{\"index\":" + std::to_string(channelIndex) + 
                           ",\"error\":\"" + e.what() + "\"}");
            
            // Sleep longer after error
            std::this_thread::sleep_for(std::chrono::seconds(1));
        }
    }
    
    logger_.logDebug("channel", "Channel thread exiting", 
                    "{\"index\":" + std::to_string(channelIndex) + "}");
}

bool ComBase::start() {
    if (running_) {
        return true; // Already running
    }
    
    running_ = true;
    
    // Start all channels
    std::lock_guard<std::mutex> lock(channelsMutex_);
    bool allStarted = true;
    
    for (const auto& channel : channels_) {
        if (!startChannel(channel.first)) {
            allStarted = false;
            logger_.logError("channel", "Failed to start channel", 
                           "{\"index\":" + std::to_string(channel.first) + "}");
        }
    }
    
    return allStarted;
}

bool ComBase::stop() {
    if (!running_) {
        return true; // Already stopped
    }
    
    running_ = false;
    
    // Stop all channels
    std::vector<int> channelIndices;
    {
        std::lock_guard<std::mutex> lock(channelsMutex_);
        for (const auto& channel : channels_) {
            channelIndices.push_back(channel.first);
        }
    }
    
    bool allStopped = true;
    for (int index : channelIndices) {
        if (!stopChannel(index)) {
            allStopped = false;
            logger_.logError("channel", "Failed to stop channel", 
                           "{\"index\":" + std::to_string(index) + "}");
        }
    }
    
    return allStopped;
}

bool ComBase::isRunning() const {
    return running_;
}

bool ComBase::parsePointTable(const std::string& filename, 
                             PointType type, 
                             std::map<std::string, DataPointConfig>& points) {
    std::ifstream file(filename);
    if (!file.is_open()) {
        logger_.logError("file", "Failed to open point table", 
                        "{\"file\":\"" + filename + "\"}");
        return false;
    }

    std::string line;
    std::getline(file, line); // Skip header

    // Split column names to determine column positions
    std::vector<std::string> headers = splitCSV(line);
    int addrCol = -1, slaveIdCol = -1, nameCol = -1, dataTypeCol = -1;
    int funcCodeCol = -1, byteOrderCol = -1, scaleCol = -1, offsetCol = -1;
    int unitCol = -1, descCol = -1;
    
    // Find column positions
    for (size_t i = 0; i < headers.size(); i++) {
        std::string header = toLower(trim(headers[i]));
        if (header == "address") addrCol = i;
        else if (header == "slaveid") slaveIdCol = i;
        else if (header == "name") nameCol = i;
        else if (header == "datatype") dataTypeCol = i;
        else if (header == "functioncode") funcCodeCol = i;
        else if (header == "byteorder") byteOrderCol = i;
        else if (header == "scale" || header == "coefficiency") scaleCol = i;
        else if (header == "offset") offsetCol = i;
        else if (header == "unit") unitCol = i;
        else if (header == "description") descCol = i;
    }
    
    // Validate required columns exist
    if (addrCol == -1 || nameCol == -1 || dataTypeCol == -1) {
        logger_.logError("file", "Missing required columns in point table", 
                      "{\"file\":\"" + filename + "\"}");
        return false;
    }

    int lineNum = 1;
    while (std::getline(file, line)) {
        lineNum++;
        if (line.empty() || line[0] == '#') continue;
        
        std::vector<std::string> tokens = splitCSV(line);
        if (tokens.size() <= std::max({addrCol, nameCol, dataTypeCol})) {
            logger_.logWarning("file", "Invalid point table entry", 
                            "{\"file\":\"" + filename + "\",\"line\":" + 
                            std::to_string(lineNum) + "}");
            continue;
        }

        try {
            DataPointConfig pointConfig;
            ModbusPointConfig modbusConfig;
            
            // Set basic point information
            pointConfig.id = tokens[nameCol];
            pointConfig.pointType_ = type;
            pointConfig.datatype_ = stringToDataType(tokens[dataTypeCol]);
            
            // Set Modbus specific information
            modbusConfig.address = std::stoi(tokens[addrCol]);
            
            // Set Slave ID (if available)
            if (slaveIdCol != -1 && tokens.size() > slaveIdCol) {
                modbusConfig.slaveId = std::stoi(tokens[slaveIdCol]);
            } else {
                modbusConfig.slaveId = 1; // Default Slave ID
            }
            
            // Set function code (if available)
            if (funcCodeCol != -1 && tokens.size() > funcCodeCol) {
                modbusConfig.functionCode = std::stoi(tokens[funcCodeCol]);
            } else {
                // Set default function code based on point type
                switch (type) {
                    case PointType::DI: modbusConfig.functionCode = 2; break; // Read Discrete Inputs
                    case PointType::AI: modbusConfig.functionCode = 4; break; // Read Input Registers
                    case PointType::DO: modbusConfig.functionCode = 5; break; // Write Single Coil
                    case PointType::AO: modbusConfig.functionCode = 6; break; // Write Single Register
                    default: modbusConfig.functionCode = 3; break;           // Default: Read Holding Registers
                }
            }
            
            // Set data type and bit length (if available)
            modbusConfig.dataType = static_cast<int>(pointConfig.datatype_);
            modbusConfig.bitLength = getDataTypeSize(pointConfig.datatype_) * 8;
            
            // Set byte order (if available)
            if (byteOrderCol != -1 && tokens.size() > byteOrderCol) {
                pointConfig.byteOrder_ = stringToByteOrder(tokens[byteOrderCol]);
            } else {
                pointConfig.byteOrder_ = ByteOrder::AB; // Default byte order
            }
            
            // Set scale factor (if available)
            if (scaleCol != -1 && tokens.size() > scaleCol) {
                pointConfig.scale = std::stod(tokens[scaleCol]);
            }
            
            // Set offset (if available)
            if (offsetCol != -1 && tokens.size() > offsetCol) {
                pointConfig.offset = std::stod(tokens[offsetCol]);
            }
            
            // Set unit (if available)
            if (unitCol != -1 && tokens.size() > unitCol) {
                pointConfig.unit = tokens[unitCol];
            }
            
            // Set description (if available)
            if (descCol != -1 && tokens.size() > descCol) {
                pointConfig.description = tokens[descCol];
            } else {
                pointConfig.description = pointConfig.id;
            }
            
            // Set validity flag
            pointConfig.isValid = true;
            
            // Set Modbus point configuration
            pointConfig.pointConfig_ = modbusConfig;
            
            // Add to point mapping
            points[pointConfig.id] = pointConfig;
            
        } catch (const std::exception& e) {
            logger_.logError("file", "Error parsing point table entry", 
                          "{\"file\":\"" + filename + "\",\"line\":" + 
                          std::to_string(lineNum) + ",\"error\":\"" + e.what() + "\"}");
        }
    }
    
    logger_.logInfo("file", "Loaded point table", 
                  "{\"file\":\"" + filename + "\",\"count\":" + 
                  std::to_string(points.size()) + "}");
    
    return !points.empty();
}

bool ComBase::updateConfig(const std::string& config) {
    try {
        // Parse configuration JSON
        Json::Value root;
        Json::Reader reader;
        
        if (!reader.parse(config, root)) {
            logger_.logError("config", "Failed to parse configuration", 
                          "{\"error\":\"" + reader.getFormattedErrorMessages() + "\"}");
            return false;
        }
        
        // Update global configuration
        if (root.isMember("global")) {
            updateGlobalConfig(root["global"]);
        }
        
        // Update channel configurations
        if (root.isMember("channels") && root["channels"].isArray()) {
            for (const auto& channelJson : root["channels"]) {
                if (!channelJson.isMember("index")) continue;
                
                int index = channelJson["index"].asInt();
                ChannelConfig channelConfig;
                
                if (parseChannelConfig(channelJson, channelConfig)) {
                    updateChannel(channelConfig);
                }
            }
        }
        
        return true;
    } catch (const std::exception& e) {
        logger_.logError("config", "Exception during config update", 
                      "{\"error\":\"" + std::string(e.what()) + "\"}");
        return false;
    }
}

bool ComBase::updateChannel(const ChannelConfig& config) {
    std::lock_guard<std::mutex> lock(channelsMutex_);
    
    // Check if channel exists
    auto it = channels_.find(config.index);
    bool isNewChannel = (it == channels_.end());
    
    // If channel is running, stop it first
    bool wasRunning = isChannelRunning(config.index);
    if (wasRunning) {
        stopChannel(config.index);
    }
    
    // Update channel configuration
    channels_[config.index] = config;
    
    // If channel was running, restart it
    if (wasRunning) {
        startChannel(config.index);
    }
    
    logger_.logInfo("channel", isNewChannel ? "Channel created" : "Channel updated", 
                  "{\"index\":" + std::to_string(config.index) + 
                  ",\"name\":\"" + config.name + "\"}");
    
    return true;
}

bool ComBase::reloadPointTable(int channelIndex, PointType type, const std::string& filename) {
    std::lock_guard<std::mutex> lock(channelsMutex_);
    
    // Check if channel exists
    auto channelIt = channels_.find(channelIndex);
    if (channelIt == channels_.end()) {
        logger_.logError("channel", "Channel not found for point table reload", 
                      "{\"index\":" + std::to_string(channelIndex) + "}");
        return false;
    }
    
    // Create new point mapping
    std::map<std::string, DataPointConfig> newPoints;
    
    // Parse point table
    if (!parsePointTable(filename, type, newPoints)) {
        return false;
    }
    
    // Backup old point configuration
    auto& channelConfig = channelIt->second;
    auto oldPoints = channelConfig.points;
    
    // Remove all points of the specified type
    for (auto it = channelConfig.points.begin(); it != channelConfig.points.end();) {
        if (it->second.pointType_ == type) {
            it = channelConfig.points.erase(it);
        } else {
            ++it;
        }
    }
    
    // Add new points
    for (const auto& point : newPoints) {
        channelConfig.points[point.first] = point.second;
    }
    
    // Log the operation
    logger_.logInfo("channel", "Point table reloaded", 
                  "{\"index\":" + std::to_string(channelIndex) + 
                  ",\"type\":" + pointTypeToString(type) + 
                  ",\"count\":" + std::to_string(newPoints.size()) + "}");
    
    return true;
}

bool ComBase::reconfigureChannel(int channelIndex) {
    // Check if channel exists
    {
        std::lock_guard<std::mutex> lock(channelsMutex_);
        if (channels_.find(channelIndex) == channels_.end()) {
            logger_.logError("channel", "Channel not found for reconfiguration", 
                          "{\"index\":" + std::to_string(channelIndex) + "}");
            return false;
        }
    }
    
    // If channel is running, restart it
    bool wasRunning = isChannelRunning(channelIndex);
    if (wasRunning) {
        if (!stopChannel(channelIndex)) {
            logger_.logError("channel", "Failed to stop channel for reconfiguration", 
                          "{\"index\":" + std::to_string(channelIndex) + "}");
            return false;
        }
        
        if (!startChannel(channelIndex)) {
            logger_.logError("channel", "Failed to restart channel after reconfiguration", 
                          "{\"index\":" + std::to_string(channelIndex) + "}");
            return false;
        }
    }
    
    logger_.logInfo("channel", "Channel reconfigured", 
                  "{\"index\":" + std::to_string(channelIndex) + 
                  ",\"running\":" + (wasRunning ? "true" : "false") + "}");
    
    return true;
}

PhysicalInterfaceType ComBase::getPhysicalInterfaceType() const {
    return physicalInterface_;
}

void ComBase::setPhysicalInterfaceType(PhysicalInterfaceType type) {
    physicalInterface_ = type;
}

} // namespace Communication 