# 第一阶段：编译环境
FROM python:3.10.12-slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y gcc python3-dev
COPY services/hissrv/requirements.txt /tmp/hissrv-requirements.txt
COPY services/apigateway/requirements.txt /tmp/apigateway-requirements.txt
COPY services/netsrv/requirements.txt /tmp/netsrv-requirements.txt
COPY services/alarmsrv/requirements.txt /tmp/alarmsrv-requirements.txt
RUN cat /tmp/*-requirements.txt | grep -v "^#" | grep -v "^$" | sort -u > /tmp/merged-requirements.txt
# 将依赖安装到特定目录
RUN pip install --no-cache-dir --prefix=/install -r /tmp/merged-requirements.txt

# 第二阶段：运行环境 (最终镜像)
FROM python:3.10.12-slim
WORKDIR /app
ENV PYTHONPATH=/app

# 安装 Docker CLI（升级功能需要）
# 说明：apigateway 需要通过 Docker Socket 创建升级容器
# Docker CLI 通过挂载的 /var/run/docker.sock 与宿主机 Docker Daemon 通信
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    gnupg \
    && install -m 0755 -d /etc/apt/keyrings \
    && curl -fsSL https://download.docker.com/linux/debian/gpg -o /etc/apt/keyrings/docker.asc \
    && chmod a+r /etc/apt/keyrings/docker.asc \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/debian $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null \
    && apt-get update \
    && apt-get install -y --no-install-recommends docker-ce-cli \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# 从编译阶段只复制安装好的库，不复制 gcc 等垃圾
COPY --from=builder /install /usr/local
COPY services/hissrv/ /app/services/hissrv/
COPY services/apigateway/ /app/services/apigateway/
COPY services/netsrv/ /app/services/netsrv/
COPY services/alarmsrv/ /app/services/alarmsrv/
COPY services/python-services/entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh && \
    mkdir -p logs config && \
    chmod 777 logs config && \
    ln -sfn /app/logs /app/services/hissrv/logs && \
    ln -sfn /app/config /app/services/hissrv/config && \
    ln -sfn /app/logs /app/services/apigateway/logs && \
    ln -sfn /app/config /app/services/apigateway/config && \
    ln -sfn /app/logs /app/services/netsrv/logs && \
    ln -sfn /app/config /app/services/netsrv/config && \
    ln -sfn /app/logs /app/services/alarmsrv/logs && \
    ln -sfn /app/config /app/services/alarmsrv/config
EXPOSE 6004 6005 6006 6007
ENTRYPOINT ["/app/entrypoint.sh"]
