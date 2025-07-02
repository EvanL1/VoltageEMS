import { request } from '@/utils/request';

export interface GrafanaDashboard {
  uid: string;
  title: string;
  tags: string[];
  folderTitle?: string;
  folderUid?: string;
  url: string;
}

export interface GrafanaOrg {
  id: number;
  name: string;
}

export interface GrafanaUser {
  id: number;
  email: string;
  name: string;
  login: string;
  orgId: number;
}

/**
 * Grafana 服务
 * 处理与 Grafana 的所有交互
 */
export class GrafanaService {
  private static instance: GrafanaService;
  private grafanaBaseUrl = '/grafana';
  private apiKey: string | null = null;

  private constructor() {}

  static getInstance(): GrafanaService {
    if (!GrafanaService.instance) {
      GrafanaService.instance = new GrafanaService();
    }
    return GrafanaService.instance;
  }

  /**
   * 确保 Grafana 认证
   * 为当前用户创建或获取 Grafana API Key
   */
  async ensureAuth(): Promise<void> {
    try {
      // 检查是否已有有效的 API Key
      const existingKey = sessionStorage.getItem('grafana_api_key');
      if (existingKey) {
        this.apiKey = existingKey;
        // 验证 key 是否仍然有效
        const isValid = await this.validateApiKey();
        if (isValid) return;
      }

      // 创建新的 API Key
      const response = await request.post('/api/grafana/auth/create-key', {
        name: `web-session-${Date.now()}`,
        role: 'Viewer',
        secondsToLive: 86400 // 24小时
      });

      this.apiKey = response.data.key;
      sessionStorage.setItem('grafana_api_key', this.apiKey);

      // 设置 Grafana session cookie
      document.cookie = `grafana_session=${this.apiKey}; path=/grafana; max-age=86400`;
    } catch (error) {
      console.error('Grafana auth failed:', error);
      throw new Error('无法建立 Grafana 连接');
    }
  }

  /**
   * 验证 API Key 是否有效
   */
  private async validateApiKey(): Promise<boolean> {
    try {
      await this.grafanaRequest('/api/org');
      return true;
    } catch {
      return false;
    }
  }

  /**
   * 发送 Grafana API 请求
   */
  private async grafanaRequest(path: string, options: RequestInit = {}): Promise<any> {
    if (!this.apiKey) {
      throw new Error('Grafana API Key not initialized');
    }

    const response = await fetch(`${this.grafanaBaseUrl}${path}`, {
      ...options,
      headers: {
        'Authorization': `Bearer ${this.apiKey}`,
        'Content-Type': 'application/json',
        ...options.headers
      }
    });

    if (!response.ok) {
      throw new Error(`Grafana API error: ${response.statusText}`);
    }

    return response.json();
  }

  /**
   * 获取所有仪表板列表
   */
  async getDashboards(): Promise<GrafanaDashboard[]> {
    const data = await this.grafanaRequest('/api/search?type=dash-db');
    return data;
  }

  /**
   * 获取特定文件夹的仪表板
   */
  async getDashboardsByFolder(folderUid: string): Promise<GrafanaDashboard[]> {
    const data = await this.grafanaRequest(`/api/search?type=dash-db&folderIds=${folderUid}`);
    return data;
  }

  /**
   * 获取仪表板详情
   */
  async getDashboard(uid: string): Promise<any> {
    const data = await this.grafanaRequest(`/api/dashboards/uid/${uid}`);
    return data.dashboard;
  }

  /**
   * 创建仪表板
   */
  async createDashboard(dashboard: any, folderUid?: string): Promise<any> {
    const payload = {
      dashboard,
      folderUid,
      overwrite: false,
      message: 'Created from VoltageEMS'
    };

    return this.grafanaRequest('/api/dashboards/db', {
      method: 'POST',
      body: JSON.stringify(payload)
    });
  }

  /**
   * 更新仪表板
   */
  async updateDashboard(uid: string, dashboard: any): Promise<any> {
    const existing = await this.getDashboard(uid);
    
    const payload = {
      dashboard: {
        ...existing,
        ...dashboard,
        version: existing.version + 1
      },
      overwrite: true,
      message: 'Updated from VoltageEMS'
    };

    return this.grafanaRequest('/api/dashboards/db', {
      method: 'POST',
      body: JSON.stringify(payload)
    });
  }

  /**
   * 删除仪表板
   */
  async deleteDashboard(uid: string): Promise<void> {
    await this.grafanaRequest(`/api/dashboards/uid/${uid}`, {
      method: 'DELETE'
    });
  }

  /**
   * 获取仪表板快照
   */
  async createSnapshot(dashboardUid: string, name: string): Promise<any> {
    const dashboard = await this.getDashboard(dashboardUid);
    
    const payload = {
      dashboard,
      name,
      expires: 3600 // 1小时后过期
    };

    return this.grafanaRequest('/api/snapshots', {
      method: 'POST',
      body: JSON.stringify(payload)
    });
  }

  /**
   * 获取当前用户信息
   */
  async getCurrentUser(): Promise<GrafanaUser> {
    return this.grafanaRequest('/api/user');
  }

  /**
   * 获取组织信息
   */
  async getCurrentOrg(): Promise<GrafanaOrg> {
    return this.grafanaRequest('/api/org');
  }

  /**
   * 构建仪表板 URL
   */
  buildDashboardUrl(uid: string, params?: {
    from?: string;
    to?: string;
    orgId?: number;
    theme?: 'light' | 'dark';
    kiosk?: boolean | 'tv';
    refresh?: string;
    variables?: Record<string, string>;
  }): string {
    const searchParams = new URLSearchParams();

    if (params) {
      if (params.orgId) searchParams.append('orgId', params.orgId.toString());
      if (params.from) searchParams.append('from', params.from);
      if (params.to) searchParams.append('to', params.to);
      if (params.theme) searchParams.append('theme', params.theme);
      if (params.refresh) searchParams.append('refresh', params.refresh);
      
      if (params.kiosk !== undefined) {
        searchParams.append('kiosk', params.kiosk === true ? '1' : params.kiosk);
      }

      // 添加变量
      if (params.variables) {
        Object.entries(params.variables).forEach(([key, value]) => {
          searchParams.append(`var-${key}`, value);
        });
      }
    }

    const queryString = searchParams.toString();
    return `${this.grafanaBaseUrl}/d/${uid}${queryString ? '?' + queryString : ''}`;
  }
}

// 导出单例
export const grafanaService = GrafanaService.getInstance();