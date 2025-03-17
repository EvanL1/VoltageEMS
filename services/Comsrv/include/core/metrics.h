#pragma once

#include <memory>
#include <string>
#include <map>
#include <prometheus/counter.h>
#include <prometheus/gauge.h>
#include <prometheus/histogram.h>
#include <prometheus/exposer.h>
#include <prometheus/registry.h>

namespace voltage {
namespace comsrv {

using Labels = std::map<std::string, std::string>;

/**
 * @brief Metrics manager for communication service
 */
class Metrics {
public:
    /**
     * @brief Get singleton instance
     */
    static Metrics& instance();

    /**
     * @brief Initialize metrics exposer
     * 
     * @param bind_address Address to bind metrics server (default: "0.0.0.0:9100")
     * @param global_labels Optional global labels to apply to all metrics
     */
    void init(const std::string& bind_address = "0.0.0.0:9100", const Labels& global_labels = {});

    // Communication metrics
    void incrementBytesSent(const std::string& protocol, size_t bytes, const Labels& extra_labels = {});
    void incrementBytesReceived(const std::string& protocol, size_t bytes, const Labels& extra_labels = {});
    void incrementPacketsSent(const std::string& protocol, const Labels& extra_labels = {});
    void incrementPacketsReceived(const std::string& protocol, const Labels& extra_labels = {});
    void incrementPacketErrors(const std::string& protocol, const std::string& error_type, const Labels& extra_labels = {});
    void observePacketProcessingTime(const std::string& protocol, double seconds, const Labels& extra_labels = {});

    // Channel metrics
    void setChannelStatus(const std::string& channel_id, bool connected, const Labels& extra_labels = {});
    void setChannelResponseTime(const std::string& channel_id, double seconds, const Labels& extra_labels = {});
    void incrementChannelErrors(const std::string& channel_id, const std::string& error_type, const Labels& extra_labels = {});

    // Protocol metrics
    void setProtocolStatus(const std::string& protocol, bool active, const Labels& extra_labels = {});
    void incrementProtocolErrors(const std::string& protocol, const std::string& error_type, const Labels& extra_labels = {});

    // Service metrics
    void setServiceStatus(bool running);
    void setServiceUptime(double seconds);
    void incrementServiceErrors(const std::string& error_type);

private:
    Metrics();
    ~Metrics() = default;
    Metrics(const Metrics&) = delete;
    Metrics& operator=(const Metrics&) = delete;

    // Helper method to merge labels
    Labels mergeLabels(const Labels& extra_labels) const;

    std::shared_ptr<prometheus::Registry> registry_;
    std::unique_ptr<prometheus::Exposer> exposer_;
    Labels global_labels_;

    // Communication metrics
    prometheus::Family<prometheus::Counter>& bytes_total_;
    prometheus::Family<prometheus::Counter>& packets_total_;
    prometheus::Family<prometheus::Counter>& packet_errors_;
    prometheus::Family<prometheus::Histogram>& packet_processing_duration_seconds_;

    // Channel metrics
    prometheus::Family<prometheus::Gauge>& channel_status_;
    prometheus::Family<prometheus::Gauge>& channel_response_time_seconds_;
    prometheus::Family<prometheus::Counter>& channel_errors_;

    // Protocol metrics
    prometheus::Family<prometheus::Gauge>& protocol_status_;
    prometheus::Family<prometheus::Counter>& protocol_errors_;

    // Service metrics
    prometheus::Family<prometheus::Gauge>& service_status_;
    prometheus::Family<prometheus::Gauge>& service_uptime_seconds_;
    prometheus::Family<prometheus::Counter>& service_errors_;
};

} // namespace comsrv
} // namespace voltage
