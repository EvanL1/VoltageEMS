import React, { useEffect, useState } from 'react';
import { message } from 'antd';
import { useAuth } from '@/hooks/useAuth';
import styles from './index.module.css';

interface GrafanaEmbedProps {
  dashboardUid: string;
  height?: string | number;
  timeRange?: {
    from: string;
    to: string;
  };
  variables?: Record<string, string>;
  theme?: 'light' | 'dark';
  refresh?: string;
}

/**
 * Grafana 嵌入组件
 * 用于在前端页面中嵌入 Grafana 仪表板
 */
export const GrafanaEmbed: React.FC<GrafanaEmbedProps> = ({
  dashboardUid,
  height = '600px',
  timeRange,
  variables = {},
  theme = 'light',
  refresh = '10s'
}) => {
  const { ensureGrafanaAuth } = useAuth();
  const [isReady, setIsReady] = useState(false);
  const [iframeKey, setIframeKey] = useState(0);

  useEffect(() => {
    const initGrafana = async () => {
      try {
        // 确保 Grafana 认证
        await ensureGrafanaAuth();
        setIsReady(true);
      } catch (error) {
        message.error('Grafana 认证失败，请刷新页面重试');
        console.error('Grafana auth error:', error);
      }
    };

    initGrafana();
  }, []);

  // 构建 Grafana URL
  const buildGrafanaUrl = () => {
    const params = new URLSearchParams({
      orgId: '1',
      theme,
      refresh,
      kiosk: 'tv', // tv 模式隐藏所有 UI，只显示面板
    });

    // 添加时间范围
    if (timeRange) {
      params.append('from', timeRange.from);
      params.append('to', timeRange.to);
    }

    // 添加变量
    Object.entries(variables).forEach(([key, value]) => {
      params.append(`var-${key}`, value);
    });

    return `/grafana/d/${dashboardUid}?${params.toString()}`;
  };

  // 处理 iframe 消息
  useEffect(() => {
    const handleMessage = (event: MessageEvent) => {
      if (event.origin !== window.location.origin) return;

      // 处理来自 Grafana 的消息
      if (event.data.type === 'grafana-dashboard-loaded') {
        console.log('Grafana dashboard loaded');
      }
    };

    window.addEventListener('message', handleMessage);
    return () => window.removeEventListener('message', handleMessage);
  }, []);

  // 刷新 iframe
  const refreshIframe = () => {
    setIframeKey(prev => prev + 1);
  };

  if (!isReady) {
    return (
      <div className={styles.loading}>
        <div className={styles.spinner} />
        <p>正在加载 Grafana...</p>
      </div>
    );
  }

  return (
    <div className={styles.grafanaContainer}>
      <iframe
        key={iframeKey}
        src={buildGrafanaUrl()}
        className={styles.grafanaIframe}
        style={{ height }}
        title={`Grafana Dashboard - ${dashboardUid}`}
        frameBorder="0"
        allowFullScreen
      />
    </div>
  );
};

/**
 * Grafana 仪表板包装组件
 * 提供更多控制功能
 */
export const GrafanaDashboard: React.FC<GrafanaEmbedProps & {
  title?: string;
  onRefresh?: () => void;
}> = ({ title, onRefresh, ...embedProps }) => {
  return (
    <div className={styles.dashboardWrapper}>
      {title && (
        <div className={styles.dashboardHeader}>
          <h3>{title}</h3>
          {onRefresh && (
            <button onClick={onRefresh} className={styles.refreshBtn}>
              刷新
            </button>
          )}
        </div>
      )}
      <GrafanaEmbed {...embedProps} />
    </div>
  );
};