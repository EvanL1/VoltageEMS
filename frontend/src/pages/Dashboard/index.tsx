import React, { useState, useEffect } from 'react';
import { Row, Col, Card, Statistic, Space, Button, Modal, Select, message } from 'antd';
import { 
  DashboardOutlined, 
  AreaChartOutlined, 
  AlertOutlined,
  SettingOutlined,
  PlusOutlined 
} from '@ant-design/icons';
import { GrafanaEmbed } from '@/components/GrafanaIntegration';
import { grafanaService } from '@/services/GrafanaService';
import styles from './index.module.css';

const { Option } = Select;

interface DashboardWidget {
  id: string;
  type: 'statistic' | 'grafana' | 'custom';
  title: string;
  span: number;
  config: any;
}

const Dashboard: React.FC = () => {
  const [widgets, setWidgets] = useState<DashboardWidget[]>([
    {
      id: 'overview-stats',
      type: 'statistic',
      title: '系统概览',
      span: 24,
      config: {
        stats: [
          { title: '在线设备', value: 156, suffix: '台', status: 'success' },
          { title: '今日能耗', value: 1234.56, suffix: 'kWh', precision: 2 },
          { title: '活跃告警', value: 3, status: 'warning' },
          { title: '系统效率', value: 98.5, suffix: '%', precision: 1, status: 'success' }
        ]
      }
    },
    {
      id: 'realtime-monitor',
      type: 'grafana',
      title: '实时监控',
      span: 12,
      config: {
        dashboardUid: 'realtime-overview',
        height: '400px',
        refresh: '5s'
      }
    },
    {
      id: 'energy-trend',
      type: 'grafana',
      title: '能耗趋势',
      span: 12,
      config: {
        dashboardUid: 'energy-consumption',
        height: '400px',
        timeRange: { from: 'now-24h', to: 'now' }
      }
    }
  ]);

  const [isAddModalVisible, setIsAddModalVisible] = useState(false);
  const [availableDashboards, setAvailableDashboards] = useState<any[]>([]);
  const [selectedDashboard, setSelectedDashboard] = useState<string>('');

  // 加载可用的 Grafana 仪表板
  useEffect(() => {
    const loadDashboards = async () => {
      try {
        const dashboards = await grafanaService.getDashboards();
        setAvailableDashboards(dashboards);
      } catch (error) {
        console.error('Failed to load dashboards:', error);
      }
    };

    loadDashboards();
  }, []);

  // 添加新的小部件
  const handleAddWidget = () => {
    if (!selectedDashboard) {
      message.warning('请选择一个仪表板');
      return;
    }

    const dashboard = availableDashboards.find(d => d.uid === selectedDashboard);
    if (!dashboard) return;

    const newWidget: DashboardWidget = {
      id: `widget-${Date.now()}`,
      type: 'grafana',
      title: dashboard.title,
      span: 12,
      config: {
        dashboardUid: dashboard.uid,
        height: '400px'
      }
    };

    setWidgets([...widgets, newWidget]);
    setIsAddModalVisible(false);
    setSelectedDashboard('');
    message.success('添加成功');
  };

  // 移除小部件
  const handleRemoveWidget = (widgetId: string) => {
    setWidgets(widgets.filter(w => w.id !== widgetId));
  };

  // 渲染统计卡片
  const renderStatisticWidget = (widget: DashboardWidget) => {
    const { stats } = widget.config;
    return (
      <Card title={widget.title} className={styles.widget}>
        <Row gutter={16}>
          {stats.map((stat: any, index: number) => (
            <Col span={6} key={index}>
              <Card className={`${styles.statCard} ${styles[stat.status]}`}>
                <Statistic
                  title={stat.title}
                  value={stat.value}
                  precision={stat.precision}
                  suffix={stat.suffix}
                />
              </Card>
            </Col>
          ))}
        </Row>
      </Card>
    );
  };

  // 渲染 Grafana 小部件
  const renderGrafanaWidget = (widget: DashboardWidget) => {
    return (
      <Card 
        title={widget.title} 
        className={styles.widget}
        extra={
          <Button 
            type="text" 
            danger 
            size="small"
            onClick={() => handleRemoveWidget(widget.id)}
          >
            移除
          </Button>
        }
      >
        <GrafanaEmbed {...widget.config} />
      </Card>
    );
  };

  // 渲染小部件
  const renderWidget = (widget: DashboardWidget) => {
    switch (widget.type) {
      case 'statistic':
        return renderStatisticWidget(widget);
      case 'grafana':
        return renderGrafanaWidget(widget);
      default:
        return null;
    }
  };

  return (
    <div className={styles.dashboard}>
      <div className={styles.header}>
        <h2>监控中心</h2>
        <Space>
          <Button icon={<SettingOutlined />}>设置</Button>
          <Button 
            type="primary" 
            icon={<PlusOutlined />}
            onClick={() => setIsAddModalVisible(true)}
          >
            添加仪表板
          </Button>
        </Space>
      </div>

      <Row gutter={[16, 16]}>
        {widgets.map(widget => (
          <Col key={widget.id} span={widget.span}>
            {renderWidget(widget)}
          </Col>
        ))}
      </Row>

      <Modal
        title="添加仪表板"
        visible={isAddModalVisible}
        onOk={handleAddWidget}
        onCancel={() => {
          setIsAddModalVisible(false);
          setSelectedDashboard('');
        }}
        width={600}
      >
        <div className={styles.addModal}>
          <p>选择要添加的 Grafana 仪表板：</p>
          <Select
            value={selectedDashboard}
            onChange={setSelectedDashboard}
            style={{ width: '100%' }}
            placeholder="请选择仪表板"
            showSearch
            filterOption={(input, option) =>
              option?.children.toLowerCase().includes(input.toLowerCase())
            }
          >
            {availableDashboards.map(dashboard => (
              <Option key={dashboard.uid} value={dashboard.uid}>
                {dashboard.title}
                {dashboard.folderTitle && ` (${dashboard.folderTitle})`}
              </Option>
            ))}
          </Select>
          
          {selectedDashboard && (
            <div className={styles.preview}>
              <h4>预览：</h4>
              <GrafanaEmbed
                dashboardUid={selectedDashboard}
                height="300px"
                theme="light"
              />
            </div>
          )}
        </div>
      </Modal>
    </div>
  );
};

export default Dashboard;