# Multi-stage build for minimal runtime image

# Build stage
FROM ubuntu:22.04 as builder

# Set timezone
ENV TZ=Asia/Shanghai
RUN ln -snf /usr/share/zoneinfo/$TZ /etc/localtime && echo $TZ > /etc/timezone

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    pkg-config \
    libmodbus-dev \
    libhiredis-dev \
    libjsoncpp-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /build

# Copy source code
COPY . .

# Build project
RUN mkdir -p build && cd build \
    && cmake .. \
    && make -j$(nproc) \
    && make install DESTDIR=/install

# Runtime stage - minimal image
FROM ubuntu:22.04

# Set timezone
ENV TZ=Asia/Shanghai
RUN ln -snf /usr/share/zoneinfo/$TZ /etc/localtime && echo $TZ > /etc/timezone

# Install only runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    libmodbus5 \
    libhiredis0.14 \
    libjsoncpp25 \
    && rm -rf /var/lib/apt/lists/*

# Create necessary directories
RUN mkdir -p /etc/comsrv /var/log/comsrv

# Copy only the built application and necessary files from builder
COPY --from=builder /install/usr/local/bin/comsrv /usr/local/bin/
COPY --from=builder /install/usr/local/lib/libcomsrv.so* /usr/local/lib/
COPY config /etc/comsrv/

# Update library cache
RUN ldconfig

# Set environment variables
ENV CONFIG_PATH=/etc/comsrv
ENV LOG_PATH=/var/log/comsrv

# Expose Modbus TCP port
EXPOSE 502

# Set entrypoint
ENTRYPOINT ["/usr/local/bin/comsrv", "-c", "/etc/comsrv", "-l", "/var/log/comsrv"] 