#include <iostream>
#include <string>
#include <vector>
#include <memory>
#include <thread>
#include <chrono>
#include <fstream>
#include <sstream>
#include <map>
#include <mutex>
#include <condition_variable>
#include <atomic>
#include <csignal>

#include "core/comBase.h"
#include "core/config/configManager.h"
#include "core/protocolFactory.h"
#include "core/protocols/modbus/modbusMaster.h"
#include "core/protocols/modbus/modbusRTUMaster.h"

// Global flag for program termination
std::atomic<bool> g_running(true);

// Signal handler for graceful shutdown
void signalHandler(int signal) {
    std::cout << "Received signal: " << signal << std::endl;
    g_running = false;
}

// Register all protocol creators
void registerProtocols() {
    auto& factory = ProtocolFactory::getInstance();
    
    // Register ModbusRTUMaster creator
    factory.registerProtocol("modbus_rtu_master", [](const std::map<std::string, ConfigManager::ConfigValue>& config) -> std::unique_ptr<ComBase> {
        try {
            std::string portName = std::get<std::string>(config.at("port"));
            int baudRate = std::get<int>(config.at("baudrate"));
            int dataBits = std::get<int>(config.at("databits"));
            
            SerialParity parity = SerialParity::NONE;
            std::string parityStr = std::get<std::string>(config.at("parity"));
            if (parityStr == "odd") {
                parity = SerialParity::ODD;
            } else if (parityStr == "even") {
                parity = SerialParity::EVEN;
            }
            
            int stopBits = std::get<int>(config.at("stopbits"));
            int timeout = std::get<int>(config.at("timeout"));
            
            return std::make_unique<ModbusRTUMaster>(portName, baudRate, dataBits, parity, stopBits, timeout);
        } catch (const std::exception& e) {
            std::cerr << "Error creating ModbusRTUMaster: " << e.what() << std::endl;
            return nullptr;
        }
    });
    
    // Register other protocol creators here
}

int main(int argc, char* argv[]) {
    // Register signal handlers
    std::signal(SIGINT, signalHandler);
    std::signal(SIGTERM, signalHandler);
    
    std::cout << "Comsrv starting..." << std::endl;
    
    // Register all protocol creators
    registerProtocols();
    
    // Load configuration
    std::string configFile = "comsrv.json";
    if (argc > 1) {
        configFile = argv[1];
    }
    
    // Create protocol instances from configuration
    auto& factory = ProtocolFactory::getInstance();
    std::vector<std::unique_ptr<ComBase>> protocols = factory.createProtocolsFromConfig(configFile);
    
    if (protocols.empty()) {
        std::cerr << "No protocols created from configuration" << std::endl;
        return 1;
    }
    
    std::cout << "Created " << protocols.size() << " protocol instances" << std::endl;
    
    // Start all protocols
    for (auto& protocol : protocols) {
        protocol->start();
    }
    
    // Main loop
    while (g_running) {
        // Process commands, monitor status, etc.
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
    }
    
    // Stop all protocols
    for (auto& protocol : protocols) {
        protocol->stop();
    }
    
    std::cout << "Comsrv shutting down..." << std::endl;
    return 0;
} 