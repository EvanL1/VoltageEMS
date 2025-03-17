#ifndef COM_BASE_H
#define COM_BASE_H

#include <string>
#include <memory>
#include <vector>
#include <map>
#include <mutex>
#include <atomic>
#include <chrono>
#include <yaml-cpp/yaml.h>

namespace voltage {
namespace comsrv {

/**
 * @brief Channel configuration structure
 */
struct ChannelConfig {
    std::string id;                  // Unique channel identifier
    std::string name;                // Human readable name
    std::string description;         // Channel description
    std::string protocol;            // Protocol type (e.g., "modbus_rtu", "modbus_tcp")
    YAML::Node parameters;           // Protocol-specific parameters in YAML format
    
    ChannelConfig() = default;
    
    // Create from YAML node
    static ChannelConfig fromYaml(const YAML::Node& node) {
        ChannelConfig config;
        config.id = node["id"].as<std::string>();
        config.name = node["name"].as<std::string>();
        config.description = node["description"].as<std::string>();
        config.protocol = node["protocol"].as<std::string>();
        config.parameters = node["parameters"];
        return config;
    }
};

/**
 * @brief Channel status information structure
 */
struct ChannelStatus {
    std::string id;                 // Channel identifier
    bool connected;                 // Connection status
    double lastResponseTime;        // Last response time in seconds
    std::string lastError;          // Last error message
    std::chrono::steady_clock::time_point lastUpdateTime;  // Last status update time
    
    ChannelStatus(const std::string& channelId)
        : id(channelId)
        , connected(false)
        , lastResponseTime(0.0)
        , lastError("")
        , lastUpdateTime(std::chrono::steady_clock::now())
    {}
    
    // Check if channel has an error
    bool hasError() const {
        return !lastError.empty();
    }
    
    // Get the last error message
    std::string getLastError() const {
        return lastError;
    }
    
    // Check if channel is connected
    bool isConnected() const {
        return connected;
    }
    
    // Get the channel ID
    std::string getId() const {
        return id;
    }
    
    // Get the last response time
    double getLastResponseTime() const {
        return lastResponseTime;
    }
};

/**
 * @brief Base class for all communication services
 * 
 * This class defines the interface for all communication services.
 * Each service can manage multiple channels with different protocols.
 */
class ComBase {
public:
    /**
     * @brief Constructor
     * 
     * @param serviceName Name of the communication service
     */
    explicit ComBase(const std::string& serviceName);

    /**
     * @brief Virtual destructor
     */
    virtual ~ComBase();

    /**
     * @brief Load configuration from YAML file
     * 
     * @param configFile Path to YAML configuration file
     * @return true if loaded successfully, false otherwise
     */
    bool loadConfig(const std::string& configFile);

    /**
     * @brief Add a new channel from configuration
     * 
     * @param config Channel configuration
     * @return true if added successfully, false if channel ID already exists
     */
    bool addChannel(const ChannelConfig& config);

    /**
     * @brief Remove a channel
     * 
     * @param channelId Channel identifier
     * @return true if removed successfully, false if channel not found
     */
    bool removeChannel(const std::string& channelId);

    /**
     * @brief Start the service and all configured channels
     * 
     * @return true if started successfully, false otherwise
     */
    virtual bool start() = 0;

    /**
     * @brief Stop the service and all channels
     * 
     * @return true if stopped successfully, false otherwise
     */
    virtual bool stop() = 0;

    /**
     * @brief Check if the service is running
     * 
     * @return true if running, false otherwise
     */
    bool isRunning() const;

    /**
     * @brief Get the service name
     * 
     * @return Service name
     */
    std::string getName() const;

    /**
     * @brief Get list of configured channels
     * 
     * @return Vector of channel configurations
     */
    std::vector<ChannelConfig> getChannelConfigs() const;

    /**
     * @brief Get list of channel statuses
     * 
     * @return Vector of channel status information
     */
    std::vector<ChannelStatus> getChannelStatuses() const;

    /**
     * @brief Check if there are any errors in the service
     * 
     * @return true if there are errors, false otherwise
     */
    bool hasError() const;

    /**
     * @brief Get the last error message from the service
     * 
     * @return Last error message
     */
    std::string getLastError() const;

protected:
    std::string m_serviceName;      // Name of this service instance
    std::atomic<bool> m_running;    // Running status
    std::mutex m_mutex;             // Mutex for thread safety
    std::string m_lastError;        // Last error message

    std::map<std::string, ChannelConfig> m_channelConfigs;    // Channel configurations
    std::map<std::string, ChannelStatus> m_channelStatuses;   // Channel statuses

    /**
     * @brief Record communication metrics for a channel
     * 
     * @param channelId Channel identifier
     * @param bytesSent Number of bytes sent
     * @param bytesReceived Number of bytes received
     * @param responseTime Response time in seconds
     */
    void recordMetrics(const std::string& channelId, size_t bytesSent, size_t bytesReceived, double responseTime);

    /**
     * @brief Record error metrics
     * 
     * @param errorType Type of error
     * @param channelId Optional channel identifier
     */
    void recordError(const std::string& errorType, const std::string& channelId = "");

    /**
     * @brief Update channel status
     * 
     * @param channelId Channel identifier
     * @param connected Connection status
     * @param responseTime Response time in seconds
     * @param error Optional error message
     */
    void updateChannelStatus(const std::string& channelId, bool connected, double responseTime = 0.0, const std::string& error = "");

    /**
     * @brief Validate channel configuration
     * 
     * @param config Channel configuration to validate
     * @return true if configuration is valid, false otherwise
     */
    virtual bool validateChannelConfig(const ChannelConfig& config) = 0;
};

} // namespace comsrv
} // namespace voltage

#endif // COM_BASE_H 