#!/bin/bash

# 配置
REDIS_HOST="localhost"
REDIS_PORT="6379"
REDIS_PREFIX="ems:"

# 检查redis-cli是否可用
check_redis_cli() {
    if ! command -v redis-cli &> /dev/null; then
        echo "错误: redis-cli 未安装，请先安装 Redis 客户端工具"
        exit 1
    fi
}

# 检查jq是否可用
check_jq() {
    if ! command -v jq &> /dev/null; then
        echo "错误: jq 未安装，请先安装 jq 工具"
        exit 1
    fi
}

# 显示使用帮助
usage() {
    echo "使用方法: $0 <模板ID> <实例数量> [实例前缀] [起始索引]"
    echo ""
    echo "参数:"
    echo "  <模板ID>     - 要使用的模板ID，必须存在于index.json中"
    echo "  <实例数量>   - 要创建的实例数量"
    echo "  [实例前缀]   - 实例ID的前缀，默认为'instance'"
    echo "  [起始索引]   - 实例ID的起始索引，默认为1"
    echo ""
    echo "示例:"
    echo "  $0 stepper_motor_model 16 motor 1"
    echo "  将创建16个步进电机模型实例，ID从motor_1到motor_16"
    exit 1
}

# 导入实例到Redis
import_to_redis() {
    local key="$1"
    local json="$2"
    
    echo "正在导入 $key..."
    echo "$json" | redis-cli -h "$REDIS_HOST" -p "$REDIS_PORT" -x SET "$key" > /dev/null
    
    if [ $? -ne 0 ]; then
        echo "错误: 无法导入 $key 到 Redis"
        return 1
    fi
    
    return 0
}

# 创建实例
create_instance() {
    local template_id="$1"
    local instance_id="$2"
    local instance_name="$3"
    
    # 检查模板文件是否存在
    local template_file=$(jq -r ".templates[] | select(.id == \"$template_id\") | .file" index.json)
    
    if [ -z "$template_file" ] || [ "$template_file" == "null" ]; then
        echo "错误: 找不到模板 $template_id"
        return 1
    fi
    
    if [ ! -f "$template_file" ]; then
        echo "错误: 模板文件 $template_file 不存在"
        return 1
    fi
    
    # 读取模板内容
    local template_content=$(cat "$template_file")
    
    # 修改ID和名称
    local instance_content=$(echo "$template_content" | jq ".model.id = \"$instance_id\" | .model.name = \"$instance_name\"")
    
    # 修改数据源键
    local source_key="${REDIS_PREFIX}data:$instance_id"
    instance_content=$(echo "$instance_content" | jq ".model.input_mappings[].source_key = \"$source_key\"")
    
    # 修改输出键
    local output_key="${REDIS_PREFIX}model:output:$instance_id"
    instance_content=$(echo "$instance_content" | jq ".model.output_key = \"$output_key\"")
    
    # 修改控制通道
    instance_content=$(echo "$instance_content" | jq "(.actions[] | select(.channel | contains(\"Control\")) | .channel) |= \"${instance_id}_Control\"")
    
    # 导入到Redis
    local redis_key="${REDIS_PREFIX}model:config:$instance_id"
    import_to_redis "$redis_key" "$instance_content"
    
    return $?
}

# 主函数
main() {
    # 检查工具依赖
    check_redis_cli
    check_jq
    
    # 检查参数
    if [ $# -lt 2 ]; then
        usage
    fi
    
    local template_id="$1"
    local instance_count="$2"
    local instance_prefix="${3:-instance}"
    local start_index="${4:-1}"
    
    # 检查模板是否存在
    if ! jq -e ".templates[] | select(.id == \"$template_id\")" index.json > /dev/null; then
        echo "错误: 模板 $template_id 不存在"
        exit 1
    fi
    
    # 创建实例
    local success_count=0
    
    for ((i=start_index; i<start_index+instance_count; i++)); do
        local instance_id="${instance_prefix}_$i"
        local instance_name="$template_id #$i"
        
        echo "正在创建实例 $instance_id..."
        
        if create_instance "$template_id" "$instance_id" "$instance_name"; then
            echo "成功创建实例 $instance_id"
            ((success_count++))
        else
            echo "创建实例 $instance_id 失败"
        fi
    done
    
    echo ""
    echo "完成: 成功创建 $success_count/$instance_count 个实例"
}

# 执行主函数
main "$@" 