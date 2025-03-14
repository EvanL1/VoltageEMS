#ifndef COM_BASE_H
#define COM_BASE_H

#include <string>
#include <memory>
#include <functional>
#include <variant>
#include <map>
#include <vector>
#include <deque>
#include <mutex>
#include <fstream>
#include <cstdint>
#include <hiredis/hiredis.h>
#include "logger.h"
#include <thread>
#include <atomic>

namespace Communication {

enum class ProtocolType {
    IEC_104,
    IEC_101,
    IEC_103,
    IEC_61850,
    DL645_1997,
    DL645_2007, 
    CAN,
    MODBUS,
    DI_DO,
    CUSTOM,
};

enum class PhysicalInterfaceType {
    NETWORK,     // Network interface
    SERIAL,      // Serial port interface
    DRY_CONTACT, // Dry contact interface
    CAN          // CAN bus interface
};

enum class DeviceRole {
    MASTER,     // Master/Client
    SLAVE       // Slave/Server
};

/*--------------------------------------------------------
The following code defines the data structures for channel 
and point table configuration.
----------------------------------------------------------*/
struct ModbusTCPConfig {
    std::string ip;      // Device IP address
    int port;            // Port number
};

struct ModbusRTUConfig {
    std::string serialPort; // Serial port device (e.g., /dev/ttyS0)
    int baudRate;           // Baud rate
    int dataBits;           // Data bits (7/8)
    int stopBits;           // Stop bits (1/2)
    char parity;            // Parity bit ('N', 'E', 'O')
};

struct IEC104Config {
    std::string remoteIp; // Remote IP
    int remotePort;       // Remote port
    int commonAddr;       // Common address
};

struct IEC61850Config {
    std::string serverIp;  // Server IP address
    std::string logicalDevice; // Logical device name
};

struct CANConfig {
    std::string interface; // CAN bus interface (e.g., can0)
    int bitrate;           // Bitrate (e.g., 500000)
};

struct CustomConfig {
    std::map<std::string, std::string> settings; // Allows user-defined Key-Value configuration
};

using ProtocolChannelConfig = std::variant<
    std::monostate,
    ModbusTCPConfig,  
    ModbusRTUConfig,  
    IEC104Config,  
    IEC61850Config,  
    CANConfig,
    CustomConfig
>;

// Data type enumeration
enum class DataType {
    INT16,      // 16-bit signed integer
    UINT16,     // 16-bit unsigned integer
    INT32,      // 32-bit signed integer
    UINT32,     // 32-bit unsigned integer
    FLOAT32,    // 32-bit floating point
    BOOL        // Boolean value
};

// Byte order enumeration
enum class ByteOrder {
    AB,         // Big-endian
    BA,         // Little-endian
    ABCD,       // Big-endian (32-bit)
    CDAB,       // Little-endian (32-bit)
    BADC,       // Mixed byte order
    DCBA        // Reverse byte order
};

// Data point type enumeration
enum class PointType {
    DI,     // Digital Input (Status/Position indication)
    AI,     // Analog Input (Measurement)
    DO,     // Digital Output (Command/Control)
    AO      // Analog Output (Setpoint)
};

struct ModbusPointConfig {
    int slaveId;        // Modbus slave ID
    int address;        // Modbus address
    int functionCode;   // Modbus function code
    int dataType;       // Modbus data type
    int bitLength;      // Modbus bit length
};

struct IEC104PointConfig {
    int ioa;           // Information Object Address
    int typeId;        // Type identifier
};

struct IEC61850PointConfig {
    std::string logicalNode;   // Logical node
    std::string dataAttribute; // Data attribute
};

struct CANPointConfig {
    uint32_t canId;     // CAN message ID
    int byteOffset;     // Data byte offset
    int bitLength;      // Bit length
};

using PointConfig = std::variant<std::monostate, 
                                ModbusPointConfig, 
                                IEC104PointConfig, 
                                IEC61850PointConfig, 
                                CANPointConfig>;

// Data point configuration structure
struct DataPointConfig {
    std::string id;          // Point identifier
    PointType pointType_;    // Point type (DI/AI/DO/AO)
    DataType datatype_;      // Data type
    ByteOrder byteOrder_;    // Byte order
    double scale = 1.0;      // Scaling factor
    double offset = 0.0;     // Offset value
    std::string unit;        // Engineering unit
    double min = 0.0;        // Minimum value
    double max = 0.0;        // Maximum value
    bool isValid = true;     // Validity flag
    std::string description; // Point description

