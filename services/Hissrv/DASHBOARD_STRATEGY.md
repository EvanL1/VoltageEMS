# 仪表板策略指南

## 1. 直接使用 Grafana 的优势

### 1.1 开箱即用的功能
- **60+ 种可视化组件**：时序图、热力图、状态图、地理图等
- **强大的查询编辑器**：支持复杂的数据转换和计算
- **模板变量**：动态切换设备、时间范围、聚合方式
- **告警系统**：可视化配置告警规则
- **注释功能**：在图表上标记重要事件
- **导入/导出**：轻松分享和复用仪表板

### 1.2 无需额外开发
- 不用开发自己的图表组件
- 不用实现数据聚合逻辑
- 不用处理时区转换
- 不用优化渲染性能

## 2. 推荐的仪表板组织结构

```
grafana/dashboards/
├── overview/              # 总览类
│   ├── system-overview.json      # 系统总览
│   ├── device-summary.json       # 设备汇总
│   └── energy-overview.json      # 能耗总览
├── device/               # 设备类
│   ├── device-detail.json        # 设备详情
│   ├── device-comparison.json    # 设备对比
│   └── device-health.json        # 设备健康度
├── analysis/             # 分析类
│   ├── trend-analysis.json       # 趋势分析
│   ├── anomaly-detection.json    # 异常检测
│   └── correlation-analysis.json # 相关性分析
└── reports/              # 报表类
    ├── daily-report.json         # 日报
    ├── monthly-report.json       # 月报
    └── custom-report.json        # 自定义报表
```

## 3. 标准仪表板模板

### 3.1 设备监控仪表板
```json
{
  "panels": [
    {
      "title": "实时状态",
      "type": "stat",
      "targets": [{"target": "$device.status"}]
    },
    {
      "title": "关键指标趋势",
      "type": "timeseries",
      "targets": [
        {"target": "$device.voltage"},
        {"target": "$device.current"},
        {"target": "$device.power"}
      ]
    },
    {
      "title": "告警历史",
      "type": "table",
      "targets": [{"target": "$device.alerts"}]
    }
  ]
}
```

### 3.2 能耗分析仪表板
```json
{
  "panels": [
    {
      "title": "能耗趋势",
      "type": "timeseries",
      "targets": [{"target": "sum($device.energy)"}]
    },
    {
      "title": "能耗分布",
      "type": "piechart",
      "targets": [{"target": "$device.energy by (device)"}]
    },
    {
      "title": "能效指标",
      "type": "gauge",
      "targets": [{"target": "$device.efficiency"}]
    }
  ]
}
```

## 4. 权限和访问控制

### 4.1 用户角色映射
```typescript
// 前端 -> Grafana 角色映射
const roleMapping = {
  'admin': 'Admin',        // 可以创建/编辑/删除
  'engineer': 'Editor',    // 可以创建/编辑
  'operator': 'Viewer',    // 只能查看
  'guest': 'Viewer'        // 只能查看公开仪表板
};
```

### 4.2 仪表板权限
```yaml
# 基于文件夹的权限控制
folders:
  - name: "运维仪表板"
    permissions:
      - role: Editor
        permission: Edit
      - role: Viewer
        permission: View
        
  - name: "管理仪表板"
    permissions:
      - role: Admin
        permission: Admin
```

## 5. 最佳实践

### 5.1 命名规范
- **仪表板 ID**: `{category}-{function}-{version}`
  - 例如：`device-monitoring-v1`
- **面板标题**：使用中文，简洁明了
- **变量名**：使用英文，遵循 camelCase

### 5.2 性能优化
```yaml
# 仪表板设置建议
refresh: "30s"          # 默认刷新间隔
time_range: "6h"        # 默认时间范围
max_data_points: 500    # 限制数据点数量

# 查询优化
- 使用变量减少重复查询
- 合理设置聚合间隔
- 避免 SELECT * 查询
```

