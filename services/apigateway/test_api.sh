#!/bin/bash

# API Gateway 测试脚本

API_URL="http://localhost:8080/api/v1"

echo "=== API Gateway 测试 ==="
echo

# 1. 健康检查
echo "1. 健康检查:"
curl -s "$API_URL/health" | jq .
echo

# 2. 登录测试
echo "2. 登录测试 (admin):"
LOGIN_RESPONSE=$(curl -s -X POST "$API_URL/auth/login" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "admin",
    "password": "admin123"
  }')
echo "$LOGIN_RESPONSE" | jq .

# 提取 token
ACCESS_TOKEN=$(echo "$LOGIN_RESPONSE" | jq -r '.data.access_token')
REFRESH_TOKEN=$(echo "$LOGIN_RESPONSE" | jq -r '.data.refresh_token')

echo
echo "Access Token: ${ACCESS_TOKEN:0:20}..."
echo

# 3. 获取当前用户信息
echo "3. 获取当前用户信息:"
curl -s "$API_URL/auth/me" \
  -H "Authorization: Bearer $ACCESS_TOKEN" | jq .
echo

# 4. 尝试未授权访问
echo "4. 尝试未授权访问:"
curl -s "$API_URL/comsrv/channels" | jq .
echo

# 5. 授权访问
echo "5. 授权访问 (需要运行 comsrv):"
curl -s "$API_URL/comsrv/channels" \
  -H "Authorization: Bearer $ACCESS_TOKEN" | jq .
echo

# 6. 刷新 Token
echo "6. 刷新 Token:"
NEW_TOKEN_RESPONSE=$(curl -s -X POST "$API_URL/auth/refresh" \
  -H "Content-Type: application/json" \
  -d "{
    \"refresh_token\": \"$REFRESH_TOKEN\"
  }")
echo "$NEW_TOKEN_RESPONSE" | jq .
echo

# 7. 登出
echo "7. 登出:"
curl -s -X POST "$API_URL/auth/logout" \
  -H "Authorization: Bearer $ACCESS_TOKEN" | jq .
echo

# 8. 详细健康检查
echo "8. 详细健康检查:"
curl -s "$API_URL/health/detailed" | jq .
echo

# 9. 测试不同角色
echo "9. 测试 viewer 角色:"
VIEWER_RESPONSE=$(curl -s -X POST "$API_URL/auth/login" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "viewer",
    "password": "viewer123"
  }')
VIEWER_TOKEN=$(echo "$VIEWER_RESPONSE" | jq -r '.data.access_token')

echo "Viewer 用户信息:"
curl -s "$API_URL/auth/me" \
  -H "Authorization: Bearer $VIEWER_TOKEN" | jq .