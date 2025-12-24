# 使用Python 3.11 slim镜像，支持aarch64
FROM python:3.11-slim

# 设置工作目录
WORKDIR /app

# 设置环境变量
ENV PYTHONPATH=/app
ENV PYTHONUNBUFFERED=1
ENV PYTHONDONTWRITEBYTECODE=1

# 安装系统依赖（只安装必要的）
RUN apt-get update && apt-get install -y \
    gcc \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# 复制依赖文件
COPY requirements.txt .

# 安装Python依赖（优化pip缓存）
RUN pip install --no-cache-dir --upgrade pip && \
    pip install --no-cache-dir -r requirements.txt

# 复制应用代码
COPY . .

# 创建必要目录并设置权限（允许任意用户写入）
RUN mkdir -p logs config && chmod 777 logs

# 暴露端口
EXPOSE 6005

# 启动命令
CMD ["python", "main.py"]
