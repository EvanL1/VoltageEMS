#!/bin/bash
# 导入模型模板到Redis

REDIS_HOST="localhost"
REDIS_PORT=6379
REDIS_PREFIX="ems:"

# 检查redis-cli是否可用
if ! command -v redis-cli &> /dev/null; then
    echo "错误: 未找到redis-cli命令"
    exit 1
fi

# 导入指定模板
import_template() {
    local template_file=$1
    local model_id=$2
    
    # 读取模板文件
    local template_content=$(cat $template_file)
    
    # 导入到Redis
    redis-cli -h $REDIS_HOST -p $REDIS_PORT SET "${REDIS_PREFIX}model:config:${model_id}" "$template_content"
    
    echo "已导入模板: $model_id"
}

# 导入所有模板
import_all_templates() {
    local templates_dir="."
    local index_file="${templates_dir}/index.json"
    
    # 检查索引文件是否存在
    if [ ! -f "$index_file" ]; then
        echo "错误: 未找到索引文件 $index_file"
        exit 1
    fi
    
    # 读取索引文件中的模板列表
    local template_ids=$(jq -r '.templates[].id' $index_file)
    local template_files=$(jq -r '.templates[].file' $index_file)
    
    # 将两个列表组合在一起
    local i=0
    for id in $template_ids; do
        local file=$(echo "$template_files" | sed -n "$((i+1))p")
        import_template "${templates_dir}/${file}" "$id"
        i=$((i+1))
    done
}

# 主函数
main() {
    if [ $# -eq 0 ]; then
        # 导入所有模板
        import_all_templates
    else
        # 导入指定模板
        local template_id=$1
        local template_file="./${template_id}.json"
        
        if [ ! -f "$template_file" ]; then
            echo "错误: 未找到模板文件 $template_file"
            exit 1
        fi
        
        import_template "$template_file" "$template_id"
    fi
}

# 执行主函数
main "$@" 