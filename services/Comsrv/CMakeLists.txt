cmake_minimum_required(VERSION 3.10)
project(comsrv)

# Set C++ standard
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Add prometheus-cpp
include(FetchContent)
FetchContent_Declare(
    prometheus-cpp
    GIT_REPOSITORY https://github.com/jupp0r/prometheus-cpp.git
    GIT_TAG v1.1.0
)
FetchContent_MakeAvailable(prometheus-cpp)

# Add yaml-cpp
FetchContent_Declare(
    yaml-cpp
    GIT_REPOSITORY https://github.com/jbeder/yaml-cpp.git
    GIT_TAG yaml-cpp-0.7.0
)
FetchContent_MakeAvailable(yaml-cpp)

# Add spdlog
FetchContent_Declare(
    spdlog
    GIT_REPOSITORY https://github.com/gabime/spdlog.git
    GIT_TAG v1.12.0
)
FetchContent_MakeAvailable(spdlog)

# Add source files
file(GLOB_RECURSE SOURCES 
    "src/*.cpp"
)

# Add header files
file(GLOB_RECURSE HEADERS 
    "include/*.h"
)

# Create executable
add_executable(comsrv ${SOURCES} ${HEADERS})

# Add include directories
target_include_directories(comsrv PRIVATE 
    ${CMAKE_CURRENT_SOURCE_DIR}/include
)

# Link libraries
target_link_libraries(comsrv PRIVATE
    prometheus-cpp::core
    prometheus-cpp::pull
    yaml-cpp
    spdlog::spdlog
)

# Install targets
install(TARGETS comsrv
    RUNTIME DESTINATION bin
)

# Install configuration files
install(DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/config/
    DESTINATION etc/comsrv
    FILES_MATCHING PATTERN "*.yaml"
    PATTERN "*.csv"
)

# Set compilation options
if(MSVC)
    target_compile_options(comsrv PRIVATE /W4)
else()
    target_compile_options(comsrv PRIVATE -Wall -Wextra -pedantic)
endif()

# Set output directory
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)

# Add tests directory
if(EXISTS ${PROJECT_SOURCE_DIR}/tests)
    enable_testing()
    add_subdirectory(tests)
endif()

# Print configuration information
message(STATUS "Build type: ${CMAKE_BUILD_TYPE}")
message(STATUS "C++ standard: ${CMAKE_CXX_STANDARD}")
message(STATUS "Compiler: ${CMAKE_CXX_COMPILER_ID} ${CMAKE_CXX_COMPILER_VERSION}")
message(STATUS "Output directory: ${CMAKE_RUNTIME_OUTPUT_DIRECTORY}") 