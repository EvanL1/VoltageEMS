#include "core/protocols/modbus/modbusRTUMaster.h"
#include "core/metrics.h"
#include <iostream>
#include <chrono>
#include <thread>
#include <fcntl.h>
#include <termios.h>
#include <unistd.h>
#include <string.h>
#include <errno.h>

ModbusRTUMaster::ModbusRTUMaster(const std::string& portName, int baudRate, int dataBits, 
                                SerialParity parity, int stopBits, int timeout)
    : ModbusMaster("modbus_rtu_master")
    , m_portName(portName)
    , m_baudRate(baudRate)
    , m_dataBits(dataBits)
    , m_parity(parity)
    , m_stopBits(stopBits)
    , m_timeout(timeout)
    , m_serialPort(-1)
    , m_serialRunning(false)
    , m_currentRequest(nullptr)
{
    std::cout << "Creating ModbusRTUMaster instance:" << std::endl
              << "  Port: " << m_portName << std::endl
              << "  Baud rate: " << m_baudRate << std::endl
              << "  Data bits: " << m_dataBits << std::endl
              << "  Parity: " << static_cast<int>(m_parity) << std::endl
              << "  Stop bits: " << m_stopBits << std::endl
              << "  Timeout: " << m_timeout << " ms" << std::endl;
    
    // Initialize metrics
    auto& metrics = voltage::comsrv::Metrics::instance();
    metrics.setProtocolStatus(getName(), false);
}

ModbusRTUMaster::~ModbusRTUMaster() {
    if (m_running) {
        stop();
    }
    
    // Update metrics
    auto& metrics = voltage::comsrv::Metrics::instance();
    metrics.setProtocolStatus(getName(), false);
    
    std::cout << "ModbusRTUMaster instance destroyed" << std::endl;
}

