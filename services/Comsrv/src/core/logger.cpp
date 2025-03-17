#include "core/logger.h"
#include <filesystem>
#include <iomanip>
#include <sstream>

namespace voltage {
namespace comsrv {

Logger& Logger::instance() {
    static Logger instance;
    return instance;
}

void Logger::initServiceLogger(const std::string& filename,
                             size_t max_size,
                             size_t max_files,
                             bool console_output,
                             const std::string& format,
                             LogLevel level) {
    try {
        // Create directory if it doesn't exist
        std::filesystem::path log_path(filename);
        std::filesystem::create_directories(log_path.parent_path());

        // Create sinks
        std::vector<spdlog::sink_ptr> sinks;
        
        // Add rotating file sink
        auto file_sink = std::make_shared<spdlog::sinks::rotating_file_sink_mt>(
            filename, max_size, max_files);
        sinks.push_back(file_sink);

        // Add console sink if requested
        if (console_output) {
            auto console_sink = std::make_shared<spdlog::sinks::stdout_color_sink_mt>();
            sinks.push_back(console_sink);
        }

        // Create logger with all sinks
        service_logger_ = std::make_shared<spdlog::logger>("service", sinks.begin(), sinks.end());
        
        // Set pattern
        service_logger_->set_pattern(format);

        // Set level
        switch (level) {
            case LogLevel::TRACE: service_logger_->set_level(spdlog::level::trace); break;
            case LogLevel::DEBUG: service_logger_->set_level(spdlog::level::debug); break;
            case LogLevel::INFO: service_logger_->set_level(spdlog::level::info); break;
            case LogLevel::WARN: service_logger_->set_level(spdlog::level::warn); break;
            case LogLevel::ERROR: service_logger_->set_level(spdlog::level::err); break;
            case LogLevel::CRITICAL: service_logger_->set_level(spdlog::level::critical); break;
        }

        // Register as default logger
        spdlog::set_default_logger(service_logger_);

        serviceInfo("Service logger initialized");
    } catch (const spdlog::spdlog_ex& ex) {
        std::cerr << "Service logger initialization failed: " << ex.what() << std::endl;
        throw;
    }
}

std::shared_ptr<spdlog::logger> Logger::createChannelLogger(
    const std::string& channel_id,
    const std::string& base_path,
    size_t max_size,
    size_t max_files,
    const std::string& format,
    LogLevel level) {
    try {
        // Create directory if it doesn't exist
        std::filesystem::path log_dir(base_path);
        std::filesystem::create_directories(log_dir);

        // Create log filename
        std::string filename = (log_dir / (channel_id + ".log")).string();

        // Create rotating file sink
        auto file_sink = std::make_shared<spdlog::sinks::rotating_file_sink_mt>(
            filename, max_size, max_files);

        // Create logger
        auto logger = std::make_shared<spdlog::logger>(channel_id, file_sink);
        
        // Set pattern
        logger->set_pattern(format);

        // Set level
        switch (level) {
            case LogLevel::TRACE: logger->set_level(spdlog::level::trace); break;
            case LogLevel::DEBUG: logger->set_level(spdlog::level::debug); break;
            case LogLevel::INFO: logger->set_level(spdlog::level::info); break;
            case LogLevel::WARN: logger->set_level(spdlog::level::warn); break;
            case LogLevel::ERROR: logger->set_level(spdlog::level::err); break;
            case LogLevel::CRITICAL: logger->set_level(spdlog::level::critical); break;
        }

        // Store logger
        channel_loggers_[channel_id] = logger;

        serviceInfo("Channel logger created for " + channel_id);
        return logger;
    } catch (const spdlog::spdlog_ex& ex) {
        serviceError("Channel logger creation failed for " + channel_id + ": " + ex.what());
        throw;
    }
}

// Service logging methods
void Logger::serviceTrace(const std::string& message) {
    if (service_logger_) service_logger_->trace(message);
}

void Logger::serviceDebug(const std::string& message) {
    if (service_logger_) service_logger_->debug(message);
}

void Logger::serviceInfo(const std::string& message) {
    if (service_logger_) service_logger_->info(message);
}

void Logger::serviceWarn(const std::string& message) {
    if (service_logger_) service_logger_->warn(message);
}

void Logger::serviceError(const std::string& message) {
    if (service_logger_) service_logger_->error(message);
}

void Logger::serviceCritical(const std::string& message) {
    if (service_logger_) service_logger_->critical(message);
}

// Channel logging methods
void Logger::channelTrace(const std::string& channel_id, const std::string& message) {
    auto it = channel_loggers_.find(channel_id);
    if (it != channel_loggers_.end()) it->second->trace(message);
}

void Logger::channelDebug(const std::string& channel_id, const std::string& message) {
    auto it = channel_loggers_.find(channel_id);
    if (it != channel_loggers_.end()) it->second->debug(message);
}

void Logger::channelInfo(const std::string& channel_id, const std::string& message) {
    auto it = channel_loggers_.find(channel_id);
    if (it != channel_loggers_.end()) it->second->info(message);
}

void Logger::channelWarn(const std::string& channel_id, const std::string& message) {
    auto it = channel_loggers_.find(channel_id);
    if (it != channel_loggers_.end()) it->second->warn(message);
}

void Logger::channelError(const std::string& channel_id, const std::string& message) {
    auto it = channel_loggers_.find(channel_id);
    if (it != channel_loggers_.end()) it->second->error(message);
}

void Logger::channelCritical(const std::string& channel_id, const std::string& message) {
    auto it = channel_loggers_.find(channel_id);
    if (it != channel_loggers_.end()) it->second->critical(message);
}

// Raw data logging methods
void Logger::logRawData(const std::string& channel_id,
                       const std::string& direction,
                       const uint8_t* data,
                       size_t length) {
    auto it = channel_loggers_.find(channel_id);
    if (it != channel_loggers_.end()) {
        std::stringstream ss;
        ss << direction << " [";
        for (size_t i = 0; i < length; ++i) {
            if (i > 0) ss << " ";
            ss << std::hex << std::setw(2) << std::setfill('0') 
               << static_cast<int>(data[i]);
        }
        ss << "]";
        it->second->info(ss.str());
    }
}

// Parse detail logging methods
void Logger::logParseDetail(const std::string& channel_id,
                          const std::string& point_type,
                          const std::string& point_name,
                          const std::string& value,
                          const std::string& detail) {
    auto it = channel_loggers_.find(channel_id);
    if (it != channel_loggers_.end()) {
        std::stringstream ss;
        ss << "Parse " << point_type << " [" << point_name << "] = " 
           << value << " (" << detail << ")";
        it->second->debug(ss.str());
    }
}

} // namespace comsrv
} // namespace voltage 