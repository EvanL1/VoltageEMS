# ComsRv数据模拟器
FROM python:3.11-slim

# 安装系统依赖
RUN apt-get update && apt-get install -y \
    redis-tools \
    && rm -rf /var/lib/apt/lists/*

# 设置工作目录
WORKDIR /app

# 复制requirements.txt并安装Python依赖
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# 复制模拟器代码
COPY simulator.py .
COPY config.json .

# 创建日志目录
RUN mkdir -p /logs

# 设置环境变量
ENV PYTHONUNBUFFERED=1
ENV REDIS_URL=redis://redis:6379
ENV LOG_LEVEL=INFO

# 运行模拟器
CMD ["python", "simulator.py"]