#!/usr/bin/env bun

import { spawn } from "bun";
import { watch } from "fs";

// 颜色输出
const colors = {
  reset: "\x1b[0m",
  red: "\x1b[31m",
  green: "\x1b[32m",
  yellow: "\x1b[33m",
  blue: "\x1b[34m",
  magenta: "\x1b[35m",
  cyan: "\x1b[36m",
};

function log(message: string, color: keyof typeof colors = "reset") {
  console.log(`${colors[color]}${message}${colors.reset}`);
}

// 并行启动多个开发服务
async function startDev() {
  log("🚀 Starting VoltageEMS Config UI development server...", "cyan");

  const processes = [];

  // 检查 Redis 连接
  try {
    const checkRedis = spawn(["redis-cli", "ping"], {
      stdout: "pipe",
      stderr: "pipe",
    });
    
    const output = await new Response(checkRedis.stdout).text();
    if (output.trim() === "PONG") {
      log("✅ Redis is running", "green");
    } else {
      throw new Error("Redis not responding");
    }
  } catch (error) {
    log("⚠️ Warning: Redis is not running. Some features may not work.", "yellow");
    log("Start Redis with: docker run -d -p 6379:6379 redis:alpine", "yellow");
  }

  // 启动 Tauri 开发服务器
  const tauriProcess = spawn(["bunx", "tauri", "dev"], {
    stdio: ["inherit", "inherit", "inherit"],
    env: {
      ...process.env,
      RUST_LOG: "debug",
      TAURI_DEV: "true",
    },
  });

  processes.push(tauriProcess);

  // 可选：启动后端 API 模拟服务器
  if (process.env.MOCK_API === "true") {
    log("🔧 Starting mock API server...", "blue");
    const mockProcess = spawn(["bun", "run", "scripts/mock-server.ts"], {
      stdio: ["inherit", "inherit", "inherit"],
    });
    processes.push(mockProcess);
  }

  // 监听文件变化（可选）
  if (process.env.WATCH_CONFIG === "true") {
    log("👁️ Watching configuration files...", "magenta");
    watch("../../services", { recursive: true }, (event, filename) => {
      if (filename?.endsWith(".yml") || filename?.endsWith(".yaml")) {
        log(`📝 Config file changed: ${filename}`, "yellow");
      }
    });
  }

  // 处理退出信号
  process.on("SIGINT", () => {
    log("\n🛑 Shutting down development server...", "red");
    processes.forEach(p => p.kill());
    process.exit(0);
  });

  // 等待所有进程
  await Promise.all(processes.map(p => p.exited));
}

// 显示开发服务器信息
function showDevInfo() {
  console.log("\n" + "=".repeat(50));
  log("VoltageEMS Config UI Development Server", "cyan");
  console.log("=".repeat(50));
  console.log("\nEnvironment Variables:");
  console.log(`  RUST_LOG: ${process.env.RUST_LOG || "info"}`);
  console.log(`  MOCK_API: ${process.env.MOCK_API || "false"}`);
  console.log(`  WATCH_CONFIG: ${process.env.WATCH_CONFIG || "false"}`);
  console.log("\nUseful Commands:");
  console.log("  • Press Ctrl+C to stop");
  console.log("  • Run 'bun test' in another terminal to run tests");
  console.log("  • Run 'bun run scripts/build.ts' to build for production");
  console.log("\n" + "=".repeat(50) + "\n");
}

// 主函数
if (import.meta.main) {
  showDevInfo();
  await startDev();
}