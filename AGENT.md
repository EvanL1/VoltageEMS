# VoltageEMS AI Agent Documentation

## Overview

This document describes the AI agent capabilities and guidelines for the VoltageEMS (Energy Management System) project. The AI agent is designed to assist with development, maintenance, and enhancement of this industrial-grade energy management system.

## Project Architecture

VoltageEMS is a comprehensive energy management system built with a microservices architecture:

### Core Services
- **comsrv**: Communication service handling various industrial protocols
- **hissrv**: Historical data service for time-series data management
- **modsrv**: Model service for data modeling and analytics
- **netsrv**: Network service for communication infrastructure

### Frontend
- **Electron-based desktop application** with modern web technologies
- **Vue.js/React-based UI** for configuration and monitoring

### Supported Protocols
- **Modbus** (RTU/TCP)
- **IEC 60870-5-104**
- **CAN Bus**
- **Custom protocols** through extensible framework

## AI Agent Capabilities

### 1. Code Development & Maintenance
- **Rust backend development** with proper error handling and documentation
- **Frontend development** using modern JavaScript/TypeScript frameworks
- **Protocol implementation** for industrial communication standards
- **Database integration** and data modeling

### 2. Documentation & Standards
- **Technical documentation** generation and maintenance
- **API documentation** with proper schemas
- **Code comments** in English following project standards
- **Architecture diagrams** and system design documentation

### 3. Testing & Quality Assurance
- **Unit test generation** with `#[cfg(test)]` attributes for Rust
- **Integration testing** for protocol implementations
- **Performance testing** and optimization suggestions
- **Code review** and best practices enforcement

### 4. Configuration Management
- **Environment-based configuration** setup
- **Protocol configuration** templates and validation
- **Deployment configuration** for different environments

## Development Guidelines

### Code Standards
1. **Language Requirements**:
   - All code comments and documentation in English
   - User communication in Chinese
   - Consistent naming conventions

2. **Rust Development**:
   - Use custom error types (`ModelSrvError`, `ComSrvError`, etc.)
   - Implement proper error propagation with `?` operator
   - Add comprehensive test coverage with `#[cfg(test)]`
   - Use triple-slash `///` for function documentation

3. **Git Practices**:
   - Conventional commits: `feat(service): description`
   - English commit messages with detailed descriptions
   - Proper scope identification in commits

### File Organization
```
services/
├── comsrv/          # Communication service
│   ├── src/
│   │   ├── api/     # REST API endpoints
│   │   ├── core/    # Core business logic
│   │   └── protocols/ # Protocol implementations
├── hissrv/          # Historical data service
├── modsrv/          # Model service
└── netsrv/          # Network service

frontend/
├── electron/        # Electron main process
├── src/
│   ├── components/  # Vue/React components
│   ├── views/       # Application views
│   └── utils/       # Utility functions
```

## Protocol Development

### Adding New Protocols
1. **Create protocol module** under `services/comsrv/src/core/protocols/`
2. **Implement common traits** from `protocols/common/`
3. **Add configuration support** in respective config modules
4. **Write comprehensive tests** for protocol functionality
5. **Update documentation** with protocol specifications

### Common Components
- **protocol_factory.rs**: Protocol instantiation and management
- **connection_pool.rs**: Connection pooling for network protocols
- **error handling**: Standardized error types across protocols

## Configuration Management

### Environment Variables
- Use `.env` files for development configuration
- Provide fallback mechanisms for missing configurations
- Document all configuration options in service-specific docs

### Protocol Configuration
- YAML-based configuration files
- Validation schemas for configuration integrity
- Hot-reload capabilities where applicable

## Monitoring & Logging

### Logging Standards
- **Structured logging** with appropriate levels (info, error, debug)
- **Context inclusion** in log messages
- **Error tracking** with detailed stack traces
- **Performance metrics** logging for optimization

### Monitoring
- **Health check endpoints** for all services
- **Metrics collection** for system performance
- **Alert mechanisms** for critical system events

## Testing Strategy

### Unit Testing
- **Rust services**: Use `#[cfg(test)]` modules
- **Frontend**: Jest/Vitest for component testing
- **Protocol testing**: Mock device simulators

### Integration Testing
- **End-to-end protocol communication** testing
- **Service interaction** validation
- **Database integration** testing

### Performance Testing
- **Load testing** for communication services
- **Stress testing** for protocol handlers
- **Memory usage** profiling and optimization

## Deployment

### Development Environment
- **Docker containers** for service isolation
- **Local database** setup (PostgreSQL/InfluxDB)
- **Development tools** and debugging setup

### Production Deployment
- **Containerized deployment** with Docker Compose
- **Environment-specific configurations**
- **Monitoring and alerting** setup
- **Backup and recovery** procedures

## AI Agent Usage Guidelines

### When to Engage the AI Agent
1. **New feature development** requiring protocol implementation
2. **Code review and optimization** suggestions
3. **Documentation generation** and updates
4. **Testing strategy** development and implementation
5. **Architecture decisions** and design patterns
6. **Troubleshooting** complex system issues

### Best Practices for AI Interaction
1. **Provide context** about the specific service or component
2. **Include relevant error messages** or logs when troubleshooting
3. **Specify requirements** clearly for new features
4. **Ask for explanations** of complex implementations
5. **Request test coverage** for new code

## Contributing

### Code Contributions
1. Follow established coding standards
2. Include comprehensive tests
3. Update documentation as needed
4. Use conventional commit messages
5. Ensure all services build and pass tests

### Documentation Contributions
1. Keep technical documentation up to date
2. Include examples and usage scenarios
3. Maintain consistency across services
4. Update this AGENT.md when adding new capabilities

## Support and Maintenance

### Regular Maintenance Tasks
- **Dependency updates** and security patches
- **Performance monitoring** and optimization
- **Documentation reviews** and updates
- **Test coverage** analysis and improvement

### Troubleshooting Resources
- **Service-specific logs** in respective `logs/` directories
- **Configuration validation** tools
- **Protocol debugging** utilities
- **Performance profiling** tools

---

*This document is maintained by the AI agent and should be updated as the project evolves and new capabilities are added.* 