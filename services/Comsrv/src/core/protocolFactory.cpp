#include "core/protocolFactory.h"
#include <iostream>
#include <nlohmann/json.hpp>
#include <fstream>

using json = nlohmann::json;

// Singleton instance
ProtocolFactory& ProtocolFactory::getInstance() {
    static ProtocolFactory instance;
    return instance;
}

ProtocolFactory::ProtocolFactory() {
    std::cout << "ProtocolFactory initialized" << std::endl;
}

ProtocolFactory::~ProtocolFactory() {
    std::cout << "ProtocolFactory destroyed" << std::endl;
}

bool ProtocolFactory::registerProtocol(
    const std::string& protocolType, 
    std::function<std::unique_ptr<ComBase>(const std::map<std::string, ConfigManager::ConfigValue>&)> creator) {
    
    std::lock_guard<std::mutex> lock(m_mutex);
    
    // Check if already registered
    if (m_creators.find(protocolType) != m_creators.end()) {
        std::cerr << "Protocol type already registered: " << protocolType << std::endl;
        return false;
    }
    
    // Register the creator function
    m_creators[protocolType] = creator;
    std::cout << "Registered protocol type: " << protocolType << std::endl;
    return true;
}

std::unique_ptr<ComBase> ProtocolFactory::createProtocol(
    const std::string& protocolType, 
    const std::map<std::string, ConfigManager::ConfigValue>& config) {
    
    std::lock_guard<std::mutex> lock(m_mutex);
    
    // Check if protocol type is registered
    auto it = m_creators.find(protocolType);
    if (it == m_creators.end()) {
        std::cerr << "Unknown protocol type: " << protocolType << std::endl;
        return nullptr;
    }
    
    // Create the protocol instance
    try {
        return it->second(config);
    } catch (const std::exception& e) {
        std::cerr << "Error creating protocol instance: " << e.what() << std::endl;
        return nullptr;
    }
}

std::vector<std::unique_ptr<ComBase>> ProtocolFactory::createProtocolsFromConfig(const std::string& configFile) {
    std::vector<std::unique_ptr<ComBase>> protocols;
    
    // Load the configuration file
    std::ifstream file(configFile);
    if (!file.is_open()) {
        std::cerr << "Failed to open config file: " << configFile << std::endl;
        return protocols;
    }
    
    try {
        json j;
        file >> j;
        
        // Parse protocols section
        if (j.contains("protocols") && j["protocols"].is_array()) {
            for (const auto& protocol : j["protocols"]) {
                if (!protocol.contains("type") || !protocol["type"].is_string()) {
                    std::cerr << "Protocol configuration missing 'type' field" << std::endl;
                    continue;
                }
                
                std::string protocolType = protocol["type"].get<std::string>();
                
                // Convert JSON config to our config format
                std::map<std::string, ConfigManager::ConfigValue> config;
                for (auto& [key, value] : protocol.items()) {
                    if (key == "type") continue; // Skip the type field
                    
                    if (value.is_number_integer()) {
                        config[key] = value.get<int>();
                    } else if (value.is_number_float()) {
                        config[key] = value.get<double>();
                    } else if (value.is_boolean()) {
                        config[key] = value.get<bool>();
                    } else if (value.is_string()) {
                        config[key] = value.get<std::string>();
                    } else if (value.is_array()) {
                        if (value.size() > 0) {
                            if (value[0].is_number_integer()) {
                                config[key] = value.get<std::vector<int>>();
                            } else if (value[0].is_number_float()) {
                                config[key] = value.get<std::vector<double>>();
                            } else if (value[0].is_boolean()) {
                                config[key] = value.get<std::vector<bool>>();
                            } else if (value[0].is_string()) {
                                config[key] = value.get<std::vector<std::string>>();
                            }
                        }
                    }
                }
                
                // Create the protocol instance
                auto protocolInstance = createProtocol(protocolType, config);
                if (protocolInstance) {
                    protocols.push_back(std::move(protocolInstance));
                }
            }
        } else {
            std::cerr << "Config file missing 'protocols' array" << std::endl;
        }
    } catch (const json::exception& e) {
        std::cerr << "JSON parsing error: " << e.what() << std::endl;
    } catch (const std::exception& e) {
        std::cerr << "Error parsing config: " << e.what() << std::endl;
    }
    
    return protocols;
} 