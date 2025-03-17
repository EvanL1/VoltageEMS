#ifndef LOGGER_H
#define LOGGER_H

#include <string>
#include <memory>
#include <spdlog/spdlog.h>
#include <spdlog/sinks/rotating_file_sink.h>
#include <spdlog/sinks/stdout_color_sinks.h>

namespace voltage {
namespace comsrv {

enum class LogLevel {
    TRACE,
    DEBUG,
    INFO,
    WARN,
    ERROR,
    CRITICAL
};

class Logger {
public:
    static Logger& instance();

    // Initialize service logger
    void initServiceLogger(const std::string& filename,
                          size_t max_size,
                          size_t max_files,
                          bool console_output,
                          const std::string& format,
                          LogLevel level);

    // Initialize channel logger
    std::shared_ptr<spdlog::logger> createChannelLogger(
        const std::string& channel_id,
        const std::string& base_path,
        size_t max_size,
        size_t max_files,
        const std::string& format,
        LogLevel level);

    // Service logging methods
    void serviceTrace(const std::string& message);
    void serviceDebug(const std::string& message);
    void serviceInfo(const std::string& message);
    void serviceWarn(const std::string& message);
    void serviceError(const std::string& message);
    void serviceCritical(const std::string& message);

    // Channel logging methods
    void channelTrace(const std::string& channel_id, const std::string& message);
    void channelDebug(const std::string& channel_id, const std::string& message);
    void channelInfo(const std::string& channel_id, const std::string& message);
    void channelWarn(const std::string& channel_id, const std::string& message);
    void channelError(const std::string& channel_id, const std::string& message);
    void channelCritical(const std::string& channel_id, const std::string& message);

    // Raw data logging methods
    void logRawData(const std::string& channel_id,
                    const std::string& direction,
                    const uint8_t* data,
                    size_t length);

    // Parse detail logging methods
    void logParseDetail(const std::string& channel_id,
                       const std::string& point_type,
                       const std::string& point_name,
                       const std::string& value,
                       const std::string& detail);

private:
    Logger() = default;
    ~Logger() = default;
    Logger(const Logger&) = delete;
    Logger& operator=(const Logger&) = delete;

    std::shared_ptr<spdlog::logger> service_logger_;
    std::map<std::string, std::shared_ptr<spdlog::logger>> channel_loggers_;
};

} // namespace comsrv
} // namespace voltage

#endif // LOGGER_H 