FROM python:3.11-slim

# Install pymodbus
RUN pip install pymodbus==3.5.2

# Copy the custom server script
COPY docker/modbus-sim/custom_modbus_server.py /app/modbus_server.py
COPY docker/modbus-sim/enhanced_modbus_server.py /app/enhanced_modbus_server.py

WORKDIR /app

# Expose Modbus TCP port
EXPOSE 502

# Run the enhanced server for comprehensive testing
CMD ["python", "/app/enhanced_modbus_server.py"]