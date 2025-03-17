#include "core/protocolFactory.h"
#include "core/protocols/modbus/modbusRTUMaster.h"
#include "core/protocols/modbus/modbusTCPMaster.h"
#include "core/protocols/iec104/iec104Protocol.h"
#include "core/metrics.h"
#include <iostream>
#include <yaml-cpp/yaml.h>
#include <fstream>
#include <stdexcept>

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

void ProtocolFactory::registerProtocol(const std::string& type, ProtocolCreator creator) {
    if (creator == nullptr) {
        throw std::invalid_argument("Protocol creator function cannot be null");
    }
    
    std::lock_guard<std::mutex> lock(m_mutex);
    m_creators[type] = creator;
}

std::unique_ptr<ComBase> ProtocolFactory::createProtocol(const std::string& type, const ProtocolConfig& config) {
    std::lock_guard<std::mutex> lock(m_mutex);
    
    auto it = m_creators.find(type);
    if (it == m_creators.end()) {
        throw std::runtime_error("Unknown protocol type: " + type);
    }
    
    return it->second(config);
}

int ProtocolFactory::registerSupportedProtocols() {
    int count = 0;
    auto& metrics = voltage::comsrv::Metrics::instance();
    
    // Define all supported protocols with their creators
    struct ProtocolInfo {
        std::string type;
        std::function<std::unique_ptr<ComBase>(const ProtocolConfig&)> creator;
    };
    
    std::vector<ProtocolInfo> protocols = {
        {
            "modbus_rtu_master",
            [](const ProtocolConfig& config) -> std::unique_ptr<ComBase> {
                try {
                    return std::make_unique<ModbusRTUMaster>(config);
                } catch (const std::exception& e) {
                    std::cerr << "Error creating ModbusRTUMaster: " << e.what() << std::endl;
                    voltage::comsrv::Metrics::instance().incrementProtocolErrors("modbus_rtu_master", "creation_failed");
                    return nullptr;
                }
            }
        },
        {
            "modbus_tcp_master",
            [](const ProtocolConfig& config) -> std::unique_ptr<ComBase> {
                try {
                    return std::make_unique<ModbusTCPMaster>(config);
                } catch (const std::exception& e) {
                    std::cerr << "Error creating ModbusTCPMaster: " << e.what() << std::endl;
                    voltage::comsrv::Metrics::instance().incrementProtocolErrors("modbus_tcp_master", "creation_failed");
                    return nullptr;
                }
            }
        },
        {
            "iec104",
            [](const ProtocolConfig& config) -> std::unique_ptr<ComBase> {
                try {
                    return std::make_unique<IEC104Protocol>(config);
                } catch (const std::exception& e) {
                    std::cerr << "Error creating IEC104Protocol: " << e.what() << std::endl;
                    voltage::comsrv::Metrics::instance().incrementProtocolErrors("iec104", "creation_failed");
                    return nullptr;
                }
            }
        }
    };
    
    // Register all protocols
    for (const auto& protocol : protocols) {
        try {
            registerProtocol(protocol.type, protocol.creator);
            count++;
            std::cout << "Registered protocol: " << protocol.type << std::endl;
        } catch (const std::exception& e) {
            std::cerr << "Failed to register protocol " << protocol.type << ": " << e.what() << std::endl;
            metrics.incrementProtocolErrors(protocol.type, "registration_failed");
        }
    }
    
    return count;
}