    PointConfig pointConfig_; // Specific protocol configuration
};

struct ChannelConfig {
    int index;                 // Channel index
    std::string name;          // Channel name
    ProtocolType protocolType; // Protocol used by this channel
    PhysicalInterfaceType physicalInterfaceType; // Physical interface type
    DeviceRole deviceRole;     // Device role
    ProtocolChannelConfig protocolConfig; // Specific protocol configuration
    std::map<std::string, DataPointConfig> points; // Data points under this channel
    int pollRate = 1000;       // Polling rate in milliseconds, default 1s
};

/*--------------------------------------------------------
The following data structures are runtime data structures.
----------------------------------------------------------*/

// Digital Input state enumeration
enum class DIState {
    OFF = 0,        // Open/Disconnected
    ON = 1,         // Closed/Connected
    INVALID = 2     // Invalid/Unknown state
};

// Digital Output command enumeration
enum class DOState {
    OPEN = 0,       // Open command
    CLOSE = 1,      // Close command
    CANCEL = 2      // Cancel command
};

// Digital Input value structure
struct DIValue {
    std::string id;          // Point identifier
    DIState state;           // Current state
    std::string timestamp;   // Timestamp
    bool isValid;            // Validity flag
    std::string quality;     // Quality descriptor
};

// Analog Input value structure
struct AIValue {
    std::string id;          // Point identifier
    double value;            // Current value
    std::string unit;        // Engineering unit
    std::string timestamp;   // Timestamp
    bool isValid;            // Validity flag
    std::string quality;     // Quality descriptor
};

// Digital Output command structure
struct DOCommand {
    std::string id;          // Point identifier
    DOState command;         // Command type
    std::string timestamp;   // Timestamp
    bool needConfirm;        // Confirmation required
    int timeout;             // Command timeout
    std::string operator_;   // Operator identifier
};

// Analog Output command structure
struct AOCommand {
    std::string id;          // Point identifier
    double value;            // Setpoint value
    std::string unit;        // Engineering unit
    std::string timestamp;   // Timestamp
    bool needConfirm;        // Confirmation required
    int timeout;             // Command timeout
    std::string operator_;   // Operator identifier
};

// Callback function type definitions
using StatusCallback = std::function<void(const std::string& status)>;
using DataCallback = std::function<void(const uint8_t* data, size_t len)>;
using DICallback = std::function<void(const DIValue& value)>;
using AICallback = std::function<void(const AIValue& value)>;
using DOCallback = std::function<void(const std::string& id, bool success)>;
using AOCallback = std::function<void(const std::string& id, bool success)>;

// Point table item structure
struct PointTableItem {
    std::string id;            // Point ID
    PointType type;            // Point type
    DataType dataType;         // Data type
    ByteOrder byteOrder;       // Byte order
    std::string description;   // Description
};

/*---------------------------------------------------------------------*/

/*---------------------------------------------------------------------
   Base class for communication protocols
----------------------------------------------------------------------*/
class ComBase {
public:
    ComBase() = default;
    virtual ~ComBase() = default;

    // Basic interface
    virtual bool init(const std::string& config) = 0;
    virtual bool start() = 0;
    virtual bool stop() = 0;
    virtual bool isRunning() const = 0;

    // Channel management with thread support
    virtual bool startChannel(int channelIndex);
    virtual bool stopChannel(int channelIndex);
    virtual bool isChannelRunning(int channelIndex) const;

    // Configuration callbacks
    virtual void setStatusCallback(StatusCallback cb) { statusCallback_ = cb; }
    virtual void setDataCallback(DataCallback cb) { dataCallback_ = cb; }

