#!/bin/bash

echo "检查 HisSrv 代码语法..."

# 检查是否有明显的语法错误
echo "检查 Rust 文件语法..."
find src -name "*.rs" | while read file; do
    echo -n "检查 $file ... "
    if rustc --crate-type lib "$file" -Z parse-only 2>/dev/null; then
        echo "OK"
    else
        echo "FAILED"
        rustc --crate-type lib "$file" -Z parse-only 2>&1 | head -5
    fi
done

echo ""
echo "检查模块导入..."
grep -r "^mod " src/ | grep -v "^src/tests" | sort | uniq

echo ""
echo "检查未使用的导入..."
grep -r "^use " src/main.rs | head -20

echo ""
echo "检查配置文件..."
for file in hissrv.yaml hissrv-dev.yaml; do
    if [ -f "$file" ]; then
        echo "$file 存在"
    else
        echo "$file 不存在"
    fi
done

echo ""
echo "检查测试文件数量..."
find src/tests -name "*.rs" | wc -l

echo ""
echo "完成检查。"