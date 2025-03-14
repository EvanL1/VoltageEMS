#include "config.h"
#include <iostream>
#include <fstream>
#include <sstream>
#include <regex>
#include <sys/stat.h>

bool parseConfigFile(const std::string& filename, Config& config) {
    std::ifstream file(filename);
    if (!file.is_open()) {
        std::cerr << "Warning: Could not open config file: " << filename << std::endl;
        return false;
    }

    // Clear existing point storage patterns
    config.point_storage_patterns.clear();

    std::string line;
    while (std::getline(file, line)) {
        // Skip comments and empty lines
        if (line.empty() || line[0] == '#') {
            continue;
        }

        std::istringstream iss(line);
        std::string key, value;
        
        if (std::getline(iss, key, '=') && std::getline(iss, value)) {
            // Remove leading and trailing spaces
            key.erase(0, key.find_first_not_of(" \t"));
            key.erase(key.find_last_not_of(" \t") + 1);
            value.erase(0, value.find_first_not_of(" \t"));
            value.erase(value.find_last_not_of(" \t") + 1);
            
            if (key == "redis_host") {
                config.redis_host = value;
            } else if (key == "redis_port") {
                config.redis_port = std::stoi(value);
            } else if (key == "redis_password") {
                config.redis_password = value;
            } else if (key == "redis_key_pattern") {
                config.redis_key_pattern = value;
            } else if (key == "redis_socket") {
                config.redis_socket = value;
            } else if (key == "influxdb_url") {
                config.influxdb_url = value;
            } else if (key == "influxdb_db") {
                config.influxdb_db = value;
            } else if (key == "influxdb_user") {
                config.influxdb_user = value;
            } else if (key == "influxdb_password") {
                config.influxdb_password = value;
            } else if (key == "interval_seconds") {
                config.interval_seconds = std::stoi(value);
            } else if (key == "verbose") {
                config.verbose = (value == "true" || value == "1" || value == "yes");
            } else if (key == "enable_influxdb") {
                config.enable_influxdb = (value == "true" || value == "1" || value == "yes");
            } else if (key == "retention_days") {
                config.retention_days = std::stoi(value);
            } else if (key == "default_point_storage") {
                config.default_point_storage = (value == "true" || value == "1" || value == "yes");
            } else if (key == "point_storage") {
                // Parse point storage configuration
                // Format: point_pattern:true/false
                size_t colonPos = value.find_last_of(':');
                if (colonPos != std::string::npos) {
                    std::string pattern = value.substr(0, colonPos);
                    std::string storageStr = value.substr(colonPos + 1);
                    bool storage = (storageStr == "true" || storageStr == "1" || storageStr == "yes");
                    
                    config.point_storage_patterns.push_back(std::make_pair(pattern, storage));
                    
                    if (config.verbose) {
                        std::cout << "Added point storage pattern: " << pattern 
                                  << " -> " << (storage ? "store" : "ignore") << std::endl;
                    }
                }
            }
        }
    }

    return true;
}

Config parseArgs(int argc, char* argv[]) {
    Config config;
    
    for (int i = 1; i < argc; i++) {
        std::string arg = argv[i];
        
        if (arg == "--config" && i + 1 < argc) {
            config.config_file = argv[++i];
            // Parse config file first, then command line arguments can override settings
            parseConfigFile(config.config_file, config);
        } else if (arg == "--redis-host" && i + 1 < argc) {
            config.redis_host = argv[++i];
        } else if (arg == "--redis-port" && i + 1 < argc) {
            config.redis_port = std::stoi(argv[++i]);
        } else if (arg == "--redis-password" && i + 1 < argc) {
            config.redis_password = argv[++i];
        } else if (arg == "--redis-key-pattern" && i + 1 < argc) {
            config.redis_key_pattern = argv[++i];
        } else if (arg == "--redis-socket" && i + 1 < argc) {
            config.redis_socket = argv[++i];
        } else if (arg == "--influxdb-url" && i + 1 < argc) {
            config.influxdb_url = argv[++i];
        } else if (arg == "--influxdb-db" && i + 1 < argc) {
            config.influxdb_db = argv[++i];
        } else if (arg == "--influxdb-user" && i + 1 < argc) {
            config.influxdb_user = argv[++i];
        } else if (arg == "--influxdb-password" && i + 1 < argc) {
            config.influxdb_password = argv[++i];
        } else if (arg == "--interval" && i + 1 < argc) {
            config.interval_seconds = std::stoi(argv[++i]);
        } else if (arg == "--verbose") {
            config.verbose = true;
        } else if (arg == "--enable-influxdb") {
            config.enable_influxdb = true;
        } else if (arg == "--disable-influxdb") {
            config.enable_influxdb = false;
        } else if (arg == "--retention-days" && i + 1 < argc) {
            config.retention_days = std::stoi(argv[++i]);
        } else if (arg == "--help") {
            printHelp(argv[0]);
            exit(0);
        }
    }
    
    return config;
}

bool shouldStorePoint(const std::string& key, const Config& config) {
    // If global InfluxDB writing is disabled, don't store any points
    if (!config.enable_influxdb) {
        return false;
    }
    
    // Check against specific patterns
    for (const auto& pattern : config.point_storage_patterns) {
        // Convert Redis glob pattern to regex
        std::string regexPattern = pattern.first;
        // Replace Redis glob wildcards with regex equivalents
        size_t pos = 0;
        while ((pos = regexPattern.find('*', pos)) != std::string::npos) {
            regexPattern.replace(pos, 1, ".*");
            pos += 2;
        }
        pos = 0;
        while ((pos = regexPattern.find('?', pos)) != std::string::npos) {
            regexPattern.replace(pos, 1, ".");
            pos += 1;
        }
        
        // Check if key matches pattern
        std::regex regex(regexPattern);
        if (std::regex_match(key, regex)) {
            return pattern.second;  // Return whether this pattern says to store or not
        }
    }
    
    // If no pattern matched, use default
    return config.default_point_storage;
}

bool configFileChanged(const std::string& filename, std::time_t& lastModTime) {
    struct stat fileInfo;
    
    if (stat(filename.c_str(), &fileInfo) != 0) {
        return false;  // File doesn't exist or can't be accessed
    }
    
    if (fileInfo.st_mtime > lastModTime) {
        lastModTime = fileInfo.st_mtime;
        return true;
    }
    
    return false;
}

void printHelp(const char* programName) {
    std::cout << "Usage: " << programName << " [OPTIONS]\n"
              << "Options:\n"
              << "  --config FILE               Configuration file path\n"
              << "  --redis-host HOST           Redis host (default: 127.0.0.1)\n"
              << "  --redis-port PORT           Redis port (default: 6379)\n"
              << "  --redis-password PASS       Redis password\n"
              << "  --redis-key-pattern PATTERN Redis key pattern to match (default: *)\n"
              << "  --redis-socket PATH         Redis Unix socket path (if specified, TCP is not used)\n"
              << "  --influxdb-url URL          InfluxDB URL (default: http://localhost:8086)\n"
              << "  --influxdb-db DB            InfluxDB database name (default: mydb)\n"
              << "  --influxdb-user USER        InfluxDB username\n"
              << "  --influxdb-password PASS    InfluxDB password\n"
              << "  --interval SECONDS          Sync interval in seconds (default: 10)\n"
              << "  --verbose                   Enable verbose logging\n"
              << "  --enable-influxdb           Enable writing to InfluxDB (default)\n"
              << "  --disable-influxdb          Disable writing to InfluxDB\n"
              << "  --retention-days DAYS       Data retention period in days (default: 30)\n"
              << "  --help                      Show this help message\n";
} 