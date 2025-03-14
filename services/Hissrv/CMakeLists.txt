cmake_minimum_required(VERSION 3.10)
project(hissrv VERSION 1.0)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Find required packages
find_package(PkgConfig REQUIRED)
pkg_check_modules(HIREDIS REQUIRED hiredis)
pkg_check_modules(INFLUXDB REQUIRED influxdb-cxx)

# Add include directories
include_directories(
    ${HIREDIS_INCLUDE_DIRS}
    ${INFLUXDB_INCLUDE_DIRS}
)

# Add executable
add_executable(hissrv
    main.cpp
    config.cpp
    redis_handler.cpp
    influxdb_handler.cpp
)

# Link libraries
target_link_libraries(hissrv
    ${HIREDIS_LIBRARIES}
    ${INFLUXDB_LIBRARIES}
    pthread
)

# Install
install(TARGETS hissrv DESTINATION bin)
install(FILES hissrv.conf DESTINATION /etc/hissrv)
