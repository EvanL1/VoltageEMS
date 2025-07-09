# Hex 库优化示例

## 当前代码（使用 format!）

```rust
// 当前的十六进制格式化
debug!(hex_data = %data.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" "), 
       length = data.len(), 
       "[PDU Parser] Raw PDU data");
```

## 优化后（使用 hex 库）

```rust
use hex;

// 使用 hex::encode 转换整个字节数组
debug!(hex_data = %hex::encode_upper(&data), 
       length = data.len(), 
       "[PDU Parser] Raw PDU data");

// 如果需要空格分隔的格式
debug!(hex_data = %hex::encode_upper(&data)
         .chars()
         .collect::<Vec<_>>()
         .chunks(2)
         .map(|chunk| chunk.iter().collect::<String>())
         .collect::<Vec<_>>()
         .join(" "), 
       length = data.len(), 
       "[PDU Parser] Raw PDU data");

// 或者创建一个辅助函数
fn format_hex_with_spaces(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

// 更简洁的方式
fn to_hex_string(data: &[u8]) -> String {
    hex::encode_upper(data)
}
```

## 优势

1. **性能**：hex 库经过优化，比迭代 format! 更快
2. **简洁**：代码更短，更易读
3. **功能丰富**：支持编码/解码，大小写转换等

## 建议

在 Modbus 协议调试日志中大量使用十六进制格式化，建议：
1. 保留 hex 依赖
2. 创建一个通用的十六进制格式化辅助模块
3. 逐步将现有的 format! 调用替换为 hex 库调用