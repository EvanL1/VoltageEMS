#include "protocols/modbus/modbusTCPMaster.h"
#include <iostream>
#include <json/json.h>

ModbusTCPMaster::ModbusTCPMaster()
    : port_(502), unitId_(255) {
    // Set physical interface type
    setPhysicalInterfaceType(PhysicalInterfaceType::NETWORK);
}

ModbusTCPMaster::~ModbusTCPMaster() {
    // Ensure connection is properly closed
    disconnect();
}

bool ModbusTCPMaster::init(const std::string& config) {
    try {
        // Parse JSON configuration
        Json::Value root;
        Json::Reader reader;
        
        if (!reader.parse(config, root)) {
            lastError_ = "Failed to parse configuration: " + reader.getFormattedErrorMessages();
            logger_.logError("modbus_tcp", "Parse error", lastError_);
            return false;
        }
        
        // Extract TCP configuration
        if (root.isMember("host") && root["host"].isString()) {
            ipAddress_ = root["host"].asString();
        } else {
            lastError_ = "Host is required";
            logger_.logError("modbus_tcp", "Missing host", lastError_);
            return false;
        }
        
        if (root.isMember("port") && root["port"].isInt()) {
            port_ = root["port"].asInt();
        } else {
            port_ = 502;  // Default Modbus TCP port
        }
        
        if (root.isMember("slaveId") && root["slaveId"].isInt()) {
            unitId_ = root["slaveId"].asInt();
            setSlaveId(unitId_);
        } else {
            unitId_ = 255;  // Default unit ID for TCP
            setSlaveId(unitId_);
        }
        
        if (root.isMember("timeout") && root["timeout"].isInt()) {
            setTimeout(root["timeout"].asInt());
        } else {
            setTimeout(1000);  // Default timeout
        }
        
        if (root.isMember("debug") && root["debug"].isBool()) {
            setDebug(root["debug"].asBool());
        }
        
        if (root.isMember("maxRead") && root["maxRead"].isInt()) {
            maxRead_ = root["maxRead"].asInt();
        }
        
        // Connect to the Modbus TCP device
        return connect(ipAddress_, port_, unitId_);
        
    } catch (const std::exception& e) {
        lastError_ = "Exception during initialization: " + std::string(e.what());
        logger_.logError("modbus_tcp", "Init exception", lastError_);
        return false;
    }
}

bool ModbusTCPMaster::start() {
    if (running_) {
        return true;  // Already running
    }
    
    // Ensure we're connected
    if (!connected_ && !connect(ipAddress_, port_, unitId_)) {
        return false;
    }
    
    return ComBase::start();
}

bool ModbusTCPMaster::stop() {
    if (!running_) {
        return true;  // Already stopped
    }
    
    bool result = ComBase::stop();
    
    // Optionally disconnect here if needed
    // disconnect();
    
    return result;
}

bool ModbusTCPMaster::isRunning() const {
    return running_ && connected_;
}

std::string ModbusTCPMaster::getStatus() const {
    std::stringstream ss;
    ss << "Modbus TCP Master: "
       << (connected_ ? "Connected" : "Disconnected")
       << ", IP: " << ipAddress_
       << ", Port: " << port_ 
       << ", Unit ID: " << unitId_
       << ", Running: " << (running_ ? "Yes" : "No");
    return ss.str();
}

std::string ModbusTCPMaster::getStatistics() const {
    std::stringstream ss;
    ss << "Messages Received: " << messagesReceived_
       << ", Messages Sent: " << messagesSent_
       << ", Bytes Received: " << bytesReceived_
       << ", Bytes Sent: " << bytesSent_
       << ", Errors: " << errorCount_;
    return ss.str();
}