    // Get protocol type and role
    virtual ProtocolType getProtocolType() const = 0;
    virtual DeviceRole getDeviceRole() const = 0;

    // Diagnostic interface
    virtual std::string getStatus() const = 0;
    virtual std::string getStatistics() const = 0;

    // Redis interface
    bool connectToRedis(const std::string& host, int port);
    bool writeToRedis(const std::string& key, const std::string& value);
    bool disconnectFromRedis();
    
    // Physical interface interface
    virtual PhysicalInterfaceType getPhysicalInterfaceType() const;
    virtual void setPhysicalInterfaceType(PhysicalInterfaceType type);

    // Channel management interface
    virtual bool createChannel(const ChannelConfig& config);
    virtual bool removeChannel(int index);

    // Data point configuration and parsing
    virtual void addDataPoint(const std::string& id, const DataPointConfig& config);
    virtual void removeDataPoint(const std::string& id);
    virtual DataPointValue parseData(const std::string& id, const std::vector<uint16_t>& rawData);
    virtual bool writeDataToRedis(const DataPointValue& value);

    // Digital Input interface
    virtual bool readDI(const std::string& id, DIValue& value);
    virtual bool readAllDI(std::vector<DIValue>& values);
    virtual void setDICallback(DICallback cb) { diCallback_ = cb; }

    // Analog Input interface
    virtual bool readAI(const std::string& id, AIValue& value);
    virtual bool readAllAI(std::vector<AIValue>& values);
    virtual void setAICallback(AICallback cb) { aiCallback_ = cb; }

    // Digital Output interface
    virtual bool executeDO(const std::string& id, DOCommand command);
    virtual bool cancelDO(const std::string& id);
    virtual void setDOCallback(DOCallback cb) { doCallback_ = cb; }

    // Analog Output interface
    virtual bool executeAO(const std::string& id, double value);
    virtual bool cancelAO(const std::string& id);
    virtual void setAOCallback(AOCallback cb) { aoCallback_ = cb; }
    
    // Point table management interface
    virtual bool addPoint(int channelIndex, const PointTableItem& point);
    virtual bool removePoint(int channelIndex, const std::string& pointId);
    virtual bool getPointValue(int channelIndex, const std::string& pointId, DataPointValue& value);
    virtual bool setPointValue(int channelIndex, const std::string& pointId, const DataPointValue& value);
    
    // Channel status query
    virtual bool isChannelActive(int index) const;
    virtual std::string getChannelStatus(int index) const;
    virtual std::vector<PointTableItem> getChannelPoints(int index) const;

    // Dynamic configuration support
    virtual bool updateConfig(const std::string& config);
    virtual bool reloadPointTable(int channelIndex, PointType type, const std::string& filename);
    virtual bool updateChannel(const ChannelConfig& config);
    
    // Channel reconfiguration method
    virtual bool reconfigureChannel(int channelIndex);

protected:
    ProtocolType protocolType_;  // Protocol type member
    DeviceRole deviceRole_;      // Device role member
    PhysicalInterfaceType physicalInterface_ = PhysicalInterfaceType::NETWORK;  // Default to network
    StatusCallback statusCallback_;
    DataCallback dataCallback_;
    bool running_ = false;
    redisContext* redisCtx_ = nullptr;
    std::map<std::string, DataPointConfig> dataPoints_;

    // Callback functions
    DICallback diCallback_;
    AICallback aiCallback_;
    DOCallback doCallback_;
    AOCallback aoCallback_;

    // Data parsing helper functions
    // Parses the raw data into a value based on the provided configuration.
    double parseValue(const std::vector<uint16_t>& rawData, 
                     const DataPointConfig& config);
    
    // Combines two 16-bit values into a 32-bit integer based on the specified byte order.
    int32_t combine32Bit(uint16_t high, uint16_t low, ByteOrder order);

    // Combines two 16-bit values into a floating-point number based on the specified byte order.
    float combineFloat(uint16_t high, uint16_t low, ByteOrder order);
    
