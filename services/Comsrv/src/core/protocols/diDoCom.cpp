#include "diDoCom.h"

DiDoCom::DiDoCom() : running_(false) {}

DiDoCom::~DiDoCom() {
    stop();
}

bool DiDoCom::init(const std::string& config) {
    return true;
}

bool DiDoCom::start() {
    running_ = true;
    return true;
}

bool DiDoCom::stop() {
    running_ = false;
    return true;
}

bool DiDoCom::isRunning() const {
    return running_;
}

ProtocolType DiDoCom::getProtocolType() const {
    return ProtocolType::CUSTOM; 
}

DeviceRole DiDoCom::getDeviceRole() const {
    return DeviceRole::MASTER; 
}

std::string DiDoCom::getStatus() const {
    return "DI/DO status";
}

std::string DiDoCom::getStatistics() const {
    return "DI/DO statistics";
}

bool DiDoCom::readDI(int channel, bool& value) {
    value = false; 
    return true;
}

bool DiDoCom::readDO(int channel, bool& value) {
    value = false; 
    return true;
}

bool DiDoCom::writeDO(int channel, bool value) {
    return true;
} 