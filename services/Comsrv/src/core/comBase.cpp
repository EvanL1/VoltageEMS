#include "core/comBase.h"
#include "core/metrics.h"
#include <iostream>
#include <fstream>

namespace voltage {
namespace comsrv {

ComBase::ComBase(const std::string& serviceName)
    : m_serviceName(serviceName)
    , m_running(false)
{
    std::cout << "Creating communication service: " << m_serviceName << std::endl;
}

ComBase::~ComBase()
{
    if (m_running) {
        stop();
    }
    std::cout << "Destroying communication service: " << m_serviceName << std::endl;
}

bool ComBase::loadConfig(const std::string& configFile)
{
    try {
        // Load YAML configuration
        YAML::Node config = YAML::LoadFile(configFile);
        
        // Validate root node
        if (!config.IsMap()) {
            m_lastError = "Invalid configuration file format";
            return false;
        }
        
        // Get channels configuration
        auto channels = config["channels"];
        if (!channels.IsSequence()) {
            m_lastError = "Missing or invalid channels configuration";
            return false;
        }
        
        // Process each channel
        for (const auto& channelNode : channels) {
            try {
                auto channelConfig = ChannelConfig::fromYaml(channelNode);
                if (!addChannel(channelConfig)) {
                    std::cerr << "Failed to add channel " << channelConfig.id 
                             << ": " << getLastError() << std::endl;
                }
            } catch (const YAML::Exception& e) {
                std::cerr << "Error parsing channel configuration: " << e.what() << std::endl;
            }
        }
        
        return true;
    } catch (const YAML::Exception& e) {
        m_lastError = "Failed to load configuration file: " + std::string(e.what());
        return false;
    }
}

bool ComBase::addChannel(const ChannelConfig& config)
{
    if (!validateChannelConfig(config)) {
        m_lastError = "Invalid channel configuration";
        return false;
    }

    std::lock_guard<std::mutex> lock(m_mutex);
    
    // Check if channel already exists
    if (m_channelConfigs.find(config.id) != m_channelConfigs.end()) {
        m_lastError = "Channel ID already exists: " + config.id;
        return false;
    }
    
    // Add channel configuration
    m_channelConfigs[config.id] = config;
    
    // Initialize channel status
    m_channelStatuses[config.id] = ChannelStatus(config.id);
    
    std::cout << "Added channel " << config.id << " to service " << m_serviceName << std::endl;
    return true;
}

bool ComBase::removeChannel(const std::string& channelId)
{
    std::lock_guard<std::mutex> lock(m_mutex);
    
    // Check if channel exists
    auto configIt = m_channelConfigs.find(channelId);
    if (configIt == m_channelConfigs.end()) {
        m_lastError = "Channel not found: " + channelId;
        return false;
    }
    
    // Remove channel configuration and status
    m_channelConfigs.erase(configIt);
    m_channelStatuses.erase(channelId);
    
    std::cout << "Removed channel " << channelId << " from service " << m_serviceName << std::endl;
    return true;
}

bool ComBase::isRunning() const
{
    return m_running;
}

std::string ComBase::getName() const
{
    return m_serviceName;
}

std::vector<ChannelConfig> ComBase::getChannelConfigs() const
{
    std::lock_guard<std::mutex> lock(m_mutex);
    std::vector<ChannelConfig> configs;
    configs.reserve(m_channelConfigs.size());
    
    for (const auto& [_, config] : m_channelConfigs) {
        configs.push_back(config);
    }
    
    return configs;
}

std::vector<ChannelStatus> ComBase::getChannelStatuses() const
{
    std::lock_guard<std::mutex> lock(m_mutex);
    std::vector<ChannelStatus> statuses;
    statuses.reserve(m_channelStatuses.size());
    
    for (const auto& [_, status] : m_channelStatuses) {
        statuses.push_back(status);
    }
    
    return statuses;
}

bool ComBase::hasError() const
{
    std::lock_guard<std::mutex> lock(m_mutex);
    return !m_lastError.empty();
}

std::string ComBase::getLastError() const
{
    std::lock_guard<std::mutex> lock(m_mutex);
    return m_lastError;
}

void ComBase::recordMetrics(const std::string& channelId, size_t bytesSent, size_t bytesReceived, double responseTime)
{
    auto& metrics = voltage::comsrv::Metrics::instance();
    
    // Get channel protocol type
    std::string protocolType;
    {
        std::lock_guard<std::mutex> lock(m_mutex);
        auto it = m_channelConfigs.find(channelId);
        if (it != m_channelConfigs.end()) {
            protocolType = it->second.protocol;
        }
    }
    
    if (protocolType.empty()) {
        return;
    }
    
    // Record bytes sent/received
    if (bytesSent > 0) {
        metrics.incrementBytesSent(protocolType, bytesSent);
        metrics.incrementPacketsSent(protocolType);
    }
    
    if (bytesReceived > 0) {
        metrics.incrementBytesReceived(protocolType, bytesReceived);
        metrics.incrementPacketsReceived(protocolType);
    }
    
    // Record response time
    if (responseTime > 0) {
        metrics.observePacketProcessingTime(protocolType, responseTime);
    }
    
    // Update channel metrics
    metrics.setChannelStatus(channelId, true);
    if (responseTime > 0) {
        metrics.setChannelResponseTime(channelId, responseTime);
    }
}

void ComBase::recordError(const std::string& errorType, const std::string& channelId)
{
    auto& metrics = voltage::comsrv::Metrics::instance();
    
    // Get channel protocol type
    std::string protocolType;
    if (!channelId.empty()) {
        std::lock_guard<std::mutex> lock(m_mutex);
        auto it = m_channelConfigs.find(channelId);
        if (it != m_channelConfigs.end()) {
            protocolType = it->second.protocol;
        }
    }
    
    // Record protocol error
    if (!protocolType.empty()) {
        metrics.incrementProtocolErrors(protocolType, errorType);
        
        // Update channel error status
        metrics.setChannelStatus(channelId, false);
        metrics.incrementChannelErrors(channelId, errorType);
    }
    
    std::lock_guard<std::mutex> lock(m_mutex);
    m_lastError = errorType;
}

void ComBase::updateChannelStatus(const std::string& channelId, bool connected, double responseTime, const std::string& error)
{
    // Update metrics
    auto& metrics = voltage::comsrv::Metrics::instance();
    metrics.setChannelStatus(channelId, connected);
    if (responseTime > 0) {
        metrics.setChannelResponseTime(channelId, responseTime);
    }
    
    if (!error.empty()) {
        metrics.incrementChannelErrors(channelId, error);
    }
    
    // Update channel status
    std::lock_guard<std::mutex> lock(m_mutex);
    auto it = m_channelStatuses.find(channelId);
    if (it != m_channelStatuses.end()) {
        it->second.connected = connected;
        it->second.lastResponseTime = responseTime;
        it->second.lastError = error;
        it->second.lastUpdateTime = std::chrono::steady_clock::now();
    }
}

} // namespace comsrv
} // namespace voltage 