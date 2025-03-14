#include "redis_handler.h"
#include "influxdb_handler.h"
#include <iostream>
#include <cstdarg>

RedisConnection::RedisConnection() : context(nullptr), connected(false) {}

RedisConnection::~RedisConnection() {
    if (context) {
        redisFree(context);
        context = nullptr;
    }
    connected = false;
}

bool RedisConnection::connect(const Config& config) {
    // Free existing context if any
    if (context) {
        redisFree(context);
        context = nullptr;
        connected = false;
    }
    
    // Connect using Unix socket if specified, otherwise use TCP
    if (!config.redis_socket.empty()) {
        context = redisConnectUnix(config.redis_socket.c_str());
        if (context == nullptr || context->err) {
            if (context) {
                std::cerr << "Redis connection error: " << context->errstr << std::endl;
                redisFree(context);
                context = nullptr;
            } else {
                std::cerr << "Redis connection error: can't allocate redis context" << std::endl;
            }
            return false;
        }
        std::cout << "Successfully connected to Redis via Unix socket: " 
                  << config.redis_socket << std::endl;
    } else {
        context = redisConnect(config.redis_host.c_str(), config.redis_port);
        if (context == nullptr || context->err) {
            if (context) {
                std::cerr << "Redis connection error: " << context->errstr << std::endl;
                redisFree(context);
                context = nullptr;
            } else {
                std::cerr << "Redis connection error: can't allocate redis context" << std::endl;
            }
            return false;
        }
        std::cout << "Successfully connected to Redis at " 
                  << config.redis_host << ":" << config.redis_port << std::endl;
    }
    
    // Authenticate if password is provided
    if (!config.redis_password.empty()) {
        redisReply* reply = (redisReply*)redisCommand(context, "AUTH %s", config.redis_password.c_str());
        if (reply == nullptr) {
            std::cerr << "Redis authentication error: no reply" << std::endl;
            redisFree(context);
            context = nullptr;
            return false;
        }
        
        if (reply->type == REDIS_REPLY_ERROR) {
            std::cerr << "Redis authentication error: " << reply->str << std::endl;
            freeReplyObject(reply);
            redisFree(context);
            context = nullptr;
            return false;
        }
        
        freeReplyObject(reply);
    }
    
    // Test connection with PING
    redisReply* reply = (redisReply*)redisCommand(context, "PING");
    if (reply == nullptr || reply->type != REDIS_REPLY_STATUS || strcmp(reply->str, "PONG") != 0) {
        std::cerr << "Redis connection test failed" << std::endl;
        if (reply) freeReplyObject(reply);
        redisFree(context);
        context = nullptr;
        return false;
    }
    
    freeReplyObject(reply);
    connected = true;
    return true;
}

redisReply* RedisConnection::execute(const char* format, ...) {
    if (!connected || !context) {
        return nullptr;
    }
    
    va_list ap;
    va_start(ap, format);
    redisReply* reply = (redisReply*)redisvCommand(context, format, ap);
    va_end(ap);
    
    if (reply == nullptr) {
        connected = false;
    }
    
    return reply;
}

std::vector<std::string> RedisConnection::getKeys(const std::string& pattern) {
    std::vector<std::string> keys;
    
    redisReply* reply = execute("KEYS %s", pattern.c_str());
    if (reply && reply->type == REDIS_REPLY_ARRAY) {
        for (size_t i = 0; i < reply->elements; i++) {
            keys.push_back(reply->element[i]->str);
        }
    }
    
    if (reply) freeReplyObject(reply);
    return keys;
}

RedisConnection::RedisType RedisConnection::getType(const std::string& key) {
    redisReply* reply = execute("TYPE %s", key.c_str());
    RedisType type = RedisType::NONE;
    
    if (reply && reply->type == REDIS_REPLY_STATUS) {
        std::string typeStr = reply->str;
        
        if (typeStr == "string") {
            type = RedisType::STRING;
        } else if (typeStr == "list") {
            type = RedisType::LIST;
        } else if (typeStr == "set") {
            type = RedisType::SET;
        } else if (typeStr == "hash") {
            type = RedisType::HASH;
        } else if (typeStr == "zset") {
            type = RedisType::ZSET;
        }
    }
    
    if (reply) freeReplyObject(reply);
    return type;
}

std::string RedisConnection::getString(const std::string& key) {
    std::string value;
    
    redisReply* reply = execute("GET %s", key.c_str());
    if (reply && reply->type == REDIS_REPLY_STRING) {
        value = reply->str;
    }
    
    if (reply) freeReplyObject(reply);
    return value;
}

