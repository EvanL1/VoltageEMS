#include <iostream>
#include <chrono>
#include <thread>
#include "config.h"
#include "redis_handler.h"
#include "influxdb_handler.h"

int main(int argc, char* argv[]) {
    // Parse command line arguments
    Config config = parseArgs(argc, argv);
    
    // Record the last modification time of the configuration file
    std::time_t lastConfigModTime = 0;
    
    try {
        // Connect to Redis
        RedisConnection redis;
        if (!redis.connect(config)) {
            std::cerr << "Failed to connect to Redis. Exiting." << std::endl;
            return 1;
        }
        
        // Connect to InfluxDB (if enabled)
        std::shared_ptr<influxdb::InfluxDB> influxdb = connectToInfluxDB(config);
        
        std::cout << "Starting data transfer service..." << std::endl;
        if (config.enable_influxdb) {
            std::cout << "Data will be transferred from Redis to InfluxDB every " 
                      << config.interval_seconds << " seconds." << std::endl;
            std::cout << "Default point storage policy: " 
                      << (config.default_point_storage ? "Store" : "Ignore") << std::endl;
            std::cout << "Number of specific point patterns: " 
                      << config.point_storage_patterns.size() << std::endl;
        } else {
            std::cout << "Data transfer to InfluxDB is currently disabled." << std::endl;
        }
        std::cout << "Press Ctrl+C to stop" << std::endl;
        
        // Main loop
        while (true) {
            // Check if configuration file has changed
            if (configFileChanged(config.config_file, lastConfigModTime)) {
                std::cout << "Configuration file changed. Reloading..." << std::endl;
                
                // Save old configuration to detect critical changes
                bool oldEnableInfluxDB = config.enable_influxdb;
                int oldRetentionDays = config.retention_days;
                std::string oldInfluxDBUrl = config.influxdb_url;
                std::string oldInfluxDBDB = config.influxdb_db;
                std::string oldRedisHost = config.redis_host;
                int oldRedisPort = config.redis_port;
                std::string oldRedisPassword = config.redis_password;
                std::string oldRedisSocket = config.redis_socket;
                
                // Reload configuration
                Config newConfig = config;  // Keep command line parameters
                if (parseConfigFile(config.config_file, newConfig)) {
                    config = newConfig;
                    
                    // Check if Redis connection settings have changed
                    bool reconnectRedis = false;
                    if (oldRedisHost != config.redis_host || 
                        oldRedisPort != config.redis_port || 
                        oldRedisPassword != config.redis_password ||
                        oldRedisSocket != config.redis_socket) {
                        std::cout << "Redis connection settings changed. Reconnecting..." << std::endl;
                        reconnectRedis = true;
                    }
                    
                    // Reconnect to Redis if needed
                    if (reconnectRedis) {
                        if (!redis.connect(config)) {
                            std::cerr << "Failed to reconnect to Redis with new settings." << std::endl;
                        }
                    }
                    
                    // If InfluxDB settings have changed, need to reconnect
                    bool reconnectInfluxDB = false;
                    
                    if (!config.enable_influxdb && oldEnableInfluxDB) {
                        std::cout << "InfluxDB writing has been disabled." << std::endl;
                        influxdb.reset();  // Release InfluxDB connection
                    } else if (config.enable_influxdb && !oldEnableInfluxDB) {
                        std::cout << "InfluxDB writing has been enabled." << std::endl;
                        reconnectInfluxDB = true;
                    } else if (config.enable_influxdb && 
                              (oldInfluxDBUrl != config.influxdb_url || oldInfluxDBDB != config.influxdb_db)) {
                        std::cout << "InfluxDB connection settings changed." << std::endl;
                        reconnectInfluxDB = true;
                    } else if (config.enable_influxdb && oldRetentionDays != config.retention_days) {
                        std::cout << "Retention policy changed from " << oldRetentionDays 
                                  << " to " << config.retention_days << " days." << std::endl;
                        // Update retention policy
                        if (influxdb) {
                            createRetentionPolicy(influxdb, config.influxdb_db, config.retention_days);
                        }
                    }
                    
                    // If need to reconnect to InfluxDB
                    if (reconnectInfluxDB) {
                        influxdb = connectToInfluxDB(config);
                    }
                    
                    // Log point storage configuration changes
                    std::cout << "Updated point storage configuration. Default: " 
                              << (config.default_point_storage ? "Store" : "Ignore") 
                              << ", Patterns: " << config.point_storage_patterns.size() << std::endl;
                }
            }
            
            // Process Redis data and write to InfluxDB
            processRedisData(redis, influxdb, config);
            
            // Wait for next sync cycle
            std::this_thread::sleep_for(std::chrono::seconds(config.interval_seconds));
        }
        
    } catch (const std::exception& e) {
        std::cerr << "Fatal error: " << e.what() << std::endl;
        return 1;
    }
    
    return 0;
} 