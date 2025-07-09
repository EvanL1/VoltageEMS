//! 插件注册表单元测试
//!
//! 测试插件的注册、查询、加载和卸载功能

use comsrv::core::plugins::{PluginRegistry, ProtocolPlugin, ProtocolMetadata};
use std::sync::Arc;
use async_trait::async_trait;

/// 创建测试用的模拟插件
struct TestPlugin {
    id: String,
    name: String,
}

#[async_trait]
impl ProtocolPlugin for TestPlugin {
    fn metadata(&self) -> ProtocolMetadata {
        ProtocolMetadata {
            id: self.id.clone(),
            name: self.name.clone(),
            version: "1.0.0".to_string(),
            description: "Test plugin".to_string(),
            author: "Test".to_string(),
            license: "MIT".to_string(),
            features: vec![],
            dependencies: std::collections::HashMap::new(),
        }
    }
    
    fn config_template(&self) -> Vec<comsrv::core::plugins::ConfigTemplate> {
        vec![]
    }
    
    fn validate_config(&self, _config: &std::collections::HashMap<String, serde_json::Value>) -> comsrv::utils::Result<()> {
        Ok(())
    }
    
    async fn create_instance(
        &self,
        _channel_config: comsrv::core::config::types::channel::ChannelConfig,
    ) -> comsrv::utils::Result<Box<dyn comsrv::core::protocols::common::traits::ComBase>> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_plugin_registration() {
        let mut registry = PluginRegistry::new();
        
        // 创建测试插件
        let plugin = Arc::new(TestPlugin {
            id: "test_protocol".to_string(),
            name: "Test Protocol".to_string(),
        });
        
        // 注册插件
        registry.register("test_protocol", plugin.clone());
        
        // 验证注册成功
        assert!(registry.get("test_protocol").is_some());
        assert!(registry.get("unknown_protocol").is_none());
    }
    
    #[test]
    fn test_plugin_list() {
        let mut registry = PluginRegistry::new();
        
        // 注册多个插件
        registry.register("protocol1", Arc::new(TestPlugin {
            id: "protocol1".to_string(),
            name: "Protocol 1".to_string(),
        }));
        
        registry.register("protocol2", Arc::new(TestPlugin {
            id: "protocol2".to_string(),
            name: "Protocol 2".to_string(),
        }));
        
        // 获取插件列表
        let plugins = registry.list();
        assert_eq!(plugins.len(), 2);
        assert!(plugins.contains(&"protocol1".to_string()));
        assert!(plugins.contains(&"protocol2".to_string()));
    }
    
    #[test]
    fn test_plugin_metadata_list() {
        let mut registry = PluginRegistry::new();
        
        // 注册插件
        registry.register("test_protocol", Arc::new(TestPlugin {
            id: "test_protocol".to_string(),
            name: "Test Protocol".to_string(),
        }));
        
        // 获取元数据列表
        let metadata_list = registry.list_metadata();
        assert_eq!(metadata_list.len(), 1);
        
        let metadata = &metadata_list[0];
        assert_eq!(metadata.id, "test_protocol");
        assert_eq!(metadata.name, "Test Protocol");
        assert_eq!(metadata.version, "1.0.0");
    }
    
    #[test]
    fn test_plugin_removal() {
        let mut registry = PluginRegistry::new();
        
        // 注册插件
        let plugin = Arc::new(TestPlugin {
            id: "test_protocol".to_string(),
            name: "Test Protocol".to_string(),
        });
        registry.register("test_protocol", plugin);
        
        // 验证存在
        assert!(registry.get("test_protocol").is_some());
        
        // 移除插件
        registry.remove("test_protocol");
        
        // 验证已移除
        assert!(registry.get("test_protocol").is_none());
    }
    
    #[test]
    fn test_duplicate_registration() {
        let mut registry = PluginRegistry::new();
        
        // 第一次注册
        registry.register("test_protocol", Arc::new(TestPlugin {
            id: "test_protocol".to_string(),
            name: "Original".to_string(),
        }));
        
        // 再次注册相同ID（应该覆盖）
        registry.register("test_protocol", Arc::new(TestPlugin {
            id: "test_protocol".to_string(),
            name: "Updated".to_string(),
        }));
        
        // 验证被覆盖
        let plugin = registry.get("test_protocol").unwrap();
        let metadata = plugin.metadata();
        assert_eq!(metadata.name, "Updated");
    }
    
    #[test]
    fn test_clear_registry() {
        let mut registry = PluginRegistry::new();
        
        // 注册多个插件
        registry.register("protocol1", Arc::new(TestPlugin {
            id: "protocol1".to_string(),
            name: "Protocol 1".to_string(),
        }));
        
        registry.register("protocol2", Arc::new(TestPlugin {
            id: "protocol2".to_string(),
            name: "Protocol 2".to_string(),
        }));
        
        assert_eq!(registry.list().len(), 2);
        
        // 清空注册表
        registry.clear();
        
        assert_eq!(registry.list().len(), 0);
        assert!(registry.get("protocol1").is_none());
        assert!(registry.get("protocol2").is_none());
    }
    
    #[test]
    fn test_thread_safety() {
        use std::thread;
        use std::sync::Mutex;
        
        let registry = Arc::new(Mutex::new(PluginRegistry::new()));
        let mut handles = vec![];
        
        // 多线程同时注册插件
        for i in 0..10 {
            let registry_clone = Arc::clone(&registry);
            let handle = thread::spawn(move || {
                let plugin = Arc::new(TestPlugin {
                    id: format!("protocol_{}", i),
                    name: format!("Protocol {}", i),
                });
                
                let mut reg = registry_clone.lock().unwrap();
                reg.register(&format!("protocol_{}", i), plugin);
            });
            handles.push(handle);
        }
        
        // 等待所有线程完成
        for handle in handles {
            handle.join().unwrap();
        }
        
        // 验证所有插件都已注册
        let reg = registry.lock().unwrap();
        assert_eq!(reg.list().len(), 10);
        
        for i in 0..10 {
            assert!(reg.get(&format!("protocol_{}", i)).is_some());
        }
    }
}

/// 全局注册表测试
#[cfg(test)]
mod global_registry_tests {
    use super::*;
    
    #[test]
    fn test_global_registry() {
        // 清空全局注册表
        PluginRegistry::clear_global();
        
        // 注册到全局注册表
        let plugin = Arc::new(TestPlugin {
            id: "global_test".to_string(),
            name: "Global Test".to_string(),
        });
        
        PluginRegistry::register_global("global_test", plugin);
        
        // 从全局注册表获取
        assert!(PluginRegistry::get_global("global_test").is_some());
        
        // 列出全局注册表
        let global_list = PluginRegistry::list_global();
        assert!(global_list.contains(&"global_test".to_string()));
        
        // 清理
        PluginRegistry::clear_global();
    }
}