cmake_minimum_required(VERSION 3.10)
project(comsrv VERSION 1.0)

# 设置C++标准
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# 设置输出目录
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)

# 查找依赖包
find_package(PkgConfig REQUIRED)
pkg_check_modules(LIBMODBUS REQUIRED libmodbus)
pkg_check_modules(HIREDIS REQUIRED hiredis)
pkg_check_modules(JSONCPP REQUIRED jsoncpp)

# 添加编译选项
add_compile_options(-Wall -Wextra)

# 添加头文件路径
include_directories(
    ${PROJECT_SOURCE_DIR}/include
    ${PROJECT_SOURCE_DIR}/include/core
    ${PROJECT_SOURCE_DIR}/include/core/protocols
    ${PROJECT_SOURCE_DIR}/include/core/protocols/modbus
    ${LIBMODBUS_INCLUDE_DIRS}
    ${HIREDIS_INCLUDE_DIRS}
    ${JSONCPP_INCLUDE_DIRS}
)

# 递归查找所有源文件
file(GLOB_RECURSE SOURCES
    "src/core/*.cpp"
)

# 创建可执行文件
add_executable(${PROJECT_NAME} ${SOURCES})

# 链接库
target_link_libraries(${PROJECT_NAME}
    ${LIBMODBUS_LIBRARIES}
    ${HIREDIS_LIBRARIES}
    ${JSONCPP_LIBRARIES}
    pthread
)

# 创建共享库
add_library(lib${PROJECT_NAME} SHARED ${SOURCES})
set_target_properties(lib${PROJECT_NAME} PROPERTIES 
    OUTPUT_NAME ${PROJECT_NAME}
    VERSION ${PROJECT_VERSION}
    SOVERSION ${PROJECT_VERSION_MAJOR}
)

# 链接共享库
target_link_libraries(lib${PROJECT_NAME}
    ${LIBMODBUS_LIBRARIES}
    ${HIREDIS_LIBRARIES}
    ${JSONCPP_LIBRARIES}
    pthread
)

# 安装目标
install(TARGETS ${PROJECT_NAME} lib${PROJECT_NAME}
    RUNTIME DESTINATION bin
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
)

# 安装头文件
install(DIRECTORY ${PROJECT_SOURCE_DIR}/include/
    DESTINATION include/${PROJECT_NAME}
    FILES_MATCHING PATTERN "*.h"
)

# 安装配置文件
install(DIRECTORY config/
    DESTINATION /etc/${PROJECT_NAME}
    FILES_MATCHING PATTERN "*.json" PATTERN "*.csv"
)

# 添加测试支持
enable_testing()

# 打印配置信息
message(STATUS "Build type: ${CMAKE_BUILD_TYPE}")
message(STATUS "C++ standard: ${CMAKE_CXX_STANDARD}")
message(STATUS "Modbus library: ${LIBMODBUS_LIBRARIES}")
message(STATUS "Redis library: ${HIREDIS_LIBRARIES}")
message(STATUS "JSON library: ${JSONCPP_LIBRARIES}") 