#ifndef REDIS_HANDLER_H
#define REDIS_HANDLER_H

#include <hiredis/hiredis.h>
#include <memory>
#include <string>
#include <vector>
#include <unordered_map>
#include <unordered_set>
#include "config.h"

// Redis connection wrapper
class RedisConnection {
private:
    redisContext* context;
    bool connected;
    
public:
    RedisConnection();
    ~RedisConnection();
    
    // Connect to Redis
    bool connect(const Config& config);
    
    // Execute Redis command
    redisReply* execute(const char* format, ...);
    
    // Check if connected
    bool isConnected() const { return connected; }
    
    // Get keys matching pattern
    std::vector<std::string> getKeys(const std::string& pattern);
    
    // Get Redis data type
    enum class RedisType {
        STRING,
        LIST,
        SET,
        HASH,
        ZSET,
        NONE
    };
    
    RedisType getType(const std::string& key);
    
    // Get string value
    std::string getString(const std::string& key);
    
    // Get hash values
    std::unordered_map<std::string, std::string> getHash(const std::string& key);
    
    // Get list values
    std::vector<std::string> getList(const std::string& key);
    
    // Get set values
    std::unordered_set<std::string> getSet(const std::string& key);
    
    // Get sorted set values
    std::vector<std::pair<std::string, double>> getZSet(const std::string& key);
};

// Process Redis data and write to InfluxDB
void processRedisData(RedisConnection& redis, std::shared_ptr<influxdb::InfluxDB> influxdb, Config& config);

#endif // REDIS_HANDLER_H 