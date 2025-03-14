#ifndef INFLUXDB_HANDLER_H
#define INFLUXDB_HANDLER_H

#include <memory>
#include <string>
#include <InfluxDBFactory.h>
#include "config.h"

// Create InfluxDB retention policy
void createRetentionPolicy(std::shared_ptr<influxdb::InfluxDB> influxdb, const std::string& dbName, int retentionDays);

// Connect to InfluxDB
std::shared_ptr<influxdb::InfluxDB> connectToInfluxDB(Config& config);

// Try to convert Redis value to numeric
bool tryParseNumeric(const std::string& value, double& result);

#endif // INFLUXDB_HANDLER_H 