    // Validates the given value against the specified data point configuration.
    bool validateValue(double value, const DataPointConfig& config);
    
    // Retrieves the current timestamp as a string.
    std::string getCurrentTimestamp();

    // Retrieves data points of a specific type.
    virtual std::vector<DataPointConfig> getDataPointsByType(PointType type) const;
    
    // Processes digital input data for a specific point identified by its ID.
    virtual void processDIData(const std::string& id, const std::vector<uint16_t>& data);
    
    // Processes analog input data for a specific point identified by its ID.
    virtual void processAIData(const std::string& id, const std::vector<uint16_t>& data);
    
    // Validates a digital output command for a specific point identified by its ID.
    virtual bool validateDOCommand(const std::string& id, DOCommand command);
    
    // Validates an analog output value for a specific point identified by its ID.
    virtual bool validateAOValue(const std::string& id, double value);
    
    // Formats a Redis key based on the point ID and its type.
    virtual std::string formatRedisKey(const std::string& id, PointType type);

    // Logger instance
    Logger& logger_ = Logger::getInstance();

    // Channel storage
    std::map<int, ChannelConfig> channels_;

    // Channel operation helper functions
    virtual bool validateChannel(int index) const;
    virtual bool validatePoint(int channelIndex, const std::string& pointId) const;
    virtual DataPointValue processChannelValue(const DataPointValue& raw, 
                                             const ChannelProperties& props);

    // Channel data processing
    virtual void processChannelData(int channelIndex, 
                                  const std::vector<uint8_t>& data);
    virtual bool writeChannelData(int channelIndex, 
                                const std::string& pointId,
                                const std::vector<uint8_t>& data);

    // Thread management for channels
    std::map<int, std::thread> channelThreads_;         // Thread for each channel
    std::map<int, std::atomic<bool>> channelRunning_;   // Running flag for each channel
    std::mutex channelsMutex_;                          // Mutex for channel operations
    
    // Channel thread function - to be called in a separate thread
    virtual void channelThreadFunc(int channelIndex);
    
    // Thread synchronization for data access
    std::mutex dataPointsMutex_;                        // Mutex for data points access

    // Configuration update processing
    virtual void handleConfigChange(int channelIndex);
    
    // Point table parsing method
    virtual bool parsePointTable(const std::string& filename, 
                                PointType type, 
                                std::map<std::string, DataPointConfig>& points);
};

class ConfigManager {
public:
    static ConfigManager& getInstance();
    
    // Initialize configuration manager
    bool init(const std::string& configDir);
    
    // Load main configuration file
    virtual bool loadChannelConfig(const std::string& filename);
    
    // Load point table configuration
    virtual bool loadPointTable(int channelIndex, PointType type, const std::string& filename);
    
    // Get channel configuration
    virtual std::vector<ChannelConfig> getChannelConfigs() const;
    
    // Get specified channel configuration
    virtual bool getChannelConfig(int index, ChannelConfig& config) const;
    
    // Dynamic update channel configuration
    virtual bool updateChannelConfig(const ChannelConfig& config);
    
    // Dynamic update point table
    virtual bool updatePointTable(int channelIndex, PointType type, const std::string& filename);
    
    // Register configuration change callback
    virtual void setConfigChangeCallback(std::function<void(int channelIndex)> callback);
    
    // Monitor configuration file changes
    virtual void startConfigMonitoring();
    virtual void stopConfigMonitoring();

private:
    ConfigManager();
    ~ConfigManager();
    
    // File monitoring thread
    void monitoringThreadFunc();
    
    // Configuration change detection
    bool isConfigFileChanged(const std::string& filename, time_t& lastModified);
    
    // Configuration validation
    bool validateConfig(const ChannelConfig& config);
    
    std::map<int, ChannelConfig> channels_;
    std::map<std::string, time_t> fileTimestamps_;
    std::function<void(int channelIndex)> configChangeCallback_;
    
    std::thread monitoringThread_;
    std::atomic<bool> monitoringRunning_;
    std::mutex configMutex_;
    
    std::string configDir_;
};

} // namespace Communication

#endif 