bool ModbusRTUMaster::sendRequest(ModbusRequest& request) {
    // Record start time for response time measurement
    auto start_time = std::chrono::steady_clock::now();
    
    // Build request frame
    std::vector<uint8_t> frame;
    frame.push_back(request.slaveId);
    frame.push_back(static_cast<uint8_t>(request.function));
    frame.insert(frame.end(), request.data.begin(), request.data.end());
    
    // Calculate and append CRC
    uint16_t crc = calculateCRC(frame.data(), frame.size());
    frame.push_back(crc & 0xFF);
    frame.push_back((crc >> 8) & 0xFF);
    
    // Send frame
    ssize_t sent = write(m_serialPort, frame.data(), frame.size());
    if (sent != static_cast<ssize_t>(frame.size())) {
        std::string error = "Failed to send request: " + std::string(strerror(errno));
        recordError("send_error", std::to_string(request.slaveId));
        return false;
    }
    
    // Record bytes sent
    recordMetrics(sent, 0, std::to_string(request.slaveId), 0.0);
    
    // Wait for response
    uint8_t buffer[256];
    size_t received = 0;
    
    // Read response header (slave ID + function code)
    while (received < 2) {
        ssize_t n = read(m_serialPort, buffer + received, 2 - received);
        if (n <= 0) {
            if (errno == EAGAIN || errno == EWOULDBLOCK) {
                // Timeout
                std::string error = "Response timeout";
                recordError("timeout_error", std::to_string(request.slaveId));
                return false;
            }
            std::string error = "Failed to read response header: " + std::string(strerror(errno));
            recordError("read_error", std::to_string(request.slaveId));
            return false;
        }
        received += n;
    }
    
    // Check slave ID and function code
    if (buffer[0] != request.slaveId) {
        std::string error = "Invalid slave ID in response";
        recordError("invalid_slave_id", std::to_string(request.slaveId));
        return false;
    }
    
    // Check for exception response
    if (buffer[1] & 0x80) {
        // Read exception code
        ssize_t n = read(m_serialPort, buffer + 2, 1);
        if (n != 1) {
            std::string error = "Failed to read exception code: " + std::string(strerror(errno));
            recordError("read_error", std::to_string(request.slaveId));
            return false;
        }
        received++;
        
        std::string error = "Modbus exception: " + std::to_string(buffer[2]);
        recordError("modbus_exception", std::to_string(request.slaveId));
        return false;
    }
    
    // Read data length based on function code
    size_t expected_length;
    switch (static_cast<ModbusFunction>(buffer[1])) {
        case ModbusFunction::READ_COILS:
        case ModbusFunction::READ_DISCRETE_INPUTS:
        case ModbusFunction::READ_HOLDING_REGISTERS:
        case ModbusFunction::READ_INPUT_REGISTERS:
            // Read byte count
            if (read(m_serialPort, buffer + received, 1) != 1) {
                std::string error = "Failed to read data length: " + std::string(strerror(errno));
                recordError("read_error", std::to_string(request.slaveId));
                return false;
            }
            received++;
            expected_length = buffer[2] + 2; // Data + CRC
            break;
            
        case ModbusFunction::WRITE_SINGLE_COIL:
        case ModbusFunction::WRITE_SINGLE_REGISTER:
            expected_length = 4; // Address (2) + Value (2)
            break;
            
        case ModbusFunction::WRITE_MULTIPLE_COILS:
        case ModbusFunction::WRITE_MULTIPLE_REGISTERS:
            expected_length = 4; // Address (2) + Quantity (2)
            break;
            
        default:
            std::string error = "Unsupported function code";
            recordError("unsupported_function", std::to_string(request.slaveId));
            return false;
    }
    
    // Read remaining data and CRC
    while (received < expected_length + 2) {
        ssize_t n = read(m_serialPort, buffer + received, expected_length + 2 - received);
        if (n <= 0) {
            if (errno == EAGAIN || errno == EWOULDBLOCK) {
                std::string error = "Response timeout";
                recordError("timeout_error", std::to_string(request.slaveId));
                return false;
            }
            std::string error = "Failed to read response data: " + std::string(strerror(errno));
            recordError("read_error", std::to_string(request.slaveId));
            return false;
        }
        received += n;
    }
    
    // Verify CRC
    uint16_t received_crc = (buffer[received - 1] << 8) | buffer[received - 2];
    uint16_t calculated_crc = calculateCRC(buffer, received - 2);
    if (received_crc != calculated_crc) {
        std::string error = "CRC error";
        recordError("crc_error", std::to_string(request.slaveId));
        return false;
    }
    
    // Store response data
    request.response.assign(buffer, buffer + received);
    request.completed = true;
    request.success = true;
    
    // Calculate response time and record metrics
    auto end_time = std::chrono::steady_clock::now();
    double response_time = std::chrono::duration<double>(end_time - start_time).count();
    
    // Record metrics for successful transaction
    recordMetrics(sent, received, std::to_string(request.slaveId), response_time);
    
    // Update device status
    updateDeviceStatus(std::to_string(request.slaveId), true, response_time);
    
    return true;
}

