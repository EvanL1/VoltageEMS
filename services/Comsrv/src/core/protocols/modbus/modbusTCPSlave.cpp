#include "protocols/modbus/modbusTCPSlave.h"
#include <iostream>
#include <json/json.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <fcntl.h>
#include <unistd.h>
#include <errno.h>
#include <vector>
#include <algorithm>

ModbusTCPSlave::ModbusTCPSlave()
    : port_(502), unitId_(255), serverSocket_(-1) {
    // Set physical interface type
    setPhysicalInterfaceType(PhysicalInterfaceType::NETWORK);
}

ModbusTCPSlave::~ModbusTCPSlave() {
    // Ensure connection is properly closed
    disconnect();
}

bool ModbusTCPSlave::init(const std::string& config) {
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
        if (root.isMember("ip") && root["ip"].isString()) {
            ipAddress_ = root["ip"].asString();
        } else {
            ipAddress_ = "0.0.0.0";  // Default is to listen on all interfaces
        }
        
        if (root.isMember("port") && root["port"].isInt()) {
            port_ = root["port"].asInt();
        } else {
            port_ = 502;  // Default Modbus TCP port
        }
        
        if (root.isMember("unitId") && root["unitId"].isInt()) {
            unitId_ = root["unitId"].asInt();
        } else {
            unitId_ = 255;  // Default unit ID
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
        
        // Create Modbus TCP context
        ctx_ = modbus_new_tcp(ipAddress_.c_str(), port_);
        
        if (ctx_ == nullptr) {
            lastError_ = "Failed to create Modbus TCP context";
            logger_.logError("modbus_tcp", "Context creation failed", lastError_);
            return false;
        }
        
        // Configure the Modbus context
        setSlaveId(unitId_);
        
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
            logger_.logError("modbus_tcp", "Mapping creation failed", lastError_);
            return false;
        }
        
        // Connect to listen socket
        if (!connect(ipAddress_, port_)) {
            return false;
        }
        
        logger_.logInfo("modbus_tcp", "TCP slave initialized", 
                      "{\"ip\":\"" + ipAddress_ + "\",\"port\":" + std::to_string(port_) + 
                      ",\"unitId\":" + std::to_string(unitId_) + "}");
        
        return true;
        
    } catch (const std::exception& e) {
        lastError_ = "Exception during initialization: " + std::string(e.what());
        logger_.logError("modbus_tcp", "Init exception", lastError_);
        return false;
    }
}

std::string ModbusTCPSlave::getStatus() const {
    std::stringstream ss;
    ss << "Modbus TCP Slave: "
       << (connected_ ? "Connected" : "Disconnected")
       << ", IP: " << ipAddress_
       << ", Port: " << port_ 
       << ", Unit ID: " << unitId_
       << ", Running: " << (running_ ? "Yes" : "No");
    return ss.str();
}

std::string ModbusTCPSlave::getStatistics() const {
    std::stringstream ss;
    ss << "Messages Received: " << messagesReceived_
       << ", Messages Sent: " << messagesSent_
       << ", Bytes Received: " << bytesReceived_
       << ", Bytes Sent: " << bytesSent_
       << ", Errors: " << errorCount_;
    return ss.str();
}

bool ModbusTCPSlave::connect(const std::string& host, int port) {
    // For a slave, this creates a listening socket
    ipAddress_ = host;
    port_ = port;
    
    // Close any existing connection
    disconnect();
    
    // Listening socket is managed separately from modbus context
    // We'll use the modbus_tcp_listen function to set up the listening socket
    serverSocket_ = modbus_tcp_listen(ctx_, MAX_CONNECTIONS);
    
    if (serverSocket_ == -1) {
        lastError_ = "Failed to listen on TCP socket: " + std::string(modbus_strerror(errno));
        logger_.logError("modbus_tcp", "Listen failed", lastError_);
        return false;
    }
    
    // Set socket to non-blocking mode
    int flags = fcntl(serverSocket_, F_GETFL, 0);
    if (flags == -1) {
        lastError_ = "Failed to get socket flags: " + std::string(strerror(errno));
        logger_.logError("modbus_tcp", "Socket config failed", lastError_);
        close(serverSocket_);
        serverSocket_ = -1;
        return false;
    }
    
    if (fcntl(serverSocket_, F_SETFL, flags | O_NONBLOCK) == -1) {
        lastError_ = "Failed to set socket non-blocking: " + std::string(strerror(errno));
        logger_.logError("modbus_tcp", "Socket config failed", lastError_);
        close(serverSocket_);
        serverSocket_ = -1;
        return false;
    }
    
    connected_ = true;
    logger_.logInfo("modbus_tcp", "TCP slave listening", 
                  "{\"ip\":\"" + ipAddress_ + "\",\"port\":" + std::to_string(port_) + "}");
    
    return true;
}

bool ModbusTCPSlave::disconnect() {
    // Stop the listen thread if running
    stopListening();
    
    // Close the server socket
    if (serverSocket_ != -1) {
        close(serverSocket_);
        serverSocket_ = -1;
    }
    
    if (ctx_) {
        // Close the connection
        modbus_close(ctx_);
        modbus_free(ctx_);
        ctx_ = nullptr;
    }
    
    connected_ = false;
    logger_.logInfo("modbus_tcp", "TCP slave disconnected", "{}");
    
    return true;
}

