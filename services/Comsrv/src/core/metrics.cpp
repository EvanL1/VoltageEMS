#include "core/metrics.h"
#include <prometheus/counter.h>
#include <prometheus/gauge.h>
#include <prometheus/histogram.h>
#include <iostream>

namespace voltage {
namespace comsrv {

Metrics& Metrics::instance() {
    static Metrics instance;
    return instance;
}

Metrics::Metrics()
    : registry_(std::make_shared<prometheus::Registry>())
    // Communication metrics
    , bytes_total_(prometheus::BuildCounter()
        .Name("comsrv_bytes_total")
        .Help("Total number of bytes sent/received")
        .Labels({{"service", "comsrv"}})
        .Register(*registry_))
    , packets_total_(prometheus::BuildCounter()
        .Name("comsrv_packets_total")
        .Help("Total number of packets sent/received")
        .Labels({{"service", "comsrv"}})
        .Register(*registry_))
    , packet_errors_(prometheus::BuildCounter()
        .Name("comsrv_packet_errors_total")
        .Help("Total number of packet errors by type")
        .Labels({{"service", "comsrv"}})
        .Register(*registry_))
    , packet_processing_duration_seconds_(prometheus::BuildHistogram()
        .Name("comsrv_packet_processing_duration_seconds")
        .Help("Packet processing duration in seconds")
        .Labels({{"service", "comsrv"}})
        .Register(*registry_))
    // Channel metrics
    , channel_status_(prometheus::BuildGauge()
        .Name("comsrv_channel_status")
        .Help("Channel connection status (1 for connected, 0 for disconnected)")
        .Labels({{"service", "comsrv"}})
        .Register(*registry_))
    , channel_response_time_seconds_(prometheus::BuildGauge()
        .Name("comsrv_channel_response_time_seconds")
        .Help("Channel response time in seconds")
        .Labels({{"service", "comsrv"}})
        .Register(*registry_))
    , channel_errors_(prometheus::BuildCounter()
        .Name("comsrv_channel_errors_total")
        .Help("Total number of channel errors by type")
        .Labels({{"service", "comsrv"}})
        .Register(*registry_))
    // Protocol metrics
    , protocol_status_(prometheus::BuildGauge()
        .Name("comsrv_protocol_status")
        .Help("Protocol status (1 for active, 0 for inactive)")
        .Labels({{"service", "comsrv"}})
        .Register(*registry_))
    , protocol_errors_(prometheus::BuildCounter()
        .Name("comsrv_protocol_errors_total")
        .Help("Total number of protocol errors by type")
        .Labels({{"service", "comsrv"}})
        .Register(*registry_))
    // Service metrics
    , service_status_(prometheus::BuildGauge()
        .Name("comsrv_service_status")
        .Help("Service status (1 for running, 0 for stopped)")
        .Labels({{"service", "comsrv"}})
        .Register(*registry_))
    , service_uptime_seconds_(prometheus::BuildGauge()
        .Name("comsrv_service_uptime_seconds")
        .Help("Service uptime in seconds")
        .Labels({{"service", "comsrv"}})
        .Register(*registry_))
    , service_errors_(prometheus::BuildCounter()
        .Name("comsrv_service_errors_total")
        .Help("Total number of service errors by type")
        .Labels({{"service", "comsrv"}})
        .Register(*registry_))
{
}

void Metrics::init(const std::string& bind_address, const Labels& global_labels) {
    global_labels_ = global_labels;
    registry_ = std::make_shared<prometheus::Registry>();
    exposer_ = std::make_unique<prometheus::Exposer>(bind_address);
    exposer_->RegisterCollectable(registry_);

    // Initialize metric families
    bytes_total_ = &prometheus::BuildCounter()
        .Name("comsrv_bytes_total")
        .Help("Total number of bytes sent/received")
        .Register(*registry_);

    packets_total_ = &prometheus::BuildCounter()
        .Name("comsrv_packets_total")
        .Help("Total number of packets sent/received")
        .Register(*registry_);

    packet_errors_ = &prometheus::BuildCounter()
        .Name("comsrv_packet_errors_total")
        .Help("Total number of packet errors by type")
        .Register(*registry_);

    packet_processing_duration_seconds_ = &prometheus::BuildHistogram()
        .Name("comsrv_packet_processing_duration_seconds")
        .Help("Time spent processing packets")
        .Register(*registry_);

    channel_status_ = &prometheus::BuildGauge()
        .Name("comsrv_channel_status")
        .Help("Channel connection status (1=connected, 0=disconnected)")
        .Register(*registry_);

    channel_response_time_seconds_ = &prometheus::BuildGauge()
        .Name("comsrv_channel_response_time_seconds")
        .Help("Channel response time in seconds")
        .Register(*registry_);

    channel_errors_ = &prometheus::BuildCounter()
        .Name("comsrv_channel_errors_total")
        .Help("Total number of channel errors by type")
        .Register(*registry_);

    protocol_status_ = &prometheus::BuildGauge()
        .Name("comsrv_protocol_status")
        .Help("Protocol status (1=active, 0=inactive)")
        .Register(*registry_);

    protocol_errors_ = &prometheus::BuildCounter()
        .Name("comsrv_protocol_errors_total")
        .Help("Total number of protocol errors by type")
        .Register(*registry_);

    service_status_ = &prometheus::BuildGauge()
        .Name("comsrv_service_status")
        .Help("Service status (1=running, 0=stopped)")
        .Register(*registry_);

    service_uptime_seconds_ = &prometheus::BuildGauge()
        .Name("comsrv_service_uptime_seconds")
        .Help("Service uptime in seconds")
        .Register(*registry_);

    service_errors_ = &prometheus::BuildCounter()
        .Name("comsrv_service_errors_total")
        .Help("Total number of service errors by type")
        .Register(*registry_);
}

Labels Metrics::mergeLabels(const Labels& extra_labels) const {
    Labels merged = global_labels_;
    merged.insert(extra_labels.begin(), extra_labels.end());
    return merged;
}

// Communication metrics
void Metrics::incrementBytesSent(const std::string& protocol, size_t bytes, const Labels& extra_labels) {
    auto labels = mergeLabels(extra_labels);
    labels["protocol"] = protocol;
    labels["direction"] = "sent";
    bytes_total_.Add(labels).Increment(bytes);
}

void Metrics::incrementBytesReceived(const std::string& protocol, size_t bytes, const Labels& extra_labels) {
    auto labels = mergeLabels(extra_labels);
    labels["protocol"] = protocol;
    labels["direction"] = "received";
    bytes_total_.Add(labels).Increment(bytes);
}

void Metrics::incrementPacketsSent(const std::string& protocol, const Labels& extra_labels) {
    auto labels = mergeLabels(extra_labels);
    labels["protocol"] = protocol;
    labels["direction"] = "sent";
    packets_total_.Add(labels).Increment();
}

void Metrics::incrementPacketsReceived(const std::string& protocol, const Labels& extra_labels) {
    auto labels = mergeLabels(extra_labels);
    labels["protocol"] = protocol;
    labels["direction"] = "received";
    packets_total_.Add(labels).Increment();
}

void Metrics::incrementPacketErrors(const std::string& protocol, const std::string& error_type, const Labels& extra_labels) {
    auto labels = mergeLabels(extra_labels);
    labels["protocol"] = protocol;
    labels["error_type"] = error_type;
    packet_errors_.Add(labels).Increment();
}

void Metrics::observePacketProcessingTime(const std::string& protocol, double seconds, const Labels& extra_labels) {
    auto labels = mergeLabels(extra_labels);
    labels["protocol"] = protocol;
    packet_processing_duration_seconds_.Add(labels).Observe(seconds);
}

// Channel metrics
void Metrics::setChannelStatus(const std::string& channel_id, bool connected, const Labels& extra_labels) {
    auto labels = mergeLabels(extra_labels);
    labels["channel_id"] = channel_id;
    channel_status_.Add(labels).Set(connected ? 1 : 0);
}

void Metrics::setChannelResponseTime(const std::string& channel_id, double seconds, const Labels& extra_labels) {
    auto labels = mergeLabels(extra_labels);
    labels["channel_id"] = channel_id;
    channel_response_time_seconds_.Add(labels).Set(seconds);
}

void Metrics::incrementChannelErrors(const std::string& channel_id, const std::string& error_type, const Labels& extra_labels) {
    auto labels = mergeLabels(extra_labels);
    labels["channel_id"] = channel_id;
    labels["error_type"] = error_type;
    channel_errors_.Add(labels).Increment();
}

// Protocol metrics
void Metrics::setProtocolStatus(const std::string& protocol, bool active, const Labels& extra_labels) {
    auto labels = mergeLabels(extra_labels);
    labels["protocol"] = protocol;
    protocol_status_.Add(labels).Set(active ? 1 : 0);
}

void Metrics::incrementProtocolErrors(const std::string& protocol, const std::string& error_type, const Labels& extra_labels) {
    auto labels = mergeLabels(extra_labels);
    labels["protocol"] = protocol;
    labels["error_type"] = error_type;
    protocol_errors_.Add(labels).Increment();
}

// Service metrics
void Metrics::setServiceStatus(bool running) {
    service_status_.Add(global_labels_).Set(running ? 1 : 0);
}

void Metrics::setServiceUptime(double seconds) {
    service_uptime_seconds_.Add(global_labels_).Set(seconds);
}

void Metrics::incrementServiceErrors(const std::string& error_type) {
    auto labels = global_labels_;
    labels["error_type"] = error_type;
    service_errors_.Add(labels).Increment();
}

} // namespace comsrv
} // namespace voltage 