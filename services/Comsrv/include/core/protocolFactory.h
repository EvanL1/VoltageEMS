#ifndef PROTOCOL_FACTORY_H
#define PROTOCOL_FACTORY_H

#include <string>
#include <map>
#include <memory>
#include <functional>
#include <mutex>
#include "core/comBase.h"
#include "core/config/configManager.h"

/**
 * @brief Protocol Factory class
 * 
 * This class is responsible for creating and managing communication protocol instances.
 * It uses a factory pattern to create instances of different protocol types based on configuration.
 */
class ProtocolFactory {
public:
    /**
     * @brief Get singleton instance
     * 
     * @return Reference to the singleton instance
     */
    static ProtocolFactory& getInstance();

    /**
     * @brief Register a protocol creator function
     * 
     * @param protocolType Type of protocol
     * @param creator Creator function that takes a ConfigManager::ConfigValue and returns a ComBase*
     * @return true if registered successfully, false if already registered
     */
    bool registerProtocol(const std::string& protocolType, 
                         std::function<std::unique_ptr<ComBase>(const std::map<std::string, ConfigManager::ConfigValue>&)> creator);

    /**
     * @brief Create a protocol instance
     * 
     * @param protocolType Type of protocol
     * @param config Configuration for the protocol
     * @return Unique pointer to the created protocol instance, or nullptr if failed
     */
    std::unique_ptr<ComBase> createProtocol(const std::string& protocolType, 
                                           const std::map<std::string, ConfigManager::ConfigValue>& config);

    /**
     * @brief Create protocol instances from configuration
     * 
     * @param configFile Path to the configuration file
     * @return Vector of created protocol instances
     */
    std::vector<std::unique_ptr<ComBase>> createProtocolsFromConfig(const std::string& configFile);

private:
    // Private constructor for singleton
    ProtocolFactory();
    ~ProtocolFactory();

    // Disable copy and move
    ProtocolFactory(const ProtocolFactory&) = delete;
    ProtocolFactory& operator=(const ProtocolFactory&) = delete;
    ProtocolFactory(ProtocolFactory&&) = delete;
    ProtocolFactory& operator=(ProtocolFactory&&) = delete;

    // Protocol creator functions
    std::map<std::string, std::function<std::unique_ptr<ComBase>(const std::map<std::string, ConfigManager::ConfigValue>&)>> m_creators;

    // Thread safety
    std::mutex m_mutex;
};

#endif // PROTOCOL_FACTORY_H 