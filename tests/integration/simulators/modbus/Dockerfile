FROM python:3.11-slim

# 安装依赖
RUN pip install --no-cache-dir \
    pymodbus==3.5.4 \
    numpy \
    pyyaml

# 创建工作目录
WORKDIR /app

# 复制模拟器代码
COPY modbus_simulator.py /app/

# 暴露Modbus端口
EXPOSE 502

# 设置环境变量
ENV PYTHONUNBUFFERED=1

# 运行模拟器
CMD ["python", "modbus_simulator.py"]