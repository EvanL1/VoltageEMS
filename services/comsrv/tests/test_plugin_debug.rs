#[cfg(test)]
mod test_plugin_debug {
    use comsrv::plugins::plugin_registry::{discovery, PluginRegistry};

    #[test]
    fn test_plugin_loading_debug() {
        println!("\n=== 调试插件加载问题 ===\n");

        // 1. 初始状态
        println!("1. 初始状态:");
        let registry = PluginRegistry::global();
        {
            let reg = registry.read().unwrap();
            let stats = reg.get_statistics();
            println!("   插件总数: {}", stats.total_plugins);
            println!("   工厂总数: {}", stats.total_factories);
        }

        // 2. 加载插件
        println!("\n2. 加载插件:");
        match discovery::load_all_plugins() {
            Ok(()) => println!("   插件加载成功"),
            Err(e) => println!("   插件加载失败: {}", e),
        }

        // 3. 查看加载后状态
        println!("\n3. 加载后状态:");
        {
            let reg = registry.read().unwrap();
            let stats = reg.get_statistics();
            println!("   插件总数: {}", stats.total_plugins);
            println!("   工厂总数: {}", stats.total_factories);

            println!("\n   已注册的插件ID:");
            let ids = reg.list_plugin_ids();
            for id in &ids {
                println!("   - {}", id);
            }

            println!("\n   插件类型统计:");
            for (plugin_type, count) in &stats.plugin_types {
                println!("   - {}: {}", plugin_type, count);
            }
        }

        // 4. 尝试获取特定插件
        println!("\n4. 尝试获取插件:");
        {
            let plugin_ids = vec!["modbus_tcp", "modbus_rtu", "iec104", "can", "virtual"];
            for id in plugin_ids {
                if let Some(plugin) = PluginRegistry::get_global(id) {
                    let metadata = plugin.metadata();
                    println!("   ✓ {}: {} v{}", id, metadata.name, metadata.version);
                } else {
                    println!("   ✗ {}: 未找到", id);
                }
            }
        }
    }

    #[test]
    fn test_plugin_factory_creation() {
        // 确保插件已加载
        let _ = discovery::load_all_plugins();

        // 测试插件工厂是否能正确创建实例
        let test_cases = vec![
            ("modbus_tcp", "Modbus TCP Protocol"),
            ("modbus_rtu", "Modbus RTU Protocol"),
            ("virtual", "Virtual Protocol"),
        ];

        for (plugin_id, expected_name) in test_cases {
            println!("\n测试插件工厂: {}", plugin_id);

            if let Some(plugin) = PluginRegistry::get_global(plugin_id) {
                let metadata = plugin.metadata();
                assert_eq!(metadata.id, plugin_id);
                assert!(metadata.name.contains(expected_name));
                println!("  ✓ 插件创建成功: {}", metadata.name);
            } else {
                println!("  ✗ 插件未找到: {}", plugin_id);
            }
        }
    }
}
