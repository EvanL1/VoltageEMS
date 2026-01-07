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
