#include "influxdb_handler.h"
#include <iostream>

void createRetentionPolicy(std::shared_ptr<influxdb::InfluxDB> influxdb, const std::string& dbName, int retentionDays) {
    try {
        std::string query = "CREATE RETENTION POLICY \"" + dbName + "_retention\" ON \"" + 
                           dbName + "\" DURATION " + std::to_string(retentionDays) + "d REPLICATION 1 DEFAULT";
        
        influxdb->query(query);
        std::cout << "Created retention policy: " << retentionDays << " days" << std::endl;
    } catch (const std::exception& e) {
        // If policy already exists, try to update it
        try {
            std::string query = "ALTER RETENTION POLICY \"" + dbName + "_retention\" ON \"" + 
                               dbName + "\" DURATION " + std::to_string(retentionDays) + "d REPLICATION 1 DEFAULT";
            
            influxdb->query(query);
            std::cout << "Updated retention policy: " << retentionDays << " days" << std::endl;
        } catch (const std::exception& e2) {
            std::cerr << "Error setting retention policy: " << e2.what() << std::endl;
        }
    }
}

std::shared_ptr<influxdb::InfluxDB> connectToInfluxDB(Config& config) {
    std::shared_ptr<influxdb::InfluxDB> influxdb;
    
    if (!config.enable_influxdb) {
        std::cout << "InfluxDB writing is disabled by configuration." << std::endl;
        return influxdb;
    }
    
    try {
        std::string influxdb_connection_string = config.influxdb_url;
        if (!config.influxdb_user.empty() && !config.influxdb_password.empty()) {
            influxdb_connection_string += "?db=" + config.influxdb_db + 
                                         "&u=" + config.influxdb_user + 
                                         "&p=" + config.influxdb_password;
        } else {
            influxdb_connection_string += "?db=" + config.influxdb_db;
        }
        
        influxdb = influxdb::InfluxDBFactory::Get(influxdb_connection_string);
        
        // Test InfluxDB connection
        influxdb->ping();
        std::cout << "Successfully connected to InfluxDB at " << config.influxdb_url << std::endl;
        
        // Set data retention policy
        createRetentionPolicy(influxdb, config.influxdb_db, config.retention_days);
        
    } catch (const std::exception& e) {
        std::cerr << "Failed to connect to InfluxDB: " << e.what() << std::endl;
        config.enable_influxdb = false;
    }
    
    return influxdb;
}

bool tryParseNumeric(const std::string& value, double& result) {
    try {
        result = std::stod(value);
        return true;
    } catch (const std::exception&) {
        return false;
    }
} 