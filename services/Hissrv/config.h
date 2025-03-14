#ifndef CONFIG_H
#define CONFIG_H

#include <string>
#include <vector>
#include <utility>

// Configuration structure
struct Config {
    // Redis configuration
    std::string redis_host = "127.0.0.1";
    int redis_port = 6379;
    std::string redis_password = "";
    std::string redis_key_pattern = "*";  // Default to get all keys
    std::string redis_socket = "/var/run/redis/redis.sock";        // Unix socket path, if empty use TCP

    // InfluxDB configuration
    std::string influxdb_url = "http://localhost:8086";
    std::string influxdb_db = "mydb";
    std::string influxdb_user = "";
    std::string influxdb_password = "";
    
    // Program configuration
    int interval_seconds = 10;  // Data synchronization interval
    bool verbose = false;       // Whether to output detailed logs
    bool enable_influxdb = true; // Whether to enable InfluxDB writing
    int retention_days = 30;    // Data retention days
    std::string config_file = "hissrv.conf"; // Configuration file path
    
    // Point storage configuration
    std::vector<std::pair<std::string, bool>> point_storage_patterns;
    bool default_point_storage = true;
};

// Parse configuration file
bool parseConfigFile(const std::string& filename, Config& config);

// Parse command line arguments
Config parseArgs(int argc, char* argv[]);

// Check if a point should be stored based on configuration
bool shouldStorePoint(const std::string& key, const Config& config);

// Check if configuration file has changed
bool configFileChanged(const std::string& filename, std::time_t& lastModTime);

// Print help message
void printHelp(const char* programName);

#endif // CONFIG_H 