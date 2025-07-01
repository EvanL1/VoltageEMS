import React, { useState, useEffect } from 'react';
import { Card, Tabs, Select, DatePicker, Space, Button, Spin, Empty } from 'antd';
import { useSearchParams } from 'react-router-dom';
import dayjs from 'dayjs';
import { GrafanaEmbed, GrafanaDashboard } from '@/components/GrafanaIntegration';
import { grafanaService } from '@/services/GrafanaService';
import styles from './HistoryAnalysis.module.css';

const { TabPane } = Tabs;
const { RangePicker } = DatePicker;
const { Option } = Select;

interface DashboardConfig {
  uid: string;
  title: string;
  description?: string;
  tags?: string[];
  defaultTimeRange?: {
    from: string;
    to: string;
  };
}

// 预定义的仪表板配置
const PREDEFINED_DASHBOARDS: DashboardConfig[] = [
  {
    uid: 'voltage-ems-overview',
    title: '系统总览',
    description: '系统整体运行状态和关键指标',
    tags: ['overview', 'system'],
    defaultTimeRange: { from: 'now-6h', to: 'now' }
  },
  {
    uid: 'device-analysis',
    title: '设备分析',
    description: '设备运行数据详细分析',
    tags: ['device', 'analysis'],
    defaultTimeRange: { from: 'now-24h', to: 'now' }
  },
  {
    uid: 'energy-consumption',
    title: '能耗分析',
    description: '能源消耗趋势和效率分析',
    tags: ['energy', 'consumption'],
    defaultTimeRange: { from: 'now-7d', to: 'now' }
  },
  {
    uid: 'alarm-history',
    title: '告警历史',
    description: '历史告警记录和统计分析',
    tags: ['alarm', 'history'],
    defaultTimeRange: { from: 'now-30d', to: 'now' }
  }
];

const HistoryAnalysis: React.FC = () => {
  const [searchParams, setSearchParams] = useSearchParams();
  const [activeTab, setActiveTab] = useState(searchParams.get('tab') || 'overview');
  const [selectedDevice, setSelectedDevice] = useState<string>('all');
  const [timeRange, setTimeRange] = useState<[dayjs.Dayjs, dayjs.Dayjs]>([
    dayjs().subtract(24, 'hour'),
    dayjs()
  ]);
  const [customDashboards, setCustomDashboards] = useState<DashboardConfig[]>([]);
  const [loading, setLoading] = useState(true);

  // 加载用户自定义仪表板
  useEffect(() => {
    const loadCustomDashboards = async () => {
      try {
        const dashboards = await grafanaService.getDashboards();
        const customDashboards = dashboards
          .filter(d => d.tags?.includes('custom'))
          .map(d => ({
            uid: d.uid,
            title: d.title,
            tags: d.tags
          }));
        setCustomDashboards(customDashboards);
      } catch (error) {
        console.error('Failed to load custom dashboards:', error);
      } finally {
        setLoading(false);
      }
    };

    loadCustomDashboards();
  }, []);

  // 处理标签页切换
  const handleTabChange = (key: string) => {
    setActiveTab(key);
    setSearchParams({ tab: key });
  };

  // 处理时间范围变化
  const handleTimeRangeChange = (dates: any) => {
    if (dates) {
      setTimeRange(dates);
    }
  };

  // 获取当前激活的仪表板配置
  const getActiveDashboard = (): DashboardConfig | undefined => {
    return [...PREDEFINED_DASHBOARDS, ...customDashboards].find(
      d => d.uid === activeTab
    );
  };

  // 创建新的自定义仪表板
  const handleCreateDashboard = async () => {
    try {
      // 这里可以打开一个模态框让用户选择模板或从头创建
      window.open('/grafana/dashboard/new', '_blank');
    } catch (error) {
      console.error('Failed to create dashboard:', error);
    }
  };

  // 导出当前仪表板
  const handleExportDashboard = async () => {
    try {
      const dashboard = getActiveDashboard();
      if (!dashboard) return;

      const snapshot = await grafanaService.createSnapshot(
        dashboard.uid,
        `${dashboard.title}_${dayjs().format('YYYY-MM-DD_HH-mm')}`
      );
      
      window.open(snapshot.url, '_blank');
    } catch (error) {
      console.error('Failed to export dashboard:', error);
    }
  };

  const activeDashboard = getActiveDashboard();

  return (
    <div className={styles.historyAnalysis}>
      <Card
        title="历史数据分析"
        extra={
          <Space>
            <Button onClick={handleCreateDashboard}>创建仪表板</Button>
            <Button onClick={handleExportDashboard}>导出</Button>
          </Space>
        }
      >
        <div className={styles.toolbar}>
          <Space size="large">
            <Select
              value={selectedDevice}
              onChange={setSelectedDevice}
              style={{ width: 200 }}
              placeholder="选择设备"
            >
              <Option value="all">所有设备</Option>
              <Option value="device_001">变压器 #1</Option>
              <Option value="device_002">变压器 #2</Option>
              <Option value="device_003">配电柜 #1</Option>
            </Select>

            <RangePicker
              value={timeRange}
              onChange={handleTimeRangeChange}
              showTime
              format="YYYY-MM-DD HH:mm"
              presets={[
                { label: '最近1小时', value: [dayjs().subtract(1, 'hour'), dayjs()] },
                { label: '最近6小时', value: [dayjs().subtract(6, 'hour'), dayjs()] },
                { label: '最近24小时', value: [dayjs().subtract(24, 'hour'), dayjs()] },
                { label: '最近7天', value: [dayjs().subtract(7, 'day'), dayjs()] },
                { label: '最近30天', value: [dayjs().subtract(30, 'day'), dayjs()] },
              ]}
            />
          </Space>
        </div>

        <Tabs activeKey={activeTab} onChange={handleTabChange}>
          {PREDEFINED_DASHBOARDS.map(dashboard => (
            <TabPane tab={dashboard.title} key={dashboard.uid}>
              <div className={styles.dashboardContainer}>
                <GrafanaEmbed
                  dashboardUid={dashboard.uid}
                  height="calc(100vh - 320px)"
                  timeRange={{
                    from: timeRange[0].toISOString(),
                    to: timeRange[1].toISOString()
                  }}
                  variables={{
                    device: selectedDevice,
                  }}
                  theme="light"
                  refresh="10s"
                />
              </div>
            </TabPane>
          ))}

          {customDashboards.length > 0 && (
            <TabPane tab="自定义仪表板" key="custom">
              <div className={styles.customDashboards}>
                {customDashboards.map(dashboard => (
                  <Card
                    key={dashboard.uid}
                    title={dashboard.title}
                    size="small"
                    className={styles.dashboardCard}
                  >
                    <GrafanaEmbed
                      dashboardUid={dashboard.uid}
                      height="400px"
                      timeRange={{
                        from: timeRange[0].toISOString(),
                        to: timeRange[1].toISOString()
                      }}
                      variables={{
                        device: selectedDevice,
                      }}
                    />
                  </Card>
                ))}
              </div>
            </TabPane>
          )}
        </Tabs>

        {loading && (
          <div className={styles.loading}>
            <Spin size="large" />
          </div>
        )}
      </Card>
    </div>
  );
};

export default HistoryAnalysis;