#include "core/config/configManager.h"
#include <fstream>
#include <iostream>
#include <nlohmann/json.hpp>

// For convenience
using json = nlohmann::json;

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
    std::ifstream file(filename);
    if (!file.is_open()) {
        std::cerr << "Failed to open config file: " << filename << std::endl;
        return false;
    }

    try {
        json j;
        file >> j;
        
        std::lock_guard<std::mutex> lock(m_mutex);
        
        // Clear existing config
        m_config.clear();
        
        // Parse JSON into our config structure
        for (auto& [section, sectionData] : j.items()) {
            for (auto& [key, value] : sectionData.items()) {
                if (value.is_number_integer()) {
                    m_config[section][key] = value.get<int>();
                } else if (value.is_number_float()) {
                    m_config[section][key] = value.get<double>();
                } else if (value.is_boolean()) {
                    m_config[section][key] = value.get<bool>();
                } else if (value.is_string()) {
                    m_config[section][key] = value.get<std::string>();
                } else if (value.is_array()) {
                    if (value.size() > 0) {
                        if (value[0].is_number_integer()) {
                            m_config[section][key] = value.get<std::vector<int>>();
                        } else if (value[0].is_number_float()) {
                            m_config[section][key] = value.get<std::vector<double>>();
                        } else if (value[0].is_boolean()) {
                            m_config[section][key] = value.get<std::vector<bool>>();
                        } else if (value[0].is_string()) {
                            m_config[section][key] = value.get<std::vector<std::string>>();
                        }
                    }
                }
            }
        }
        
        return true;
    } catch (const json::exception& e) {
        std::cerr << "JSON parsing error: " << e.what() << std::endl;
        return false;
    } catch (const std::exception& e) {
        std::cerr << "Error loading config: " << e.what() << std::endl;
        return false;
    }
}

bool ConfigManager::saveToFile(const std::string& filename) {
    try {
        std::lock_guard<std::mutex> lock(m_mutex);
        
        json j;
        
        // Convert our config structure to JSON
        for (const auto& [section, sectionData] : m_config) {
            for (const auto& [key, value] : sectionData) {
                std::visit([&j, &section, &key](const auto& v) {
                    j[section][key] = v;
                }, value);
            }
        }
        
        // Write to file
        std::ofstream file(filename);
        if (!file.is_open()) {
            std::cerr << "Failed to open config file for writing: " << filename << std::endl;
            return false;
        }
        
        file << j.dump(4); // Pretty print with 4 spaces
        return true;
    } catch (const json::exception& e) {
        std::cerr << "JSON serialization error: " << e.what() << std::endl;
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