bool ModbusTCPSlave::setIPAddress(const std::string& ip) {
    if (connected_) {
        lastError_ = "Cannot change IP address while connected";
        logger_.logError("modbus_tcp", "Change IP failed", lastError_);
        return false;
    }
    
    ipAddress_ = ip;
    return true;
}

bool ModbusTCPSlave::setPort(int port) {
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

bool ModbusTCPSlave::setUnitId(int unitId) {
    if (unitId < 0 || unitId > 255) {
        lastError_ = "Invalid unit ID (valid range: 0-255)";
        logger_.logError("modbus_tcp", "Invalid unit ID", 
                       "{\"unitId\":" + std::to_string(unitId) + "}");
        return false;
    }
    
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

void ModbusTCPSlave::listenThreadFunc() {
    logger_.logInfo("modbus_tcp", "TCP slave listen thread started", "{}");
    
    // Set up the fd_set for select
    fd_set refset;
    FD_ZERO(&refset);
    FD_SET(serverSocket_, &refset);
    
    int fdmax = serverSocket_;
    
    // Array of client sockets
    std::vector<int> clientSockets;
    
    // Buffer for Modbus requests
    uint8_t requestBuffer[MODBUS_TCP_MAX_ADU_LENGTH];
    
    // Main loop
    while (running_) {
        // Copy the reference set
        fd_set rdset = refset;
        
        // Wait for activity on the sockets with timeout
        struct timeval tv;
        tv.tv_sec = 0;
        tv.tv_usec = 100000;  // 100ms timeout
        
        int ready = select(fdmax + 1, &rdset, NULL, NULL, &tv);
        
        if (ready == -1) {
            if (errno == EINTR) {
                // Interrupted by signal, just continue
                continue;
            }
            
            // Error in select
            lastError_ = "Select error: " + std::string(strerror(errno));
            logger_.logError("modbus_tcp", "Select error", lastError_);
            break;
        }
        
        // Check for new connections on the server socket
        if (FD_ISSET(serverSocket_, &rdset)) {
            struct sockaddr_in clientAddr;
            socklen_t addrLen = sizeof(clientAddr);
            
            int newSocket = accept(serverSocket_, (struct sockaddr*)&clientAddr, &addrLen);
            
            if (newSocket == -1) {
                if (errno != EAGAIN && errno != EWOULDBLOCK) {
                    lastError_ = "Accept error: " + std::string(strerror(errno));
                    logger_.logError("modbus_tcp", "Accept error", lastError_);
                }
            } else {
                // Set new socket to non-blocking
                int flags = fcntl(newSocket, F_GETFL, 0);
                fcntl(newSocket, F_SETFL, flags | O_NONBLOCK);
                
                // Add to the set
                FD_SET(newSocket, &refset);
                
                // Update max fd if needed
                if (newSocket > fdmax) {
                    fdmax = newSocket;
                }
                
                // Add to client sockets
                clientSockets.push_back(newSocket);
                
                // Log new connection
                char clientIP[INET_ADDRSTRLEN];
                inet_ntop(AF_INET, &(clientAddr.sin_addr), clientIP, INET_ADDRSTRLEN);
                
                logger_.logInfo("modbus_tcp", "New client connected", 
                              "{\"ip\":\"" + std::string(clientIP) + 
                              "\",\"port\":" + std::to_string(ntohs(clientAddr.sin_port)) + "}");
            }
        }
        
        // Check client sockets for data
        auto it = clientSockets.begin();
        while (it != clientSockets.end()) {
            int clientSocket = *it;
            
            if (FD_ISSET(clientSocket, &rdset)) {
                // Receive modbus request
                modbus_set_socket(ctx_, clientSocket);
                
                // Receive the request
                int requestLength = modbus_receive(ctx_, requestBuffer);
                
                if (requestLength > 0) {
                    // Valid request received
                    messagesReceived_++;
                    bytesReceived_ += requestLength;
                    
                    logger_.logDebug("modbus_tcp", "Received request", 
                                    "{\"length\":" + std::to_string(requestLength) + 
                                    ",\"data\":\"" + formatMessage(requestBuffer, requestLength) + "\"}");
                    
                    // Process the request
                    modbus_reply(ctx_, requestBuffer, requestLength, mapping_);
                    
                    // Update statistics
                    messagesSent_++;
                    bytesSent_ += requestLength;  // Approximate response length
                    
                } else if (requestLength == -1) {
                    // Error receiving
                    if (errno != EAGAIN && errno != EWOULDBLOCK) {
                        lastError_ = modbus_strerror(errno);
                        lastErrorCode_ = errno;
                        
                        if (errno == ECONNRESET || errno == EPIPE) {
                            // Client disconnected
                            logger_.logInfo("modbus_tcp", "Client disconnected", "{}");
                            
                            // Remove from fd set
                            FD_CLR(clientSocket, &refset);
                            
                            // Close socket
                            close(clientSocket);
                            
                            // Remove from client sockets
                            it = clientSockets.erase(it);
                            continue;
                        } else {
                            // Other error
                            logger_.logError("modbus_tcp", "Error receiving request", 
                                          "{\"error\":\"" + lastError_ + "\"}");
                            errorCount_++;
                        }
                    }
                }
            }
            
            ++it;
        }
        
        // Short sleep to avoid high CPU usage
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
    
    // Clean up client sockets
    for (int clientSocket : clientSockets) {
        close(clientSocket);
    }
    
    logger_.logInfo("modbus_tcp", "TCP slave listen thread stopped", "{}");
} 