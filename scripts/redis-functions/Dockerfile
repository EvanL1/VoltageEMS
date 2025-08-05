FROM redis:8-alpine

# 安装 bash (Alpine 默认只有 sh)
RUN apk add --no-cache bash

# 复制所有 Redis 函数脚本
COPY scripts/redis-functions /scripts

# 设置环境变量
ENV REDIS_HOST=localhost
ENV REDIS_PORT=6379

# 设置工作目录
WORKDIR /scripts

# 使用自定义入口点
ENTRYPOINT ["/scripts/docker-entrypoint.sh"]