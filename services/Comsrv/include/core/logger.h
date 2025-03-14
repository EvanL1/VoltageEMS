#ifndef LOGGER_H
#define LOGGER_H

#include <string>
#include <deque>
#include <mutex>
#include <fstream>
#include <memory>
#include <vector>
#include <hiredis/hiredis.h>

// Log level enumeration
enum class LogLevel {
    DEBUG,      // Detailed information for debugging
    INFO,       // General operational information
    WARNING,    // Warning messages for potential issues
    ERROR,      // Error messages for actual problems
    CRITICAL    // Critical errors that may cause system failure
};

// Log entry structure
struct LogEntry {
    std::string timestamp;    // Time of the event
    std::string channelId;    // Channel/Device identifier
    LogLevel level;           // Log level
    std::string message;      // Log message
    std::string details;      // Additional details (JSON format)
};

class Logger {
public:
    static Logger& getInstance();

    // Logging interface
    void log(const std::string& channelId, LogLevel level, 
             const std::string& message, const std::string& details = "");
    void logDebug(const std::string& channelId, const std::string& message, 
                  const std::string& details = "");
    void logInfo(const std::string& channelId, const std::string& message, 
                 const std::string& details = "");
    void logWarning(const std::string& channelId, const std::string& message, 
                   const std::string& details = "");
    void logError(const std::string& channelId, const std::string& message, 
                  const std::string& details = "");
    void logCritical(const std::string& channelId, const std::string& message, 
                    const std::string& details = "");

    // Log retrieval interface
    std::vector<LogEntry> getLogEntries(const std::string& channelId = "", 
                                       const std::string& startTime = "",
                                       const std::string& endTime = "",
                                       LogLevel minLevel = LogLevel::DEBUG,
                                       int maxEntries = 100);
    
    // Log configuration
    void setLogLevel(LogLevel level) { logLevel_ = level; }
    void setLogRetention(int days) { logRetentionDays_ = days; }
    void setMaxLogEntries(int maxEntries) { maxLogEntries_ = maxEntries; }
    void enableLogToRedis(bool enable) { logToRedis_ = enable; }
    void enableLogToFile(bool enable, const std::string& path = "");
    bool connectToRedis(const std::string& host, int port);
    void disconnectFromRedis();

private:
    Logger();  // Private constructor for singleton
    ~Logger();
    Logger(const Logger&) = delete;
    Logger& operator=(const Logger&) = delete;

    // Logging helpers
    void writeLogToRedis(const LogEntry& entry);
    void writeLogToFile(const LogEntry& entry);
    void cleanupOldLogs();
    std::string formatLogEntry(const LogEntry& entry);
    bool isLogLevelEnabled(LogLevel level) const { return level >= logLevel_; }
    std::string logLevelToString(LogLevel level) const;
    std::string getCurrentTimestamp();

    // Logging configuration
    LogLevel logLevel_ = LogLevel::INFO;
    int logRetentionDays_ = 30;
    int maxLogEntries_ = 10000;
    bool logToRedis_ = true;
    bool logToFile_ = false;
    std::string logFilePath_;
    std::ofstream logFile_;
    redisContext* redisCtx_ = nullptr;

    // Log storage
    std::deque<LogEntry> inMemoryLogs_;
    std::mutex logMutex_;
};

#endif // LOGGER_H 