std::unordered_map<std::string, std::string> RedisConnection::getHash(const std::string& key) {
    std::unordered_map<std::string, std::string> hash;
    
    redisReply* reply = execute("HGETALL %s", key.c_str());
    if (reply && reply->type == REDIS_REPLY_ARRAY) {
        for (size_t i = 0; i < reply->elements; i += 2) {
            if (i + 1 < reply->elements) {
                std::string field = reply->element[i]->str;
                std::string value = reply->element[i + 1]->str;
                hash[field] = value;
            }
        }
    }
    
    if (reply) freeReplyObject(reply);
    return hash;
}

std::vector<std::string> RedisConnection::getList(const std::string& key) {
    std::vector<std::string> list;
    
    redisReply* reply = execute("LRANGE %s 0 -1", key.c_str());
    if (reply && reply->type == REDIS_REPLY_ARRAY) {
        for (size_t i = 0; i < reply->elements; i++) {
            list.push_back(reply->element[i]->str);
        }
    }
    
    if (reply) freeReplyObject(reply);
    return list;
}

std::unordered_set<std::string> RedisConnection::getSet(const std::string& key) {
    std::unordered_set<std::string> set;
    
    redisReply* reply = execute("SMEMBERS %s", key.c_str());
    if (reply && reply->type == REDIS_REPLY_ARRAY) {
        for (size_t i = 0; i < reply->elements; i++) {
            set.insert(reply->element[i]->str);
        }
    }
    
    if (reply) freeReplyObject(reply);
    return set;
}

std::vector<std::pair<std::string, double>> RedisConnection::getZSet(const std::string& key) {
    std::vector<std::pair<std::string, double>> zset;
    
    redisReply* reply = execute("ZRANGE %s 0 -1 WITHSCORES", key.c_str());
    if (reply && reply->type == REDIS_REPLY_ARRAY) {
        for (size_t i = 0; i < reply->elements; i += 2) {
            if (i + 1 < reply->elements) {
                std::string member = reply->element[i]->str;
                double score = std::stod(reply->element[i + 1]->str);
                zset.push_back(std::make_pair(member, score));
            }
        }
    }
    
    if (reply) freeReplyObject(reply);
    return zset;
}

