#ifndef COM_BASE_H
#define COM_BASE_H

#include <string>
#include <memory>
#include <vector>
#include <mutex>
#include <atomic>

/**
 * @brief Base class for all communication protocols
 * 
 * This class defines the interface for all communication protocols.
 * It provides basic functionality for starting, stopping, and managing
 * communication sessions.
 */
class ComBase {
public:
    /**
     * @brief Constructor
     * 
     * @param name Name of the communication instance
     */
    ComBase(const std::string& name);

    /**
     * @brief Virtual destructor
     */
    virtual ~ComBase();

    /**
     * @brief Start the communication
     * 
     * @return true if started successfully, false otherwise
     */
    virtual bool start() = 0;

    /**
     * @brief Stop the communication
     * 
     * @return true if stopped successfully, false otherwise
     */
    virtual bool stop() = 0;

    /**
     * @brief Check if the communication is running
     * 
     * @return true if running, false otherwise
     */
    bool isRunning() const;

    /**
     * @brief Get the name of the communication instance
     * 
     * @return Name of the communication instance
     */
    std::string getName() const;

protected:
    std::string m_name;
    std::atomic<bool> m_running;
    std::mutex m_mutex;
};

#endif // COM_BASE_H 