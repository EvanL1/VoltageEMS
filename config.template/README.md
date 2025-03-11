# 配置示例

VoltageEMS 服务配置示例文件，供参考使用。

## 使用方法

1. 复制到 `config/` 目录
2. 根据实际环境修改配置
3. 使用 Monarch 同步到数据库

```bash
cp -r config-example config
monarch init all
monarch sync all
```

## 目录结构

- `comsrv/` - 通信服务配置
- `modsrv/` - 模型服务配置及实例定义
- `rulesrv/` - 规则引擎配置
