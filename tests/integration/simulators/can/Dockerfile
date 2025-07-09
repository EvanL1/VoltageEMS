FROM python:3.11-slim

# 安装系统依赖
RUN apt-get update && apt-get install -y \
    can-utils \
    iproute2 \
    && rm -rf /var/lib/apt/lists/*

# 安装Python依赖
RUN pip install --no-cache-dir \
    python-can \
    numpy \
    pyyaml

# 创建工作目录
WORKDIR /app

# 复制模拟器代码
COPY can_simulator.py /app/

# 设置环境变量
ENV PYTHONUNBUFFERED=1

# 运行模拟器
CMD ["python", "can_simulator.py"]