#include "comBase.h"
#include "logger.h"
#include "protocols/modbus/modbusMaster.h"
#include "protocols/modbus/modbusSlave.h"
#include <iostream>
#include <fstream>
#include <thread>
#include <chrono>
#include <signal.h>
#include <json/json.h>
#include <map>
#include <memory>
#include <atomic>
#include <vector>
#include <string>
#include <cstring>
#include <csignal>

using namespace Communication;

// Global flag for signaling application shutdown
std::atomic<bool> g_running(true);

// Signal handler for graceful shutdown
void signalHandler(int signum) {
    std::cout << "Interrupt signal (" << signum << ") received. Shutting down gracefully..." << std::endl;
    g_running = false;
}

// Print program usage
void printUsage(const char* programName) {
    std::cout << "Usage: " << programName << " [OPTIONS]" << std::endl;
    std::cout << "Options:" << std::endl;
    std::cout << "  -h, --help                 Display this help message" << std::endl;
    std::cout << "  -c, --config <directory>   Specify configuration directory (default: /etc/comsrv)" << std::endl;
    std::cout << "  -l, --log <directory>      Specify log directory (default: /var/log/comsrv)" << std::endl;
    std::cout << "  -v, --verbose              Enable verbose output" << std::endl;
    std::cout << "  -d, --daemon               Run as daemon" << std::endl;
}

// Storage for all communication channels
struct CommunicationSystem {
    std::map<int, std::unique_ptr<ComBase>> channels;
    
    // Add a new channel
    bool addChannel(int index, std::unique_ptr<ComBase> channel) {
        if (channels.find(index) != channels.end()) {
            // Channel with this index already exists
            return false;
        }
        channels[index] = std::move(channel);
        return true;
    }
    
    // Remove a channel
    bool removeChannel(int index) {
        if (channels.find(index) == channels.end()) {
            // Channel with this index doesn't exist
            return false;
        }
        channels.erase(index);
        return true;
    }
    
    // Start all channels
    void startAll() {
        for (auto& pair : channels) {
            pair.second->start();
        }
    }
    
    // Stop all channels
    void stopAll() {
        for (auto& pair : channels) {
            pair.second->stop();
        }
    }
    
    // Get status of all channels
    std::string getStatus() const {
        std::stringstream ss;
        ss << "Communication System Status:" << std::endl;
        for (const auto& pair : channels) {
            ss << "Channel " << pair.first << ": " << pair.second->getStatus() << std::endl;
        }
        return ss.str();
    }
};

