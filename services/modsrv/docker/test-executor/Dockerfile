# 测试执行器镜像
FROM python:3.11-slim

# 安装系统依赖和工具
RUN apt-get update && apt-get install -y \
    curl \
    redis-tools \
    jq \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 安装Python依赖
RUN pip install --no-cache-dir \
    requests==2.31.0 \
    redis==5.0.1 \
    pytest==7.4.0 \
    pytest-asyncio==0.21.1 \
    PyYAML==6.0.1

# 复制测试脚本
COPY run-tests.sh /app/
COPY test-*.py /app/
COPY api_test_suite.py /app/

# 创建结果目录
RUN mkdir -p /app/results /app/logs

# 设置执行权限
RUN chmod +x /app/run-tests.sh

# 设置环境变量
ENV PYTHONUNBUFFERED=1

CMD ["/app/run-tests.sh"]