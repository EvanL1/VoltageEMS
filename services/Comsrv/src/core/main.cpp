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
#include <functional>

#include "core/comBase.h"
#include "core/config/configManager.h"
#include "core/protocolFactory.h"
#include "core/protocols/modbus/modbusMaster.h"
#include "core/protocols/modbus/modbusRTUMaster.h"
#include "core/metrics.h"
#include <yaml-cpp/yaml.h>

// Global flag for program termination
std::atomic<bool> g_running(true);

// Signal handler for graceful shutdown
void signalHandler(int signal) {
    std::cout << "Received signal: " << signal << std::endl;
    g_running = false;
}

int main(int argc, char* argv[]) {
    try {
        // Initialize metrics
        auto& metrics = voltage::comsrv::Metrics::instance();
        metrics.init("0.0.0.0:9100");
        
        // Register signal handlers
        std::signal(SIGINT, signalHandler);
        std::signal(SIGTERM, signalHandler);
        
        std::cout << "Comsrv starting..." << std::endl;
        
        // Register all protocol types
        auto& factory = ProtocolFactory::getInstance();
        int protocolCount = factory.registerSupportedProtocols();
        std::cout << "Registered " << protocolCount << " protocol types" << std::endl;
        
        // Load configuration
        std::string configFile = "config/comsrv.yaml";
        if (argc > 1) {
            configFile = argv[1];
        }
        
        // Create protocol instances from configuration
        std::vector<std::unique_ptr<voltage::comsrv::ComBase>> protocols = factory.createProtocolsFromConfig(configFile);
        
        if (protocols.empty()) {
            std::cerr << "No protocols created from configuration" << std::endl;
            metrics.incrementProtocolErrors("all", "no_protocols_created");
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
            for (const auto& protocol : protocols) {
                // Update protocol metrics
                metrics.setProtocolStatus(protocol->getName(), protocol->isRunning());
                if (protocol->hasError()) {
                    metrics.incrementProtocolErrors(protocol->getName(), protocol->getLastError());
                }
                
                // Update channel metrics for each channel
                for (const auto& channel : protocol->getChannelStatuses()) {
                    metrics.setChannelStatus(channel.getId(), channel.isConnected());
                    metrics.setChannelResponseTime(channel.getId(), channel.getLastResponseTime());
                    if (channel.hasError()) {
                        metrics.incrementChannelErrors(channel.getId(), channel.getLastError());
                    }
                }
            }
            
            std::this_thread::sleep_for(std::chrono::milliseconds(100));
        }
        
        // Stop all protocols
        for (auto& protocol : protocols) {
            protocol->stop();
            metrics.setProtocolStatus(protocol->getName(), false);
        }
        
        std::cout << "Comsrv shutting down..." << std::endl;
        return 0;
    }
    catch (const std::exception& e) {
        std::cerr << "Fatal error: " << e.what() << std::endl;
        return 1;
    }
} 