### 5.3 用户体验
1. **预设时间范围快捷选项**
   - 最近 15 分钟（实时监控）
   - 最近 1 小时（问题排查）
   - 最近 24 小时（日常巡检）
   - 最近 7 天（趋势分析）

2. **合理的默认值**
   - 默认选中最重要的设备
   - 默认显示最关注的指标
   - 默认使用最常用的聚合方式

3. **清晰的可视化**
   - 使用一致的颜色方案
   - 添加阈值线和告警区域
   - 提供详细的图例说明

## 6. 集成要点

### 6.1 前端集成
```typescript
// 仪表板管理服务
class DashboardService {
  // 获取用户可访问的仪表板列表
  async getUserDashboards(): Promise<Dashboard[]> {
    const role = await this.authService.getUserRole();
    return this.grafanaApi.getDashboardsByRole(role);
  }
  
  // 创建个人仪表板
  async createPersonalDashboard(config: DashboardConfig) {
    const folder = `user-${this.userId}`;
    return this.grafanaApi.createDashboard(config, folder);
  }
  
  // 克隆模板
  async cloneFromTemplate(templateId: string, customization: any) {
    const template = await this.grafanaApi.getDashboard(templateId);
    const personalized = this.applyCustomization(template, customization);
    return this.createPersonalDashboard(personalized);
  }
}
```

### 6.2 数据源配置
```yaml
# 为不同类型的查询配置不同的数据源
datasources:
  - name: "Hissrv-Realtime"
    type: "simplejson"
    url: "http://hissrv:8080/grafana/realtime"
    isDefault: true
    
  - name: "Hissrv-History"  
    type: "simplejson"
    url: "http://hissrv:8080/grafana/history"
    jsonData:
      httpMethod: "POST"
      
  - name: "Hissrv-Aggregated"
    type: "simplejson"
    url: "http://hissrv:8080/grafana/aggregated"
```

## 7. 迁移路径

### Phase 1: 评估和准备（1-2 周）
- [ ] 安装和配置 Grafana
- [ ] 实现 Hissrv 数据源适配器
- [ ] 创建第一个示例仪表板
- [ ] 测试嵌入集成

### Phase 2: 试点应用（2-4 周）
- [ ] 选择 1-2 个核心功能迁移到 Grafana
- [ ] 收集用户反馈
- [ ] 优化集成方案
- [ ] 培训关键用户

### Phase 3: 全面推广（1-2 月）
- [ ] 迁移所有历史数据可视化到 Grafana
- [ ] 创建标准仪表板库
- [ ] 实现自助式仪表板创建
- [ ] 下线原有图表组件

### Phase 4: 深度集成（持续）
- [ ] 集成告警管理
- [ ] 实现报表自动化
- [ ] 开发自定义插件
- [ ] 优化性能和体验

## 8. 常见问题

### Q: 用户需要学习 Grafana 吗？
A: 普通用户只需要查看，不需要学习。只有需要创建自定义仪表板的高级用户才需要了解 Grafana 的基本操作。

### Q: 能否限制用户的操作？
A: 可以通过权限控制限制用户只能查看特定仪表板，隐藏编辑按钮，禁用某些功能。

### Q: 如何保持 UI 一致性？
A: 通过自定义 CSS 主题、隐藏 Grafana 原生 UI 元素、使用 iframe 嵌入等方式保持一致性。

### Q: 性能是否会受影响？
A: Grafana 本身经过高度优化，通过合理的缓存策略和查询优化，性能通常优于自研方案。

## 9. 结论

直接使用 Grafana 是最高效的选择：
1. **节省开发成本**：避免重复造轮子
2. **功能完整**：获得企业级的可视化能力
3. **生态丰富**：可以使用大量现成的插件和模板
4. **持续更新**：跟随 Grafana 社区获得新功能

建议采用**渐进式集成**策略，先在历史数据分析等复杂场景使用 Grafana，逐步扩展到其他功能模块。