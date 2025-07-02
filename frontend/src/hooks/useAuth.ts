import { useState, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { message } from 'antd';
import { authService } from '@/services/AuthService';
import { grafanaService } from '@/services/GrafanaService';

interface AuthState {
  isAuthenticated: boolean;
  user: any;
  loading: boolean;
}

export const useAuth = () => {
  const navigate = useNavigate();
  const [authState, setAuthState] = useState<AuthState>({
    isAuthenticated: false,
    user: null,
    loading: true
  });

  // 检查认证状态
  useEffect(() => {
    const checkAuth = async () => {
      try {
        const token = authService.getToken();
        if (!token) {
          setAuthState({ isAuthenticated: false, user: null, loading: false });
          return;
        }

        const user = await authService.getCurrentUser();
        setAuthState({ isAuthenticated: true, user, loading: false });
      } catch (error) {
        console.error('Auth check failed:', error);
        setAuthState({ isAuthenticated: false, user: null, loading: false });
      }
    };

    checkAuth();
  }, []);

  // 登录
  const login = useCallback(async (credentials: { username: string; password: string }) => {
    try {
      const response = await authService.login(credentials);
      const { token, user } = response;

      authService.setToken(token);
      setAuthState({ isAuthenticated: true, user, loading: false });

      message.success('登录成功');
      navigate('/');
    } catch (error: any) {
      message.error(error.message || '登录失败');
      throw error;
    }
  }, [navigate]);

  // 登出
  const logout = useCallback(async () => {
    try {
      await authService.logout();
      authService.clearToken();
      setAuthState({ isAuthenticated: false, user: null, loading: false });
      navigate('/login');
    } catch (error) {
      console.error('Logout error:', error);
    }
  }, [navigate]);

  // 确保 Grafana 认证
  const ensureGrafanaAuth = useCallback(async () => {
    if (!authState.isAuthenticated) {
      throw new Error('User not authenticated');
    }

    try {
      await grafanaService.ensureAuth();
    } catch (error) {
      console.error('Grafana auth failed:', error);
      throw error;
    }
  }, [authState.isAuthenticated]);

  return {
    ...authState,
    login,
    logout,
    ensureGrafanaAuth
  };
};