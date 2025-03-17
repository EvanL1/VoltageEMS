#include "core/config/configManager.h"
#include <fstream>
#include <iostream>
#include <yaml-cpp/yaml.h>

// Singleton instance
ConfigManager& ConfigManager::getInstance() {
    static ConfigManager instance;
    return instance;
}

ConfigManager::ConfigManager() : m_nextCallbackId(0) {
    std::cout << "ConfigManager initialized" << std::endl;
}

ConfigManager::~ConfigManager() {
    std::cout << "ConfigManager destroyed" << std::endl;
}

bool ConfigManager::loadFromFile(const std::string& filename) {
    try {
        YAML::Node config = YAML::LoadFile(filename);
        
        std::lock_guard<std::mutex> lock(m_mutex);
        
        // Clear existing config
        m_config.clear();
        
        // Parse YAML into our config structure
        for (const auto& section : config) {
            std::string sectionName = section.first.as<std::string>();
            const YAML::Node& sectionData = section.second;
            
            if (sectionData.IsMap()) {
                for (const auto& item : sectionData) {
                    std::string key = item.first.as<std::string>();
                    const YAML::Node& value = item.second;
                    
                    if (value.IsScalar()) {
                        // Try to determine the type of scalar
                        try {
                            m_config[sectionName][key] = value.as<int>();
                        } catch (...) {
                            try {
                                m_config[sectionName][key] = value.as<double>();
                            } catch (...) {
                                try {
                                    m_config[sectionName][key] = value.as<bool>();
                                } catch (...) {
                                    m_config[sectionName][key] = value.as<std::string>();
                                }
                            }
                        }
                    } else if (value.IsSequence()) {
                        if (!value.empty()) {
                            // Try to determine the type of sequence
                            try {
                                m_config[sectionName][key] = value.as<std::vector<int>>();
                            } catch (...) {
                                try {
                                    m_config[sectionName][key] = value.as<std::vector<double>>();
                                } catch (...) {
                                    try {
                                        m_config[sectionName][key] = value.as<std::vector<bool>>();
                                    } catch (...) {
                                        m_config[sectionName][key] = value.as<std::vector<std::string>>();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        return true;
    } catch (const YAML::Exception& e) {
        std::cerr << "YAML parsing error: " << e.what() << std::endl;
        return false;
    } catch (const std::exception& e) {
        std::cerr << "Error loading config: " << e.what() << std::endl;
        return false;
    }
}

bool ConfigManager::saveToFile(const std::string& filename) {
    try {
        std::lock_guard<std::mutex> lock(m_mutex);
        
        YAML::Node root;
        
        // Convert our config structure to YAML
        for (const auto& [section, sectionData] : m_config) {
            YAML::Node sectionNode;
            for (const auto& [key, value] : sectionData) {
                std::visit([&sectionNode, &key](const auto& v) {
                    sectionNode[key] = v;
                }, value);
            }
            root[section] = sectionNode;
        }
        
        // Write to file
        std::ofstream file(filename);
        if (!file.is_open()) {
            std::cerr << "Failed to open config file for writing: " << filename << std::endl;
            return false;
        }
        
        file << YAML::Dump(root);
        return true;
    } catch (const YAML::Exception& e) {
        std::cerr << "YAML serialization error: " << e.what() << std::endl;
        return false;
    } catch (const std::exception& e) {
        std::cerr << "Error saving config: " << e.what() << std::endl;
        return false;
    }
}

bool ConfigManager::hasValue(const std::string& section, const std::string& key) const {
    std::lock_guard<std::mutex> lock(m_mutex);
    
    auto sectionIt = m_config.find(section);
    if (sectionIt == m_config.end()) {
        return false;
    }
    
    return sectionIt->second.find(key) != sectionIt->second.end();
}

int ConfigManager::registerCallback(const std::string& section, const std::string& key, 
                                    std::function<void(const ConfigValue&)> callback) {
    std::lock_guard<std::mutex> lock(m_mutex);
    
    int callbackId = m_nextCallbackId++;
    m_callbacks[callbackId] = {section, key, callback};
    
    // Trigger callback with current value if it exists
    if (hasValue(section, key)) {
        callback(m_config[section][key]);
    }
    
    return callbackId;
}

void ConfigManager::unregisterCallback(int callbackId) {
    std::lock_guard<std::mutex> lock(m_mutex);
    m_callbacks.erase(callbackId);
} 