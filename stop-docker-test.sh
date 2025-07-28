#!/bin/bash
#
# 停止Comsrv Docker测试环境
#

echo "=== 停止Comsrv Docker测试环境 ==="
echo "时间: $(date '+%Y-%m-%d %H:%M:%S')"
echo ""

# 收集测试结果
echo "收集测试结果..."
if [ -f "test-results/validation_result.json" ]; then
    echo "验证结果已保存到: test-results/validation_result.json"
    echo ""
    echo "=== 验证摘要 ==="
    cat test-results/validation_result.json | grep -E '"status"|"total_points_found"|"total_issues"' || echo "无法读取验证结果"
    echo ""
fi

# 生成日志归档
echo "归档日志文件..."
ARCHIVE_NAME="test-logs-$(date +%Y%m%d-%H%M%S).tar.gz"
tar -czf "$ARCHIVE_NAME" logs/ test-results/ 2>/dev/null || true
echo "日志已归档到: $ARCHIVE_NAME"

# 停止并删除容器
echo ""
echo "停止所有容器..."
docker-compose down

# 询问是否删除数据卷
echo ""
read -p "是否删除数据卷? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "删除数据卷..."
    docker-compose down -v
fi

# 显示容器状态
echo ""
echo "=== 容器状态 ==="
docker-compose ps

echo ""
echo "=== 测试环境已停止 ==="
echo ""