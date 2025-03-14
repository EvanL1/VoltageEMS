#ifndef MODBUS_TCP_SLAVE_H
#define MODBUS_TCP_SLAVE_H

#include "protocols/modbus/modbusSlave.h"

class ModbusTCPSlave : public ModbusSlave {
public:
    ModbusTCPSlave();
    ~ModbusTCPSlave() override;

    // ComBase interface implementations
    bool init(const std::string& config) override;
    std::string getStatus() const override;
    std::string getStatistics() const override;

    // TCP-specific methods
    bool connect(const std::string& host, int port);
    bool disconnect();
    bool isConnected() const { return connected_; }
    
    // TCP configuration
    bool setIPAddress(const std::string& ip);
    bool setPort(int port);
    bool setUnitId(int unitId);
    
    const std::string& getIPAddress() const { return ipAddress_; }
    int getPort() const { return port_; }
    int getUnitId() const { return unitId_; }

private:
    // Override the listening thread function for TCP-specific handling
    void listenThreadFunc() override;
    
    std::string ipAddress_;  // Server IP address
    int port_;               // Server port number
    int unitId_;             // Unit ID for TCP (similar to slave ID)
    int serverSocket_;       // Socket for listening
    
    // Statistics
    uint64_t bytesReceived_ = 0;
    uint64_t bytesSent_ = 0;
    uint64_t messagesReceived_ = 0;
    uint64_t messagesSent_ = 0;
    uint64_t errorCount_ = 0;
    
    // Maximum number of connections
    static const int MAX_CONNECTIONS = 32;
};

#endif // MODBUS_TCP_SLAVE_H 