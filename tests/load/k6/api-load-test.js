/**
 * K6 API Load Test for VoltageEMS
 * Tests HTTP API endpoints under load
 */

import http from "k6/http";
import { check, sleep } from "k6";
import { Rate, Trend, Counter } from "k6/metrics";

// Custom metrics
const errorRate = new Rate("errors");
const responseTime = new Trend("response_time");
const requestCount = new Counter("requests");

// Test configuration
export const options = {
  stages: [
    { duration: "2m", target: 20 }, // Ramp up to 20 users
    { duration: "5m", target: 50 }, // Stay at 50 users
    { duration: "2m", target: 100 }, // Ramp up to 100 users
    { duration: "5m", target: 100 }, // Stay at 100 users
    { duration: "2m", target: 0 }, // Ramp down
  ],
  thresholds: {
    http_req_duration: ["p(95)<500"], // 95% of requests should be below 500ms
    http_req_failed: ["rate<0.1"], // Error rate should be less than 10%
    errors: ["rate<0.05"], // Custom error rate should be less than 5%
  },
};

const BASE_URL = __ENV.TARGET_URL || "http://localhost:8080";

// Test data
const testChannels = [1001, 1002, 1003];
const testModels = ["model_001", "model_002"];

export function setup() {
  console.log(`Starting API load test against: ${BASE_URL}`);

  // Verify services are available
  const healthCheck = http.get(`${BASE_URL}/health`);
  if (healthCheck.status !== 200) {
    throw new Error("Health check failed - services not ready");
  }

  return { baseUrl: BASE_URL };
}

export default function (data) {
  const baseUrl = data.baseUrl;

  // Test scenario weights
  const scenario = Math.random();

  if (scenario < 0.3) {
    // 30% - Read telemetry data
    testTelemetryRead(baseUrl);
  } else if (scenario < 0.5) {
    // 20% - Model operations
    testModelOperations(baseUrl);
  } else if (scenario < 0.7) {
    // 20% - Alarm operations
    testAlarmOperations(baseUrl);
  } else if (scenario < 0.85) {
    // 15% - Rule operations
    testRuleOperations(baseUrl);
  } else {
    // 15% - Historical data queries
    testHistoricalQueries(baseUrl);
  }

  sleep(1); // 1 second between requests
}

function testTelemetryRead(baseUrl) {
  const channelId =
    testChannels[Math.floor(Math.random() * testChannels.length)];
  const pointType = ["T", "S", "C", "A"][Math.floor(Math.random() * 4)];

  const startTime = Date.now();
  const response = http.get(
    `${baseUrl}/comsrv/channels/${channelId}/${pointType}`,
  );
  const duration = Date.now() - startTime;

  requestCount.add(1);
  responseTime.add(duration);

  const success = check(response, {
    "telemetry read status is 200": (r) => r.status === 200,
    "telemetry read response time < 200ms": () => duration < 200,
    "telemetry read has valid data": (r) => {
      try {
        const data = JSON.parse(r.body);
        return typeof data === "object" && data !== null;
      } catch {
        return false;
      }
    },
  });

  if (!success) {
    errorRate.add(1);
  }
}

function testModelOperations(baseUrl) {
  const operations = [
    () => http.get(`${baseUrl}/modsrv/models`),
    () => {
      const modelId = testModels[Math.floor(Math.random() * testModels.length)];
      return http.get(`${baseUrl}/modsrv/models/${modelId}`);
    },
    () =>
      http.post(
        `${baseUrl}/modsrv/models`,
        JSON.stringify({
          id: `test_model_${Date.now()}`,
          name: "Load Test Model",
          description: "Model created during load test",
          points: [1, 2, 3],
        }),
        { headers: { "Content-Type": "application/json" } },
      ),
  ];

  const operation = operations[Math.floor(Math.random() * operations.length)];
  const startTime = Date.now();
  const response = operation();
  const duration = Date.now() - startTime;

  requestCount.add(1);
  responseTime.add(duration);

  const success = check(response, {
    "model operation status is 2xx": (r) => r.status >= 200 && r.status < 300,
    "model operation response time < 300ms": () => duration < 300,
  });

  if (!success) {
    errorRate.add(1);
  }
}

function testAlarmOperations(baseUrl) {
  const operations = [
    () => http.get(`${baseUrl}/alarmsrv/alarms`),
    () =>
      http.post(
        `${baseUrl}/alarmsrv/alarms`,
        JSON.stringify({
          title: `Load Test Alarm ${Date.now()}`,
          description: "Alarm created during load test",
          level: "Warning",
          conditions: [{ point_id: 1, operator: "greater_than", value: 100.0 }],
        }),
        { headers: { "Content-Type": "application/json" } },
      ),
  ];

  const operation = operations[Math.floor(Math.random() * operations.length)];
  const startTime = Date.now();
  const response = operation();
  const duration = Date.now() - startTime;

  requestCount.add(1);
  responseTime.add(duration);

  const success = check(response, {
    "alarm operation status is 2xx": (r) => r.status >= 200 && r.status < 300,
    "alarm operation response time < 400ms": () => duration < 400,
  });

  if (!success) {
    errorRate.add(1);
  }
}

function testRuleOperations(baseUrl) {
  const operations = [
    () => http.get(`${baseUrl}/rulesrv/rules`),
    () =>
      http.post(
        `${baseUrl}/rulesrv/rules`,
        JSON.stringify({
          name: `Load Test Rule ${Date.now()}`,
          description: "Rule created during load test",
          conditions: [{ point_id: 1, operator: "greater_than", value: 50.0 }],
          actions: [{ type: "log", message: "Load test rule triggered" }],
        }),
        { headers: { "Content-Type": "application/json" } },
      ),
  ];

  const operation = operations[Math.floor(Math.random() * operations.length)];
  const startTime = Date.now();
  const response = operation();
  const duration = Date.now() - startTime;

  requestCount.add(1);
  responseTime.add(duration);

  const success = check(response, {
    "rule operation status is 2xx": (r) => r.status >= 200 && r.status < 300,
    "rule operation response time < 350ms": () => duration < 350,
  });

  if (!success) {
    errorRate.add(1);
  }
}

function testHistoricalQueries(baseUrl) {
  const queries = [
    `${baseUrl}/hissrv/history/latest?limit=100`,
    `${baseUrl}/hissrv/history/range?start=-1h&end=now`,
    `${baseUrl}/hissrv/history/aggregated?window=5m&function=mean`,
  ];

  const query = queries[Math.floor(Math.random() * queries.length)];
  const startTime = Date.now();
  const response = http.get(query);
  const duration = Date.now() - startTime;

  requestCount.add(1);
  responseTime.add(duration);

  const success = check(response, {
    "history query status is 2xx": (r) => r.status >= 200 && r.status < 300,
    "history query response time < 1000ms": () => duration < 1000, // Longer timeout for complex queries
  });

  if (!success) {
    errorRate.add(1);
  }
}

export function teardown(data) {
  console.log("API load test completed");

  // Cleanup test data if needed
  // Note: In a real scenario, you might want to clean up test models/alarms created during the test
}