std::vector<std::unique_ptr<ComBase>> ProtocolFactory::createProtocolsFromConfig(const std::string& configFile) {
    std::vector<std::unique_ptr<ComBase>> protocols;
    
    try {
        // Load configuration file
        YAML::Node config = YAML::LoadFile(configFile);
        
        if (!config["channels"] || !config["channels"].IsSequence()) {
            throw std::runtime_error("Invalid configuration: 'channels' section missing or not a sequence");
        }
        
        // Process each channel in the configuration
        for (const auto& channelConfig : config["channels"]) {
            // Skip disabled channels
            if (channelConfig["enabled"] && !channelConfig["enabled"].as<bool>(true)) {
                std::cout << "Skipping disabled channel: " << channelConfig["id"].as<std::string>("unknown") << std::endl;
                continue;
            }
            
            if (!channelConfig["protocol"] || !channelConfig["protocol"].IsScalar()) {
                std::cerr << "Channel missing protocol type, skipping" << std::endl;
                continue;
            }
            
            std::string protocolType = channelConfig["protocol"].as<std::string>();
            
            // Create a config map from the YAML node
            ProtocolConfig configMap;
            
            // Add channel ID
            if (channelConfig["id"]) {
                configMap["id"] = channelConfig["id"].as<std::string>();
            }
            
            // Add protocol type
            configMap["type"] = protocolType;
            
            // Add connection parameters
            if (channelConfig["connection"]) {
                for (const auto& param : channelConfig["connection"]) {
                    std::string key = param.first.as<std::string>();
                    YAML::Node value = param.second;
                    
                    if (value.IsScalar()) {
                        // Handle different scalar types
                        if (value.IsNull()) {
                            configMap["connection." + key] = "";
                        } else if (value.IsInteger()) {
                            configMap["connection." + key] = std::to_string(value.as<int>());
                        } else if (value.IsNumber()) {
                            configMap["connection." + key] = std::to_string(value.as<double>());
                        } else {
                            configMap["connection." + key] = value.as<std::string>();
                        }
                    } else if (value.IsSequence()) {
                        // Convert sequence to comma-separated string
                        std::string valueStr;
                        for (size_t i = 0; i < value.size(); ++i) {
                            if (i > 0) valueStr += ",";
                            valueStr += value[i].as<std::string>();
                        }
                        configMap["connection." + key] = valueStr;
                    }
                }
            }
            
            // Add protocol parameters
            if (channelConfig["parameters"]) {
                for (const auto& param : channelConfig["parameters"]) {
                    std::string key = param.first.as<std::string>();
                    YAML::Node value = param.second;
                    
                    if (value.IsScalar()) {
                        // Handle different scalar types
                        if (value.IsNull()) {
                            configMap["parameters." + key] = "";
                        } else if (value.IsInteger()) {
                            configMap["parameters." + key] = std::to_string(value.as<int>());
                        } else if (value.IsNumber()) {
                            configMap["parameters." + key] = std::to_string(value.as<double>());
                        } else {
                            configMap["parameters." + key] = value.as<std::string>();
                        }
                    } else if (value.IsSequence()) {
                        // Convert sequence to comma-separated string
                        std::string valueStr;
                        for (size_t i = 0; i < value.size(); ++i) {
                            if (i > 0) valueStr += ",";
                            valueStr += value[i].as<std::string>();
                        }
                        configMap["parameters." + key] = valueStr;
                    }
                }
            }
            
            // Add global labels if present
            if (config["service"] && config["service"]["labels"]) {
                for (const auto& label : config["service"]["labels"]) {
                    std::string key = label.first.as<std::string>();
                    std::string value = label.second.as<std::string>();
                    configMap["global.labels." + key] = value;
                }
            }
            
            // Add channel-specific labels if present
            if (channelConfig["labels"]) {
                for (const auto& label : channelConfig["labels"]) {
                    std::string key = label.first.as<std::string>();
                    std::string value = label.second.as<std::string>();
                    configMap["labels." + key] = value;
                }
            }
            
            // Create protocol instance
            try {
                auto protocol = createProtocol(protocolType, configMap);
                if (protocol) {
                    protocols.push_back(std::move(protocol));
                }
            } catch (const std::exception& e) {
                std::cerr << "Error creating protocol " << protocolType << ": " << e.what() << std::endl;
                voltage::comsrv::Metrics::instance().incrementProtocolErrors(protocolType, "creation_failed");
            }
        }
    } catch (const YAML::Exception& e) {
        std::cerr << "YAML parsing error: " << e.what() << std::endl;
        throw std::runtime_error("Failed to parse configuration file: " + std::string(e.what()));
    } catch (const std::exception& e) {
        std::cerr << "Error loading configuration: " << e.what() << std::endl;
        throw;
    }
    
    return protocols;
} 