void processRedisData(RedisConnection& redis, std::shared_ptr<influxdb::InfluxDB> influxdb, Config& config) {
    if (!config.enable_influxdb || !influxdb) {
        if (config.verbose) {
            std::cout << "InfluxDB writing is disabled. Waiting " 
                      << config.interval_seconds << " seconds..." << std::endl;
        }
        return;
    }
    
    if (!redis.isConnected()) {
        std::cerr << "Redis connection lost. Attempting to reconnect..." << std::endl;
        if (!redis.connect(config)) {
            std::cerr << "Failed to reconnect to Redis. Skipping data transfer cycle." << std::endl;
            return;
        }
    }
    
    try {
        // Get matching Redis keys
        std::vector<std::string> keys = redis.getKeys(config.redis_key_pattern);
        
        if (config.verbose) {
            std::cout << "Found " << keys.size() << " keys matching pattern: " 
                      << config.redis_key_pattern << std::endl;
        }
        
        int storedPoints = 0;
        int skippedPoints = 0;
        
        // Process each key
        for (const auto& key : keys) {
            try {
                // Check if this point should be stored
                if (!shouldStorePoint(key, config)) {
                    skippedPoints++;
                    if (config.verbose) {
                        std::cout << "Skipping key (not configured for storage): " << key << std::endl;
                    }
                    continue;
                }
                
                // Get key type
                auto type = redis.getType(key);
                
                // Process data according to type
                if (type == RedisConnection::RedisType::STRING) {
                    // Process string type
                    std::string value = redis.getString(key);
                    
                    // Try to parse value as numeric
                    double numeric_value;
                    bool is_numeric = tryParseNumeric(value, numeric_value);
                    
                    // Create InfluxDB data point
                    auto point = influxdb::Point("redis_data")
                        .addTag("key", key)
                        .addTag("type", "string");
                    
                    if (is_numeric) {
                        point.addField("value", numeric_value);
                    } else {
                        point.addField("text_value", value);
                    }
                    
                    // Write to InfluxDB
                    influxdb->write(std::move(point));
                    storedPoints++;
                    
                    if (config.verbose) {
                        std::cout << "Transferred string key: " << key << std::endl;
                    }
                } else if (type == RedisConnection::RedisType::HASH) {
                    // Process hash type
                    auto hash_values = redis.getHash(key);
                    
                    for (const auto& field : hash_values) {
                        // Try to parse value as numeric
                        double numeric_value;
                        bool is_numeric = tryParseNumeric(field.second, numeric_value);
                        
                        // Create InfluxDB data point
                        auto point = influxdb::Point("redis_data")
                            .addTag("key", key)
                            .addTag("type", "hash")
                            .addTag("field", field.first);
                        
                        if (is_numeric) {
                            point.addField("value", numeric_value);
                        } else {
                            point.addField("text_value", field.second);
                        }
                        
                        // Write to InfluxDB
                        influxdb->write(std::move(point));
                    }
                    storedPoints++;
                    
                    if (config.verbose) {
                        std::cout << "Transferred hash key: " << key 
                                  << " with " << hash_values.size() << " fields" << std::endl;
                    }
                } else if (type == RedisConnection::RedisType::LIST) {
                    // Process list type
                    auto list_values = redis.getList(key);
                    
                    for (size_t i = 0; i < list_values.size(); ++i) {
                        // Try to parse value as numeric
                        double numeric_value;
                        bool is_numeric = tryParseNumeric(list_values[i], numeric_value);
                        
                        // Create InfluxDB data point
                        auto point = influxdb::Point("redis_data")
                            .addTag("key", key)
                            .addTag("type", "list")
                            .addTag("index", std::to_string(i));
                        
                        if (is_numeric) {
                            point.addField("value", numeric_value);
                        } else {
                            point.addField("text_value", list_values[i]);
                        }
                        
                        // Write to InfluxDB
                        influxdb->write(std::move(point));
                    }
                    storedPoints++;
                    
                    if (config.verbose) {
                        std::cout << "Transferred list key: " << key 
                                  << " with " << list_values.size() << " items" << std::endl;
                    }
                } else if (type == RedisConnection::RedisType::SET) {
                    // Process set type
                    auto set_values = redis.getSet(key);
                    
                    for (const auto& value : set_values) {
                        // Try to parse value as numeric
                        double numeric_value;
                        bool is_numeric = tryParseNumeric(value, numeric_value);
                        
                        // Create InfluxDB data point
                        auto point = influxdb::Point("redis_data")
                            .addTag("key", key)
                            .addTag("type", "set");
                        
                        if (is_numeric) {
                            point.addField("value", numeric_value);
                        } else {
                            point.addField("text_value", value);
                        }
                        
                        // Write to InfluxDB
                        influxdb->write(std::move(point));
                    }
                    storedPoints++;
                    
                    if (config.verbose) {
                        std::cout << "Transferred set key: " << key 
                                  << " with " << set_values.size() << " members" << std::endl;
                    }
                } else if (type == RedisConnection::RedisType::ZSET) {
                    // Process sorted set type
                    auto zset_values = redis.getZSet(key);
                    
                    for (const auto& item : zset_values) {
                        // Try to parse value as numeric
                        double numeric_value;
                        bool is_numeric = tryParseNumeric(item.first, numeric_value);
                        
                        // Create InfluxDB data point
                        auto point = influxdb::Point("redis_data")
                            .addTag("key", key)
                            .addTag("type", "zset");
                        
                        // Add score as field
                        point.addField("score", item.second);
                        
                        if (is_numeric) {
                            point.addField("value", numeric_value);
                        } else {
                            point.addField("text_value", item.first);
                        }
                        
                        // Write to InfluxDB
                        influxdb->write(std::move(point));
                    }
                    storedPoints++;
                    
                    if (config.verbose) {
                        std::cout << "Transferred sorted set key: " << key 
                                  << " with " << zset_values.size() << " members" << std::endl;
                    }
                }
            } catch (const std::exception& e) {
                std::cerr << "Error processing key '" << key << "': " << e.what() << std::endl;
                // Continue processing next key
            }
        }
        
        std::cout << "Completed data transfer cycle. Found " << keys.size() 
                  << " keys, stored " << storedPoints << ", skipped " << skippedPoints
                  << ". Waiting " << config.interval_seconds << " seconds for next cycle..." 
                  << std::endl;
        
    } catch (const std::exception& e) {
        std::cerr << "Error during data transfer: " << e.what() << std::endl;
    }
} 