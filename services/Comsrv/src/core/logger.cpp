#include "logger.h"
#include <iostream>
#include <chrono>
#include <iomanip>
#include <sstream>
#include <cstring>

Logger& Logger::getInstance() {
    static Logger instance;
    return instance;
}

Logger::Logger() : redisCtx_(nullptr) {}

Logger::~Logger() {
    disconnectFromRedis();
}

bool Logger::connectToRedis(const std::string& host, int port) {
    redisCtx_ = redisConnect(host.c_str(), port);
    if (redisCtx_ == nullptr || redisCtx_->err) {
        if (redisCtx_) {
            std::cerr << "Redis connection error: " << redisCtx_->errstr << std::endl;
            redisFree(redisCtx_);
        } else {
            std::cerr << "Redis connection error: can't allocate redis context" << std::endl;
        }
        return false;
    }
    return true;
}

void Logger::disconnectFromRedis() {
    if (redisCtx_) {
        redisFree(redisCtx_);
        redisCtx_ = nullptr;
    }
}

void Logger::log(const std::string& channelId, LogLevel level, 
                const std::string& message, const std::string& details) {
    if (!isLogLevelEnabled(level)) {
        return;
    }

    LogEntry entry{
        getCurrentTimestamp(),
        channelId,
        level,
        message,
        details
    };

    // Thread-safe logging
    std::lock_guard<std::mutex> lock(logMutex_);

    // Add to in-memory storage
    inMemoryLogs_.push_back(entry);
    if (inMemoryLogs_.size() > maxLogEntries_) {
        inMemoryLogs_.pop_front();
    }

    // Write to Redis if enabled
    if (logToRedis_) {
        writeLogToRedis(entry);
    }

    // Write to file if enabled
    if (logToFile_) {
        writeLogToFile(entry);
    }

    // Cleanup old logs periodically
    static int cleanupCounter = 0;
    if (++cleanupCounter >= 1000) {
        cleanupOldLogs();
        cleanupCounter = 0;
    }
}

void Logger::logDebug(const std::string& channelId, const std::string& message, 
                     const std::string& details) {
    log(channelId, LogLevel::DEBUG, message, details);
}

void Logger::logInfo(const std::string& channelId, const std::string& message, 
                    const std::string& details) {
    log(channelId, LogLevel::INFO, message, details);
}

void Logger::logWarning(const std::string& channelId, const std::string& message, 
                       const std::string& details) {
    log(channelId, LogLevel::WARNING, message, details);
}

void Logger::logError(const std::string& channelId, const std::string& message, 
                     const std::string& details) {
    log(channelId, LogLevel::ERROR, message, details);
}

void Logger::logCritical(const std::string& channelId, const std::string& message, 
                        const std::string& details) {
    log(channelId, LogLevel::CRITICAL, message, details);
}

std::vector<LogEntry> Logger::getLogEntries(const std::string& channelId, 
                                          const std::string& startTime,
                                          const std::string& endTime,
                                          LogLevel minLevel,
                                          int maxEntries) {
    std::vector<LogEntry> result;
    std::lock_guard<std::mutex> lock(logMutex_);

    for (const auto& entry : inMemoryLogs_) {
        // Filter by channel
        if (!channelId.empty() && entry.channelId != channelId) {
            continue;
        }

        // Filter by level
        if (entry.level < minLevel) {
            continue;
        }

        // Filter by time range
        if (!startTime.empty() && entry.timestamp < startTime) {
            continue;
        }
        if (!endTime.empty() && entry.timestamp > endTime) {
            continue;
        }

        result.push_back(entry);

        // Limit number of entries
        if (result.size() >= static_cast<size_t>(maxEntries)) {
            break;
        }
    }

    return result;
}

void Logger::enableLogToFile(bool enable, const std::string& path) {
    std::lock_guard<std::mutex> lock(logMutex_);
    
    if (enable && !path.empty()) {
        logFilePath_ = path;
        if (logFile_.is_open()) {
            logFile_.close();
        }
        logFile_.open(path, std::ios::app);
        logToFile_ = logFile_.is_open();
    } else {
        logToFile_ = false;
        if (logFile_.is_open()) {
            logFile_.close();
        }
    }
}

void Logger::writeLogToRedis(const LogEntry& entry) {
    if (!redisCtx_) {
        return;
    }

    std::string key = "log:" + entry.channelId + ":" + entry.timestamp;
    std::string value = formatLogEntry(entry);
    
    redisReply* reply = (redisReply*)redisCommand(redisCtx_, "SET %s %s", 
                                                 key.c_str(), value.c_str());
    if (reply) {
        freeReplyObject(reply);
    }

    // Set expiration time based on retention policy
    std::string expireCmd = "EXPIRE " + key + " " + 
                           std::to_string(logRetentionDays_ * 24 * 3600);
    reply = (redisReply*)redisCommand(redisCtx_, expireCmd.c_str());
    if (reply) {
        freeReplyObject(reply);
    }
}

void Logger::writeLogToFile(const LogEntry& entry) {
    if (!logFile_.is_open()) {
        return;
    }

    logFile_ << formatLogEntry(entry) << std::endl;
    logFile_.flush();
}

void Logger::cleanupOldLogs() {
    if (inMemoryLogs_.empty()) {
        return;
    }

    auto now = std::chrono::system_clock::now();
    auto retention = std::chrono::hours(logRetentionDays_ * 24);
    auto cutoff = std::chrono::system_clock::to_time_t(now - retention);
    
    while (!inMemoryLogs_.empty()) {
        const auto& entry = inMemoryLogs_.front();
        std::tm tm = {};
        std::istringstream ss(entry.timestamp);
        ss >> std::get_time(&tm, "%Y-%m-%d %H:%M:%S");
        
        if (std::mktime(&tm) < cutoff) {
            inMemoryLogs_.pop_front();
        } else {
            break;
        }
    }
}

std::string Logger::formatLogEntry(const LogEntry& entry) {
    std::stringstream ss;
    ss << "{"
       << "\"timestamp\":\"" << entry.timestamp << "\","
       << "\"channel\":\"" << entry.channelId << "\","
       << "\"level\":\"" << logLevelToString(entry.level) << "\","
       << "\"message\":\"" << entry.message << "\"";
    
    if (!entry.details.empty()) {
        ss << ",\"details\":" << entry.details;
    }
    
    ss << "}";
    return ss.str();
}

std::string Logger::getCurrentTimestamp() {
    auto now = std::chrono::system_clock::now();
    auto now_c = std::chrono::system_clock::to_time_t(now);
    auto now_ms = std::chrono::duration_cast<std::chrono::milliseconds>(
        now.time_since_epoch()) % 1000;
    
    std::stringstream ss;
    ss << std::put_time(std::localtime(&now_c), "%Y-%m-%d %H:%M:%S")
       << '.' << std::setfill('0') << std::setw(3) << now_ms.count();
    
    return ss.str();
}

std::string Logger::logLevelToString(LogLevel level) const {
    switch (level) {
        case LogLevel::DEBUG:    return "DEBUG";
        case LogLevel::INFO:     return "INFO";
        case LogLevel::WARNING:  return "WARNING";
        case LogLevel::ERROR:    return "ERROR";
        case LogLevel::CRITICAL: return "CRITICAL";
        default:                 return "UNKNOWN";
    }
} 