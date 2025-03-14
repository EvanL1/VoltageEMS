#include "core/comBase.h"
#include <iostream>

ComBase::ComBase(const std::string& name)
    : m_name(name), m_running(false)
{
    std::cout << "Creating communication instance: " << m_name << std::endl;
}

ComBase::~ComBase()
{
    if (m_running) {
        stop();
    }
    std::cout << "Destroying communication instance: " << m_name << std::endl;
}

bool ComBase::isRunning() const
{
    return m_running;
}

std::string ComBase::getName() const
{
    return m_name;
} 