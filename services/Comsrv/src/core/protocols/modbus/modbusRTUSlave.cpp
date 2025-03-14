#include "protocols/modbus/modbusRTUSlave.h"
#include <iostream>
#include <json/json.h>
#include <fcntl.h>
#include <unistd.h>
#include <errno.h>

ModbusRTUSlave::ModbusRTUSlave()
    : baudRate_(9600), parity_('N'), dataBits_(8), stopBits_(1) {
    // Set physical interface type
    setPhysicalInterfaceType(PhysicalInterfaceType::SERIAL);
}

ModbusRTUSlave::~ModbusRTUSlave() {
    // Ensure connection is properly closed
    disconnect();
}

bool ModbusRTUSlave::init(const std::string& config) {
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
        
        // Set up register mapping
        int nbCoils = 0;
        int nbDiscreteInputs = 0;
        int nbHoldingRegisters = 0;
        int nbInputRegisters = 0;
        
        if (root.isMember("coils") && root["coils"].isInt()) {
            nbCoils = root["coils"].asInt();
        }
        
        if (root.isMember("discreteInputs") && root["discreteInputs"].isInt()) {
            nbDiscreteInputs = root["discreteInputs"].asInt();
        }
        
        if (root.isMember("holdingRegisters") && root["holdingRegisters"].isInt()) {
            nbHoldingRegisters = root["holdingRegisters"].asInt();
        }
        
        if (root.isMember("inputRegisters") && root["inputRegisters"].isInt()) {
            nbInputRegisters = root["inputRegisters"].asInt();
        }
        
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
        
        // Configure the Modbus context
        modbus_set_slave(ctx_, slaveId_);
        
        // Set debug mode
        modbus_set_debug(ctx_, debug_ ? TRUE : FALSE);
        
        // Set response timeout
        struct timeval tv;
        tv.tv_sec = timeout_ / 1000;
        tv.tv_usec = (timeout_ % 1000) * 1000;
        modbus_set_response_timeout(ctx_, tv.tv_sec, tv.tv_usec);
        
        // Set up message hooks for logging
        modbus_set_pre_send_callback(ctx_, modbusPreSendCallback);
        modbus_set_post_recv_callback(ctx_, modbusPostRecvCallback);
        
        // Create the Modbus mapping
        if (!setupModbusMapping(nbCoils, nbDiscreteInputs, nbHoldingRegisters, nbInputRegisters)) {
            lastError_ = "Failed to create Modbus mapping";
            logger_.logError("modbus_rtu", "Mapping creation failed", lastError_);
            return false;
        }
        
        // Connect to the serial port
        if (!connect(serialPort_, baudRate_, parity_, dataBits_, stopBits_)) {
            return false;
        }
        
        logger_.logInfo("modbus_rtu", "RTU slave initialized", 
                      "{\"port\":\"" + serialPort_ + 
                      "\",\"baud\":" + std::to_string(baudRate_) + 
                      ",\"format\":\"" + std::to_string(dataBits_) + parity_ + std::to_string(stopBits_) + 
                      "\",\"slaveId\":" + std::to_string(slaveId_) + "}");
        
        return true;
        
    } catch (const std::exception& e) {
        lastError_ = "Exception during initialization: " + std::string(e.what());
        logger_.logError("modbus_rtu", "Init exception", lastError_);
        return false;
    }
}

std::string ModbusRTUSlave::getStatus() const {
    std::stringstream ss;
    ss << "Modbus RTU Slave: "
       << (connected_ ? "Connected" : "Disconnected")
       << ", Port: " << serialPort_
       << ", Baud: " << baudRate_
       << ", Format: " << dataBits_ << parity_ << stopBits_
       << ", Slave ID: " << slaveId_
       << ", Running: " << (running_ ? "Yes" : "No");
    return ss.str();
}

std::string ModbusRTUSlave::getStatistics() const {
    std::stringstream ss;
    ss << "Messages Received: " << messagesReceived_
       << ", Messages Sent: " << messagesSent_
       << ", Bytes Received: " << bytesReceived_
       << ", Bytes Sent: " << bytesSent_
       << ", Errors: " << errorCount_
       << ", CRC Errors: " << crcErrorCount_;
    return ss.str();
}

bool ModbusRTUSlave::connect(const std::string& serialPort, int baudRate, char parity, int dataBits, int stopBits) {
    // Store connection parameters
    serialPort_ = serialPort;
    baudRate_ = baudRate;
    parity_ = parity;
    dataBits_ = dataBits;
    stopBits_ = stopBits;
    
    // Close any existing connection
    disconnect();
    
    // Create new context
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
    
    // Set up the context
    modbus_set_slave(ctx_, slaveId_);
    modbus_set_debug(ctx_, debug_ ? TRUE : FALSE);
    
    // Set timeout
    struct timeval tv;
    tv.tv_sec = timeout_ / 1000;
    tv.tv_usec = (timeout_ % 1000) * 1000;
    modbus_set_response_timeout(ctx_, tv.tv_sec, tv.tv_usec);
    
    // Set up callbacks
    modbus_set_pre_send_callback(ctx_, modbusPreSendCallback);
    modbus_set_post_recv_callback(ctx_, modbusPostRecvCallback);
    
    // Connect to the serial port
    if (modbus_connect(ctx_) == -1) {
        lastError_ = "Failed to connect to RTU port: " + std::string(modbus_strerror(errno));
        logger_.logError("modbus_rtu", "Connect failed", lastError_);
        modbus_free(ctx_);
        ctx_ = nullptr;
        return false;
    }
    
    // Try to set the serial port to non-blocking mode
    int serialFd = modbus_get_socket(ctx_);
    if (serialFd != -1) {
        int flags = fcntl(serialFd, F_GETFL, 0);
        if (flags != -1) {
            fcntl(serialFd, F_SETFL, flags | O_NONBLOCK);
        }
    }
    
    connected_ = true;
    logger_.logInfo("modbus_rtu", "RTU slave connected", 
                  "{\"port\":\"" + serialPort_ + "\",\"baud\":" + std::to_string(baudRate_) + "}");
    
    return true;
}

