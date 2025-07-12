#!/usr/bin/env bun

import { $ } from "bun";
import { existsSync, rmSync } from "fs";
import { join } from "path";

async function build() {
  console.log("🏗️ Building VoltageEMS Config UI...\n");

  // 清理之前的构建
  console.log("🧹 Cleaning previous builds...");
  const distPaths = [
    "dist",
    "src-tauri/target/release/bundle",
  ];

  for (const path of distPaths) {
    if (existsSync(path)) {
      rmSync(path, { recursive: true, force: true });
      console.log(`  ✅ Removed ${path}`);
    }
  }

  // 运行测试（可选）
  if (process.env.SKIP_TESTS !== "true") {
    console.log("\n🧪 Running tests...");
    try {
      await $`bun test`;
      console.log("✅ All tests passed");
    } catch (error) {
      console.error("❌ Tests failed. Build aborted.");
      process.exit(1);
    }
  }

  // 类型检查
  console.log("\n🔍 Type checking...");
  try {
    await $`bunx tsc --noEmit`;
    console.log("✅ Type check passed");
  } catch (error) {
    console.error("❌ Type check failed. Build aborted.");
    process.exit(1);
  }

  // 构建应用
  console.log("\n🚀 Building application...");
  const startTime = Date.now();

  try {
    await $`bunx tauri build`;
    const buildTime = ((Date.now() - startTime) / 1000).toFixed(2);
    console.log(`\n✅ Build completed in ${buildTime}s`);
  } catch (error) {
    console.error("❌ Build failed:", error);
    process.exit(1);
  }

  // 显示构建产物
  console.log("\n📦 Build artifacts:");
  const platforms = {
    darwin: {
      path: "src-tauri/target/release/bundle/dmg",
      ext: ".dmg",
      name: "macOS",
    },
    win32: {
      path: "src-tauri/target/release/bundle/msi",
      ext: ".msi",
      name: "Windows",
    },
    linux: {
      path: "src-tauri/target/release/bundle/appimage",
      ext: ".AppImage",
      name: "Linux",
    },
  };

  const platform = platforms[process.platform as keyof typeof platforms];
  if (platform && existsSync(platform.path)) {
    const files = await $`ls -la ${platform.path}`.text();
    console.log(`\n${platform.name} installer:`);
    console.log(files);
  }

  // 生成版本信息
  await generateVersionInfo();

  console.log("\n🎉 Build completed successfully!");
  console.log("\nNext steps:");
  console.log("1. Test the application");
  console.log("2. Create release notes");
  console.log("3. Upload to release server");
}

async function generateVersionInfo() {
  const packageJson = await Bun.file("package.json").json();
  const buildInfo = {
    version: packageJson.version,
    buildTime: new Date().toISOString(),
    commit: await getGitCommit(),
    branch: await getGitBranch(),
  };

  await Bun.write(
    "dist/build-info.json",
    JSON.stringify(buildInfo, null, 2)
  );

  console.log("\n📝 Build info:");
  console.log(`  Version: ${buildInfo.version}`);
  console.log(`  Commit: ${buildInfo.commit}`);
  console.log(`  Branch: ${buildInfo.branch}`);
  console.log(`  Time: ${buildInfo.buildTime}`);
}

async function getGitCommit(): Promise<string> {
  try {
    const result = await $`git rev-parse --short HEAD`.text();
    return result.trim();
  } catch {
    return "unknown";
  }
}

async function getGitBranch(): Promise<string> {
  try {
    const result = await $`git rev-parse --abbrev-ref HEAD`.text();
    return result.trim();
  } catch {
    return "unknown";
  }
}

// 运行构建
if (import.meta.main) {
  await build();
}