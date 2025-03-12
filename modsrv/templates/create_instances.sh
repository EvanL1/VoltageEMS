#!/bin/bash
# 基于模板创建多个实例并导入到Redis

REDIS_HOST="localhost"
REDIS_PORT=6379
REDIS_PREFIX="ems:"

# 检查redis-cli是否可用
if ! command -v redis-cli &> /dev/null; then
    echo "错误: 未找到redis-cli命令"
    exit 1
fi

# 检查jq是否可用
if ! command -v jq &> /dev/null; then
    echo "错误: 未找到jq命令，请安装jq"
    exit 1
fi

# 使用说明
usage() {
    echo "使用方法: $0 <模板ID> <实例数量> [实例前缀] [起始索引]"
    echo "例如: $0 stepper_motor_model 16 stepper 1"
    echo "将创建16个步进电机模型实例，ID为stepper_1到stepper_16"
    exit 1
}

# 导入模板实例到Redis
import_instance() {
    local instance_json=$1
    local instance_id=$2
    
    # 导入到Redis
    echo "$instance_json" | redis-cli -h $REDIS_HOST -p $REDIS_PORT -x SET "${REDIS_PREFIX}model:config:${instance_id}"
    
    echo "已导入实例: $instance_id"
}

# 基于模板创建实例
create_instance() {
    local template_json=$1
    local instance_id=$2
    local instance_num=$3
    
    # 使用jq修改模板
    local instance_json=$(echo "$template_json" | jq \
        --arg id "$instance_id" \
        --arg name "$(echo "$template_json" | jq -r '.name') #$instance_num" \
        --arg source_key "${REDIS_PREFIX}data:${instance_id}" \
        --arg output_key "${REDIS_PREFIX}model:output:${instance_id}" \
        --arg channel "${instance_id}_Control" \
        '.id = $id | .name = $name | 
         .input_mappings = [.input_mappings[] | .source_key = $source_key] | 
         .output_key = $output_key | 
         .actions = [.actions[] | if .channel == "Stepper_Control" then .channel = $channel else . end]')
    
    import_instance "$instance_json" "$instance_id"
}

# 主函数
main() {
    if [ $# -lt 2 ]; then
        usage
    fi
    
    local template_id=$1
    local instance_count=$2
    local instance_prefix=${3:-"instance"}
    local start_index=${4:-1}
    
    # 检查模板文件是否存在
    local template_file="./${template_id}.json"
    if [ ! -f "$template_file" ]; then
        echo "错误: 未找到模板文件 $template_file"
        exit 1
    fi
    
    # 读取模板文件
    local template_json=$(cat "$template_file")
    
    # 创建实例
    for ((i=start_index; i<start_index+instance_count; i++)); do
        local instance_id="${instance_prefix}_${i}"
        create_instance "$template_json" "$instance_id" "$i"
    done
    
    echo "成功创建 $instance_count 个实例"
}

# 执行主函数
main "$@" 