bool ModbusRTUSlave::disconnect() {
    // Stop the listen thread if running
    stopListening();
    
    if (ctx_) {
        // Close the connection
        modbus_close(ctx_);
        modbus_free(ctx_);
        ctx_ = nullptr;
    }
    
    connected_ = false;
    logger_.logInfo("modbus_rtu", "RTU slave disconnected", "{}");
    
    return true;
}

bool ModbusRTUSlave::setSerialPort(const std::string& serialPort) {
    if (connected_) {
        lastError_ = "Cannot change serial port while connected";
        logger_.logError("modbus_rtu", "Change port failed", lastError_);
        return false;
    }
    
    serialPort_ = serialPort;
    return true;
}

bool ModbusRTUSlave::setBaudRate(int baudRate) {
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

bool ModbusRTUSlave::setParity(char parity) {
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

bool ModbusRTUSlave::setDataBits(int dataBits) {
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

bool ModbusRTUSlave::setStopBits(int stopBits) {
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

void ModbusRTUSlave::listenThreadFunc() {
    logger_.logInfo("modbus_rtu", "RTU slave listen thread started", "{}");
    
    // Buffer for Modbus requests
    uint8_t requestBuffer[MODBUS_RTU_MAX_ADU_LENGTH];
    
    // Main loop
    while (running_) {
        try {
            // Check if we're connected
            if (!connected_) {
                logger_.logWarning("modbus_rtu", "Slave not connected, waiting...", "{}");
                std::this_thread::sleep_for(std::chrono::seconds(1));
                continue;
            }
            
            // Attempt to receive a request with non-blocking I/O
            int requestLength = modbus_receive(ctx_, requestBuffer);
            
            if (requestLength > 0) {
                // Valid request received
                messagesReceived_++;
                bytesReceived_ += requestLength;
                
                logger_.logDebug("modbus_rtu", "Received request", 
                               "{\"length\":" + std::to_string(requestLength) + 
                               ",\"data\":\"" + formatMessage(requestBuffer, requestLength) + "\"}");
                
                // Process the request
                int slaveId = requestBuffer[0]; // First byte is the slave ID
                
                // Only process messages intended for our slave ID or broadcast
                if (slaveId == slaveId_ || slaveId == 0) {
                    modbus_reply(ctx_, requestBuffer, requestLength, mapping_);
                    
                    // Update statistics
                    messagesSent_++;
                    bytesSent_ += requestLength; // Approximate response length
                    
                    logger_.logDebug("modbus_rtu", "Sent response", "{}");
                } else {
                    logger_.logDebug("modbus_rtu", "Ignored message for other slave ID", 
                                   "{\"targetId\":" + std::to_string(slaveId) + 
                                   ",\"ourId\":" + std::to_string(slaveId_) + "}");
                }
                
            } else if (requestLength == -1) {
                // Error or timeout receiving
                if (errno != EAGAIN && errno != EWOULDBLOCK && errno != ETIMEDOUT) {
                    // Real error (not just no data available)
                    lastError_ = modbus_strerror(errno);
                    lastErrorCode_ = errno;
                    
                    if (errno == ECONNRESET || errno == EPIPE) {
                        // Connection lost
                        logger_.logWarning("modbus_rtu", "Connection lost", 
                                         "{\"error\":\"" + lastError_ + "\"}");
                        connected_ = false;
                    } else if (errno == EMBBADCRC) {
                        // CRC error - specific to RTU
                        crcErrorCount_++;
                        logger_.logWarning("modbus_rtu", "CRC error in received message", "{}");
                    } else {
                        // Other error
                        errorCount_++;
                        logger_.logError("modbus_rtu", "Error receiving request", 
                                       "{\"error\":\"" + lastError_ + "\"}");
                    }
                }
                
                // No need to print errors for timeouts (ETIMEDOUT) or no data (EAGAIN, EWOULDBLOCK)
            }
            
            // Short sleep to avoid high CPU usage
            std::this_thread::sleep_for(std::chrono::milliseconds(1));
            
        } catch (const std::exception& e) {
            lastError_ = "Exception in RTU listen thread: " + std::string(e.what());
            logger_.logError("modbus_rtu", "Listen thread exception", lastError_);
            
            // Sleep longer after an exception
            std::this_thread::sleep_for(std::chrono::seconds(1));
        }
    }
    
    logger_.logInfo("modbus_rtu", "RTU slave listen thread stopped", "{}");
} 