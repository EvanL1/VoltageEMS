FROM python:3.11-slim

# 安装依赖
RUN pip install --no-cache-dir \
    numpy \
    pyyaml \
    struct

# 创建工作目录
WORKDIR /app

# 复制模拟器代码
COPY iec104_simulator.py /app/

# 暴露IEC104端口
EXPOSE 2404

# 设置环境变量
ENV PYTHONUNBUFFERED=1

# 运行模拟器
CMD ["python", "iec104_simulator.py"]