int main(int argc, char* argv[]) {
    // Default configuration and log directories
    std::string configDir = "/etc/comsrv";
    std::string logDir = "/var/log/comsrv";
    bool verbose = false;
    bool runAsDaemon = false;
    
    // Parse command line arguments
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "-h") == 0 || strcmp(argv[i], "--help") == 0) {
            printUsage(argv[0]);
            return 0;
        } else if (strcmp(argv[i], "-c") == 0 || strcmp(argv[i], "--config") == 0) {
            if (i + 1 < argc) {
                configDir = argv[++i];
            } else {
                std::cerr << "Error: Missing configuration directory after -c/--config" << std::endl;
                return 1;
            }
        } else if (strcmp(argv[i], "-l") == 0 || strcmp(argv[i], "--log") == 0) {
            if (i + 1 < argc) {
                logDir = argv[++i];
            } else {
                std::cerr << "Error: Missing log directory after -l/--log" << std::endl;
                return 1;
            }
        } else if (strcmp(argv[i], "-v") == 0 || strcmp(argv[i], "--verbose") == 0) {
            verbose = true;
        } else if (strcmp(argv[i], "-d") == 0 || strcmp(argv[i], "--daemon") == 0) {
            runAsDaemon = true;
        } else {
            std::cerr << "Error: Unknown option '" << argv[i] << "'" << std::endl;
            printUsage(argv[0]);
            return 1;
        }
    }
    
    // Set up signal handlers for graceful shutdown
    signal(SIGINT, signalHandler);
    signal(SIGTERM, signalHandler);
    
    // Initialize logger
    Logger& logger = Logger::getInstance();
    logger.setLogLevel(verbose ? LogLevel::DEBUG : LogLevel::INFO);
    
    // Configure file logging
    std::string logFilePath = logDir + "/comsrv.log";
    logger.enableLogToFile(true, logFilePath);
    logger.setLogRetention(30); // 30 days retention
    logger.setMaxLogEntries(10000);
    
    logger.logInfo("main", "Communication Server starting up", 
                  "{\"config\":\"" + configDir + "\",\"log\":\"" + logDir + "\"}");
    
    // Initialize configuration manager
    ConfigManager& configManager = ConfigManager::getInstance();
    if (!configManager.init(configDir)) {
        logger.logCritical("main", "Failed to initialize configuration manager", 
                        "{\"configDir\":\"" + configDir + "\"}");
        return 1;
    }
    
    // Load channel configuration
    std::string channelConfigFile = configDir + "/channels.json";
    if (!configManager.loadChannelConfig(channelConfigFile)) {
        logger.logCritical("main", "Failed to load channel configuration", 
                         "{\"file\":\"" + channelConfigFile + "\"}");
        return 1;
    }
    
    // Create communication system
    CommunicationSystem comSystem;
    
    // Get channel configurations
    std::vector<ChannelConfig> channelConfigs = configManager.getChannelConfigs();
    
    logger.logInfo("main", "Loaded channel configurations", 
                 "{\"count\":" + std::to_string(channelConfigs.size()) + "}");
    
    // Set up Redis connection based on global configuration
    std::string redisHost = "localhost";
    int redisPort = 6379;
    
    // Initialize channels
    for (const auto& config : channelConfigs) {
        // Only process enabled channels
        if (!config.enabled) {
            logger.logInfo("main", "Skipping disabled channel", 
                         "{\"index\":" + std::to_string(config.index) + 
                         ",\"name\":\"" + config.name + "\"}");
            continue;
        }
        
        std::unique_ptr<ComBase> channel;
        
        // Create appropriate channel based on protocol type and role
        if (config.protocolType == ProtocolType::MODBUS) {
            if (config.deviceRole == DeviceRole::MASTER) {
                channel = createModbusMaster(config.physicalInterfaceType);
            } else {
                channel = createModbusSlave(config.physicalInterfaceType);
            }
        } else {
            logger.logWarning("main", "Unsupported protocol type", 
                            "{\"index\":" + std::to_string(config.index) + 
                            ",\"protocol\":\"" + std::to_string(static_cast<int>(config.protocolType)) + "\"}");
            continue;
        }
        
        if (!channel) {
            logger.logError("main", "Failed to create channel", 
                          "{\"index\":" + std::to_string(config.index) + 
                          ",\"name\":\"" + config.name + "\"}");
            continue;
        }
        
        // Convert protocol configuration to JSON string for initialization
        Json::Value protocolJson;
        Json::FastWriter writer;
        
        // Extract protocol configuration based on interface type
        if (config.physicalInterfaceType == PhysicalInterfaceType::NETWORK) {
            // For TCP connections
            const auto& tcpConfig = std::get<ModbusTCPConfig>(config.protocolConfig);
            protocolJson["host"] = tcpConfig.ip;
            protocolJson["port"] = tcpConfig.port;
            // Add other TCP-specific configurations
        } else if (config.physicalInterfaceType == PhysicalInterfaceType::SERIAL) {
            // For RTU connections
            const auto& rtuConfig = std::get<ModbusRTUConfig>(config.protocolConfig);
            protocolJson["serialPort"] = rtuConfig.serialPort;
            protocolJson["baudRate"] = rtuConfig.baudRate;
            protocolJson["parity"] = std::string(1, rtuConfig.parity);
            protocolJson["dataBits"] = rtuConfig.dataBits;
            protocolJson["stopBits"] = rtuConfig.stopBits;
            // Add other RTU-specific configurations
        }
        
        // Add common configurations
        protocolJson["timeout"] = config.pollRate;
        protocolJson["debug"] = verbose;
        
        // Convert to string
        std::string configStr = writer.write(protocolJson);
        
        // Initialize channel
        if (!channel->init(configStr)) {
            logger.logError("main", "Failed to initialize channel", 
                          "{\"index\":" + std::to_string(config.index) + 
                          ",\"name\":\"" + config.name + "\"}");
            continue;
        }
        
        // Connect to Redis
        if (!channel->connectToRedis(redisHost, redisPort)) {
            logger.logWarning("main", "Failed to connect to Redis", 
                            "{\"host\":\"" + redisHost + "\",\"port\":" + std::to_string(redisPort) + "}");
        }
        
        // Add the channel to our system
        comSystem.addChannel(config.index, std::move(channel));
        
        logger.logInfo("main", "Channel initialized successfully", 
                     "{\"index\":" + std::to_string(config.index) + 
                     ",\"name\":\"" + config.name + "\"}");
    }
    
    // Set up configuration monitoring for hot-reload
    configManager.setConfigChangeCallback([&](int channelIndex) {
        logger.logInfo("main", "Channel configuration changed, reloading", 
                      "{\"index\":" + std::to_string(channelIndex) + "}");
        
        auto it = comSystem.channels.find(channelIndex);
        if (it != comSystem.channels.end()) {
            it->second->reconfigureChannel(channelIndex);
        }
    });
    
    configManager.startConfigMonitoring();
    
    // Start all communication channels
    logger.logInfo("main", "Starting all communication channels", "{}");
    comSystem.startAll();
    
    // Main application loop
    logger.logInfo("main", "Communication server running", "{}");
    
    // Simple status reporting at regular intervals if verbose
    std::thread statusThread;
    if (verbose) {
        statusThread = std::thread([&]() {
            while (g_running) {
                // Every 30 seconds, log the system status
                std::this_thread::sleep_for(std::chrono::seconds(30));
                logger.logInfo("main", "System status", comSystem.getStatus());
            }
        });
    }
    
    // Main loop - wait for shutdown signal
    while (g_running) {
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
    }
    
    // Cleanup
    logger.logInfo("main", "Shutting down communication server", "{}");
    
    // Stop configuration monitoring
    configManager.stopConfigMonitoring();
    
    // Stop all channels
    comSystem.stopAll();
    
    // Wait for status thread to join if it was started
    if (verbose && statusThread.joinable()) {
        statusThread.join();
    }
    
    logger.logInfo("main", "Communication server stopped", "{}");
    
    return 0;
} 