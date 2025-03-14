#ifndef DI_DO_COM_H
#define DI_DO_COM_H

#include "comBase.h"
#include <string>
#include <vector>

class DiDoCom : public ComBase {
public:
    DiDoCom();
    virtual ~DiDoCom();

    // Implement the ComBase interface
    bool init(const std::string& config) override;
    bool start() override;
    bool stop() override;
    bool isRunning() const override;

    ProtocolType getProtocolType() const override;
    DeviceRole getDeviceRole() const override;

    std::string getStatus() const override;
    std::string getStatistics() const override;

    // DI/DO specific methods
    bool readDI(int channel, bool& value) override;
    bool readDO(int channel, bool& value) override;
    bool writeDO(int channel, bool value) override;

private:
    // Add private members and methods specific to DI/DO
    bool running_;
};

#endif // DI_DO_COM_H 