#ifndef CONFIG_MANAGER_H
#define CONFIG_MANAGER_H

#include <string>
#include <map>
#include <memory>
#include <vector>
#include <variant>
#include <any>
#include <functional>
#include <mutex>

/**
 * @brief Configuration Manager class
 * 
 * This class provides a centralized configuration management system.
 * It supports loading and saving configurations from/to files, as well as
 * runtime configuration changes.
 */
class ConfigManager {
public:
    // Possible configuration value types
    using ConfigValue = std::variant<int, double, bool, std::string, std::vector<int>, 
                                     std::vector<double>, std::vector<bool>, std::vector<std::string>>;
    
    /**
     * @brief Get singleton instance
     * 
     * @return Reference to the singleton instance
     */
    static ConfigManager& getInstance();

    /**
     * @brief Load configuration from file
     * 
     * @param filename Path to the configuration file
     * @return true if loaded successfully, false otherwise
     */
    bool loadFromFile(const std::string& filename);

    /**
     * @brief Save configuration to file
     * 
     * @param filename Path to the configuration file
     * @return true if saved successfully, false otherwise
     */
    bool saveToFile(const std::string& filename);

    /**
     * @brief Set configuration value
     * 
     * @param section Section name
     * @param key Key name
     * @param value Configuration value
     */
    template<typename T>
    void setValue(const std::string& section, const std::string& key, const T& value);

    /**
     * @brief Get configuration value
     * 
     * @param section Section name
     * @param key Key name
     * @param defaultValue Default value to return if not found
     * @return Configuration value or defaultValue if not found
     */
    template<typename T>
    T getValue(const std::string& section, const std::string& key, const T& defaultValue) const;

    /**
     * @brief Check if configuration exists
     * 
     * @param section Section name
     * @param key Key name
     * @return true if exists, false otherwise
     */
    bool hasValue(const std::string& section, const std::string& key) const;

    /**
     * @brief Register a callback for configuration changes
     * 
     * @param section Section name
     * @param key Key name
     * @param callback Callback function
     * @return Callback ID for unregistering
     */
    int registerCallback(const std::string& section, const std::string& key, 
                         std::function<void(const ConfigValue&)> callback);

    /**
     * @brief Unregister a callback
     * 
     * @param callbackId Callback ID returned by registerCallback
     */
    void unregisterCallback(int callbackId);

private:
    // Private constructor for singleton
    ConfigManager();
    ~ConfigManager();

    // Disable copy and move
    ConfigManager(const ConfigManager&) = delete;
    ConfigManager& operator=(const ConfigManager&) = delete;
    ConfigManager(ConfigManager&&) = delete;
    ConfigManager& operator=(ConfigManager&&) = delete;

    // Configuration storage
    std::map<std::string, std::map<std::string, ConfigValue>> m_config;
    
    // Callback storage
    struct CallbackInfo {
        std::string section;
        std::string key;
        std::function<void(const ConfigValue&)> callback;
    };
    std::map<int, CallbackInfo> m_callbacks;
    int m_nextCallbackId;

    // Thread safety
    mutable std::mutex m_mutex;
};

// Template implementations
template<typename T>
void ConfigManager::setValue(const std::string& section, const std::string& key, const T& value) {
    std::lock_guard<std::mutex> lock(m_mutex);
    m_config[section][key] = value;

    // Notify callbacks
    for (const auto& [id, info] : m_callbacks) {
        if (info.section == section && info.key == key) {
            info.callback(m_config[section][key]);
        }
    }
}

template<typename T>
T ConfigManager::getValue(const std::string& section, const std::string& key, const T& defaultValue) const {
    std::lock_guard<std::mutex> lock(m_mutex);
    
    // Check if section exists
    auto sectionIt = m_config.find(section);
    if (sectionIt == m_config.end()) {
        return defaultValue;
    }
    
    // Check if key exists
    auto keyIt = sectionIt->second.find(key);
    if (keyIt == sectionIt->second.end()) {
        return defaultValue;
    }
    
    // Try to get value of the correct type
    try {
        return std::get<T>(keyIt->second);
    } catch (const std::bad_variant_access&) {
        return defaultValue;
    }
}

#endif // CONFIG_MANAGER_H 