bool ModbusTCPMaster::connect(const std::string& host, int port, int unitId) {
    // Close any existing connection
    disconnect();
    
    // Store connection parameters
    ipAddress_ = host;
    port_ = port;
    unitId_ = unitId;
    
    // Create Modbus TCP context
    ctx_ = modbus_new_tcp(ipAddress_.c_str(), port_);
    
    if (ctx_ == nullptr) {
        lastError_ = "Failed to create Modbus TCP context";
        logger_.logError("modbus_tcp", "Context creation failed", lastError_);
        return false;
    }
    
    // Configure the context
    if (modbus_set_slave(ctx_, unitId_) == -1) {
        lastError_ = modbus_strerror(errno);
        logger_.logError("modbus_tcp", "Failed to set unit ID", 
                       "{\"unitId\":" + std::to_string(unitId_) + 
                       ",\"error\":\"" + lastError_ + "\"}");
        modbus_free(ctx_);
        ctx_ = nullptr;
        return false;
    }
    
    // Set debug mode
    modbus_set_debug(ctx_, debug_ ? TRUE : FALSE);
    
    // Set response timeout
    struct timeval tv;
    tv.tv_sec = timeout_ / 1000;
    tv.tv_usec = (timeout_ % 1000) * 1000;
    modbus_set_response_timeout(ctx_, tv.tv_sec, tv.tv_usec);
    
    // Set up callbacks for logging
    modbus_set_pre_send_callback(ctx_, modbusPreSendCallback);
    modbus_set_post_recv_callback(ctx_, modbusPostRecvCallback);
    
    // Connect to the device
    if (modbus_connect(ctx_) == -1) {
        lastError_ = modbus_strerror(errno);
        logger_.logError("modbus_tcp", "Connect failed", 
                       "{\"host\":\"" + ipAddress_ + "\",\"port\":" + 
                       std::to_string(port_) + ",\"error\":\"" + lastError_ + "\"}");
        modbus_free(ctx_);
        ctx_ = nullptr;
        return false;
    }
    
    connected_ = true;
    logger_.logInfo("modbus_tcp", "TCP master connected", 
                  "{\"host\":\"" + ipAddress_ + "\",\"port\":" + 
                  std::to_string(port_) + ",\"unitId\":" + std::to_string(unitId_) + "}");
    
    return true;
}

bool ModbusTCPMaster::disconnect() {
    if (ctx_) {
        modbus_close(ctx_);
        modbus_free(ctx_);
        ctx_ = nullptr;
    }
    
    connected_ = false;
    logger_.logInfo("modbus_tcp", "TCP master disconnected", "{}");
    
    return true;
}

bool ModbusTCPMaster::setIPAddress(const std::string& ip) {
    if (connected_) {
        lastError_ = "Cannot change IP address while connected";
        logger_.logError("modbus_tcp", "Change IP failed", lastError_);
        return false;
    }
    
    ipAddress_ = ip;
    return true;
}

bool ModbusTCPMaster::setPort(int port) {
    if (connected_) {
        lastError_ = "Cannot change port while connected";
        logger_.logError("modbus_tcp", "Change port failed", lastError_);
        return false;
    }
    
    if (port <= 0 || port > 65535) {
        lastError_ = "Invalid port number";
        logger_.logError("modbus_tcp", "Invalid port", 
                       "{\"port\":" + std::to_string(port) + "}");
        return false;
    }
    
    port_ = port;
    return true;
}

bool ModbusTCPMaster::setUnitId(int unitId) {
    if (unitId < 0 || unitId > 255) {
        lastError_ = "Invalid unit ID (valid range: 0-255)";
        logger_.logError("modbus_tcp", "Invalid unit ID", 
                       "{\"unitId\":" + std::to_string(unitId) + "}");
        return false;
    }
    
    // For TCP, unit ID can be changed even while connected
    unitId_ = unitId;
    
    if (ctx_) {
        if (modbus_set_slave(ctx_, unitId) == -1) {
            lastError_ = modbus_strerror(errno);
            logger_.logError("modbus_tcp", "Failed to set unit ID", 
                           "{\"unitId\":" + std::to_string(unitId) + 
                           ",\"error\":\"" + lastError_ + "\"}");
            return false;
        }
    }
    
    return true;
} 