bool ModbusRTUMaster::start() {
    if (m_running) {
        return true;
    }
    
    // Open serial port
    m_serialPort = open(m_portName.c_str(), O_RDWR | O_NOCTTY | O_NONBLOCK);
    if (m_serialPort < 0) {
        std::string error = "Failed to open serial port: " + std::string(strerror(errno));
        recordError("port_open_error");
        return false;
    }
    
    // Configure serial port
    struct termios tty;
    if (tcgetattr(m_serialPort, &tty) != 0) {
        std::string error = "Failed to get serial port attributes: " + std::string(strerror(errno));
        close(m_serialPort);
        recordError("port_config_error");
        return false;
    }
    
    // Set baud rate
    speed_t speed;
    switch (m_baudRate) {
        case 9600: speed = B9600; break;
        case 19200: speed = B19200; break;
        case 38400: speed = B38400; break;
        case 57600: speed = B57600; break;
        case 115200: speed = B115200; break;
        default:
            std::string error = "Unsupported baud rate: " + std::to_string(m_baudRate);
            close(m_serialPort);
            recordError("invalid_baudrate");
            return false;
    }
    cfsetispeed(&tty, speed);
    cfsetospeed(&tty, speed);
    
    // Set data bits
    tty.c_cflag &= ~CSIZE;
    switch (m_dataBits) {
        case 5: tty.c_cflag |= CS5; break;
        case 6: tty.c_cflag |= CS6; break;
        case 7: tty.c_cflag |= CS7; break;
        case 8: tty.c_cflag |= CS8; break;
        default:
            std::string error = "Unsupported data bits: " + std::to_string(m_dataBits);
            close(m_serialPort);
            recordError("invalid_databits");
            return false;
    }
    
    // Set parity
    switch (m_parity) {
        case SerialParity::NONE:
            tty.c_cflag &= ~PARENB;
            break;
        case SerialParity::ODD:
            tty.c_cflag |= PARENB;
            tty.c_cflag |= PARODD;
            break;
        case SerialParity::EVEN:
            tty.c_cflag |= PARENB;
            tty.c_cflag &= ~PARODD;
            break;
    }
    
    // Set stop bits
    if (m_stopBits == 1) {
        tty.c_cflag &= ~CSTOPB;
    } else if (m_stopBits == 2) {
        tty.c_cflag |= CSTOPB;
    } else {
        std::string error = "Unsupported stop bits: " + std::to_string(m_stopBits);
        close(m_serialPort);
        recordError("invalid_stopbits");
        return false;
    }
    
    // Set timeout
    tty.c_cc[VTIME] = m_timeout / 100; // Convert to deciseconds
    tty.c_cc[VMIN] = 0;
    
    // Set other attributes
    tty.c_cflag |= (CLOCAL | CREAD);
    tty.c_iflag &= ~(IXON | IXOFF | IXANY);
    tty.c_iflag &= ~(INLCR | ICRNL);
    tty.c_oflag &= ~OPOST;
    tty.c_lflag &= ~(ICANON | ECHO | ECHOE | ISIG);
    
    // Apply settings
    if (tcsetattr(m_serialPort, TCSANOW, &tty) != 0) {
        std::string error = "Failed to set serial port attributes: " + std::string(strerror(errno));
        close(m_serialPort);
        recordError("port_config_error");
        return false;
    }
    
    // Start serial thread
    m_serialRunning = true;
    m_serialThread = std::thread(&ModbusRTUMaster::serialThread, this);
    
    m_running = true;
    
    // Update protocol status
    auto& metrics = voltage::comsrv::Metrics::instance();
    metrics.setProtocolStatus(getName(), true);
    
    return true;
}

bool ModbusRTUMaster::stop() {
    if (!m_running) {
        return true;
    }
    
    // Stop serial thread
    m_serialRunning = false;
    m_requestCondition.notify_all();
    if (m_serialThread.joinable()) {
        m_serialThread.join();
    }
    
    // Close serial port
    if (m_serialPort >= 0) {
        close(m_serialPort);
        m_serialPort = -1;
    }
    
    m_running = false;
    
    // Update protocol status
    auto& metrics = voltage::comsrv::Metrics::instance();
    metrics.setProtocolStatus(getName(), false);
    
    return true;
}

void ModbusRTUMaster::serialThread() {
    std::cout << "Serial thread started for: " << getName() << std::endl;
    
    while (m_serialRunning) {
        ModbusRequest request;
        
        // Wait for request
        {
            std::unique_lock<std::mutex> lock(m_requestMutex);
            m_requestCondition.wait(lock, [this] {
                return !m_serialRunning || !m_requestQueue.empty();
            });
            
            if (!m_serialRunning) {
                break;
            }
            
            request = m_requestQueue.front();
            m_requestQueue.pop();
        }
        
        // Process request
        {
            std::lock_guard<std::mutex> lock(m_responseMutex);
            m_currentRequest = &request;
        }
        
        bool success = sendRequest(request);
        
        // Update metrics based on request result
        if (!success) {
            // Error metrics are already recorded in sendRequest
            updateDeviceStatus(std::to_string(request.slaveId), false, 0.0, request.error);
        }
        
        // Notify response
        {
            std::lock_guard<std::mutex> lock(m_responseMutex);
            m_currentRequest = nullptr;
            m_responseCondition.notify_all();
        }
    }
    
    std::cout << "Serial thread stopped for: " << getName() << std::endl;
} 