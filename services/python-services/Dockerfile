# 统一的Python服务镜像 - 包含所有4个Python服务
# 使用Python 3.10.12 slim镜像（pandas/numpy需要预编译wheel以提高构建速度）
FROM python:3.10.12-slim

# 设置工作目录
WORKDIR /app

# 设置环境变量
ENV PYTHONPATH=/app
ENV PYTHONUNBUFFERED=1
ENV PYTHONDONTWRITEBYTECODE=1

# 安装系统依赖（参考apigateway的成功配置）
RUN apt-get update && apt-get install -y \
    gcc \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# 合并所有服务的requirements.txt并安装
# 先复制所有requirements.txt文件
COPY services/hissrv/requirements.txt /tmp/hissrv-requirements.txt
COPY services/apigateway/requirements.txt /tmp/apigateway-requirements.txt
COPY services/netsrv/requirements.txt /tmp/netsrv-requirements.txt
COPY services/alarmsrv/requirements.txt /tmp/alarmsrv-requirements.txt

# 合并requirements.txt（智能去重，保留最高版本）
# 1. 提取所有依赖（去掉注释和空行）
# 2. 按包名排序并去重（如果版本相同会自动合并）
# 3. 如果有版本冲突，pip会自动选择兼容版本
RUN cat /tmp/*-requirements.txt | \
    grep -v "^#" | \
    grep -v "^$" | \
    sort -u > /tmp/merged-requirements.txt && \
    pip install --no-cache-dir --upgrade pip && \
    pip install --no-cache-dir -r /tmp/merged-requirements.txt

# 复制所有服务的代码
COPY services/hissrv/ /app/services/hissrv/
COPY services/apigateway/ /app/services/apigateway/
COPY services/netsrv/ /app/services/netsrv/
COPY services/alarmsrv/ /app/services/alarmsrv/

# 创建统一的启动脚本
COPY services/python-services/entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh

# 创建必要目录
RUN mkdir -p logs config

# 暴露所有服务可能使用的端口
EXPOSE 6004 6005 6006 6007

# 使用统一的启动脚本
ENTRYPOINT ["/app/entrypoint.sh"]

