#include "protocols/modbus/modbusRTUMaster.h"
#include <iostream>
#include <json/json.h>

ModbusRTUMaster::ModbusRTUMaster()
    : baudRate_(9600), parity_('N'), dataBits_(8), stopBits_(1) {
    // Set physical interface type
    setPhysicalInterfaceType(PhysicalInterfaceType::SERIAL);
}

ModbusRTUMaster::~ModbusRTUMaster() {
    // Ensure connection is properly closed
    disconnect();
}

bool ModbusRTUMaster::init(const std::string& config) {
    try {
        // Parse JSON configuration
        Json::Value root;
        Json::Reader reader;
        
        if (!reader.parse(config, root)) {
            lastError_ = "Failed to parse configuration: " + reader.getFormattedErrorMessages();
            logger_.logError("modbus_rtu", "Parse error", lastError_);
            return false;
        }
        
        // Extract RTU configuration
        if (root.isMember("serialPort") && root["serialPort"].isString()) {
            serialPort_ = root["serialPort"].asString();
        } else {
            lastError_ = "Serial port is required";
            logger_.logError("modbus_rtu", "Missing serial port", lastError_);
            return false;
        }
        
        if (root.isMember("baudRate") && root["baudRate"].isInt()) {
            baudRate_ = root["baudRate"].asInt();
        } else {
            baudRate_ = 9600;  // Default baud rate
        }
        
        if (root.isMember("parity") && root["parity"].isString()) {
            parity_ = root["parity"].asString()[0];
        } else {
            parity_ = 'N';  // Default parity (None)
        }
        
        if (root.isMember("dataBits") && root["dataBits"].isInt()) {
            dataBits_ = root["dataBits"].asInt();
        } else {
            dataBits_ = 8;  // Default data bits
        }
        
        if (root.isMember("stopBits") && root["stopBits"].isInt()) {
            stopBits_ = root["stopBits"].asInt();
        } else {
            stopBits_ = 1;  // Default stop bits
        }
        
        if (root.isMember("slaveId") && root["slaveId"].isInt()) {
            setSlaveId(root["slaveId"].asInt());
        } else {
            setSlaveId(1);  // Default slave ID
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
        
        // Connect to the Modbus RTU device
        return connect(serialPort_, baudRate_, parity_, dataBits_, stopBits_);
        
    } catch (const std::exception& e) {
        lastError_ = "Exception during initialization: " + std::string(e.what());
        logger_.logError("modbus_rtu", "Init exception", lastError_);
        return false;
    }
}

bool ModbusRTUMaster::start() {
    if (running_) {
        return true;  // Already running
    }
    
    // Ensure we're connected
    if (!connected_ && !connect(serialPort_, baudRate_, parity_, dataBits_, stopBits_)) {
        return false;
    }
    
    return ComBase::start();
}

bool ModbusRTUMaster::stop() {
    if (!running_) {
        return true;  // Already stopped
    }
    
    bool result = ComBase::stop();
    
    // Optionally disconnect here if needed
    // disconnect();
    
    return result;
}

bool ModbusRTUMaster::isRunning() const {
    return running_ && connected_;
}

std::string ModbusRTUMaster::getStatus() const {
    std::stringstream ss;
    ss << "Modbus RTU Master: "
       << (connected_ ? "Connected" : "Disconnected")
       << ", Port: " << serialPort_
       << ", Settings: " << baudRate_ << "-" << dataBits_ << parity_ << stopBits_
       << ", Slave ID: " << slaveId_
       << ", Running: " << (running_ ? "Yes" : "No");
    return ss.str();
}

std::string ModbusRTUMaster::getStatistics() const {
    std::stringstream ss;
    ss << "Messages Received: " << messagesReceived_
       << ", Messages Sent: " << messagesSent_
       << ", Bytes Received: " << bytesReceived_
       << ", Bytes Sent: " << bytesSent_
       << ", Errors: " << errorCount_
       << ", CRC Errors: " << crcErrorCount_;
    return ss.str();
}

bool ModbusRTUMaster::connect(const std::string& serialPort, int baudRate, char parity, int dataBits, int stopBits) {
    // Close any existing connection
    disconnect();
    
    // Store connection parameters
    serialPort_ = serialPort;
    baudRate_ = baudRate;
    parity_ = parity;
    dataBits_ = dataBits;
    stopBits_ = stopBits;
    
    // Create Modbus RTU context
    ctx_ = modbus_new_rtu(
        serialPort_.c_str(),
        baudRate_,
        parity_,
        dataBits_,
        stopBits_
    );
    
    if (ctx_ == nullptr) {
        lastError_ = "Failed to create Modbus RTU context";
        logger_.logError("modbus_rtu", "Context creation failed", lastError_);
        return false;
    }
    
    // Configure the context
    if (modbus_set_slave(ctx_, slaveId_) == -1) {
        lastError_ = modbus_strerror(errno);
        logger_.logError("modbus_rtu", "Failed to set slave ID", 
                       "{\"slaveId\":" + std::to_string(slaveId_) + 
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
        logger_.logError("modbus_rtu", "Connect failed", 
                       "{\"port\":\"" + serialPort_ + "\",\"error\":\"" + lastError_ + "\"}");
        modbus_free(ctx_);
        ctx_ = nullptr;
        return false;
    }
    
    connected_ = true;
    logger_.logInfo("modbus_rtu", "RTU master connected", 
                  "{\"port\":\"" + serialPort_ + "\",\"baud\":" + std::to_string(baudRate_) + 
                  ",\"format\":\"" + std::to_string(dataBits_) + parity_ + std::to_string(stopBits_) + 
                  "\",\"slaveId\":" + std::to_string(slaveId_) + "}");
    
    return true;
}

bool ModbusRTUMaster::disconnect() {
    if (ctx_) {
        modbus_close(ctx_);
        modbus_free(ctx_);
        ctx_ = nullptr;
    }
    
    connected_ = false;
    logger_.logInfo("modbus_rtu", "RTU master disconnected", "{}");
    
    return true;
}

bool ModbusRTUMaster::setSerialPort(const std::string& serialPort) {
    if (connected_) {
        lastError_ = "Cannot change serial port while connected";
        logger_.logError("modbus_rtu", "Change port failed", lastError_);
        return false;
    }
    
    serialPort_ = serialPort;
    return true;
}

bool ModbusRTUMaster::setBaudRate(int baudRate) {
    if (connected_) {
        lastError_ = "Cannot change baud rate while connected";
        logger_.logError("modbus_rtu", "Change baud rate failed", lastError_);
        return false;
    }
    
    // Check for standard baud rates
    switch (baudRate) {
        case 1200:
        case 2400:
        case 4800:
        case 9600:
        case 19200:
        case 38400:
        case 57600:
        case 115200:
            baudRate_ = baudRate;
            return true;
        default:
            lastError_ = "Unsupported baud rate";
            logger_.logError("modbus_rtu", "Invalid baud rate", 
                           "{\"baud\":" + std::to_string(baudRate) + "}");
            return false;
    }
}

bool ModbusRTUMaster::setParity(char parity) {
    if (connected_) {
        lastError_ = "Cannot change parity while connected";
        logger_.logError("modbus_rtu", "Change parity failed", lastError_);
        return false;
    }
    
    // Check for valid parity
    switch (parity) {
        case 'N': // None
        case 'E': // Even
        case 'O': // Odd
            parity_ = parity;
            return true;
        default:
            lastError_ = "Invalid parity (valid: N, E, O)";
            logger_.logError("modbus_rtu", "Invalid parity", 
                           "{\"parity\":\"" + std::string(1, parity) + "\"}");
            return false;
    }
}

bool ModbusRTUMaster::setDataBits(int dataBits) {
    if (connected_) {
        lastError_ = "Cannot change data bits while connected";
        logger_.logError("modbus_rtu", "Change data bits failed", lastError_);
        return false;
    }
    
    // Check for valid data bits
    if (dataBits == 7 || dataBits == 8) {
        dataBits_ = dataBits;
        return true;
    } else {
        lastError_ = "Invalid data bits (valid: 7, 8)";
        logger_.logError("modbus_rtu", "Invalid data bits", 
                       "{\"bits\":" + std::to_string(dataBits) + "}");
        return false;
    }
}

bool ModbusRTUMaster::setStopBits(int stopBits) {
    if (connected_) {
        lastError_ = "Cannot change stop bits while connected";
        logger_.logError("modbus_rtu", "Change stop bits failed", lastError_);
        return false;
    }
    
    // Check for valid stop bits
    if (stopBits == 1 || stopBits == 2) {
        stopBits_ = stopBits;
        return true;
    } else {
        lastError_ = "Invalid stop bits (valid: 1, 2)";
        logger_.logError("modbus_rtu", "Invalid stop bits", 
                       "{\"bits\":" + std::to_string(stopBits) + "}");
        return false;
    }
} 