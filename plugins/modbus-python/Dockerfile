FROM python:3.11-slim

WORKDIR /app

# 安装依赖
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# 复制 protobuf 文件
COPY proto/protocol_plugin.proto ./proto/

# 创建src目录并生成 protobuf 代码
RUN mkdir -p src && \
    python -m grpc_tools.protoc \
    -I./proto \
    --python_out=./src \
    --grpc_python_out=./src \
    ./proto/protocol_plugin.proto

# 复制源代码
COPY src/ ./src/

# 设置环境变量
ENV PYTHONPATH=/app
ENV PYTHONUNBUFFERED=1
ENV GRPC_PORT=50051

# 暴露端口
EXPOSE 50051

# 运行服务
CMD ["python", "src/server.py"]