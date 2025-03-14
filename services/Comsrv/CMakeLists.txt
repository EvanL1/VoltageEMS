cmake_minimum_required(VERSION 3.10)
project(Comsrv VERSION 0.1.0)

# 设置C++标准
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# 查找依赖库
find_package(Threads REQUIRED)
find_package(nlohmann_json QUIET)
if(NOT nlohmann_json_FOUND)
    # 如果没有找到nlohmann_json，则从GitHub下载
    include(FetchContent)
    FetchContent_Declare(json
        GIT_REPOSITORY https://github.com/nlohmann/json.git
        GIT_TAG v3.11.2
    )
    FetchContent_MakeAvailable(json)
endif()

# 添加包含目录
include_directories(${PROJECT_SOURCE_DIR}/include)

# 收集源文件
file(GLOB_RECURSE SOURCES
    "src/*.cpp"
)

# 创建可执行文件
add_executable(comsrv ${SOURCES})

# 链接依赖库
target_link_libraries(comsrv PRIVATE
    Threads::Threads
    nlohmann_json::nlohmann_json
)

# 设置编译选项
if(MSVC)
    target_compile_options(comsrv PRIVATE /W4)
else()
    target_compile_options(comsrv PRIVATE -Wall -Wextra -pedantic)
endif()

# 设置输出目录
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)

# 安装目标
install(TARGETS comsrv DESTINATION bin)
install(DIRECTORY ${PROJECT_SOURCE_DIR}/config/ DESTINATION etc/comsrv)

# 添加tests目录
if(EXISTS ${PROJECT_SOURCE_DIR}/tests)
    enable_testing()
    add_subdirectory(tests)
endif()

# 打印配置信息
message(STATUS "Build type: ${CMAKE_BUILD_TYPE}")
message(STATUS "C++ standard: ${CMAKE_CXX_STANDARD}")
message(STATUS "Compiler: ${CMAKE_CXX_COMPILER_ID} ${CMAKE_CXX_COMPILER_VERSION}")
message(STATUS "Output directory: ${CMAKE_RUNTIME_OUTPUT_DIRECTORY}") 