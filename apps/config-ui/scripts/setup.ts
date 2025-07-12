#!/usr/bin/env bun

import { $ } from "bun";
import { existsSync } from "fs";
import { join } from "path";

// 使用 Bun Shell 进行项目设置
async function setup() {
  console.log("🚀 Setting up VoltageEMS Config UI...");

  // 检查 Rust 环境
  try {
    await $`rustc --version`;
    console.log("✅ Rust is installed");
  } catch {
    console.error("❌ Rust not found. Please install Rust first.");
    console.log("Visit: https://www.rust-lang.org/tools/install");
    process.exit(1);
  }

  // 检查 Tauri CLI
  try {
    await $`cargo tauri --version`;
    console.log("✅ Tauri CLI is installed");
  } catch {
    console.log("⚙️ Installing Tauri CLI...");
    await $`cargo install tauri-cli`;
  }

  // 创建必要的目录
  const dirs = [
    "src/stores",
    "src/composables",
    "src/types",
    "src/utils",
    "logs",
  ];

  for (const dir of dirs) {
    if (!existsSync(dir)) {
      await $`mkdir -p ${dir}`;
      console.log(`📁 Created directory: ${dir}`);
    }
  }

  // 生成类型定义
  await generateTypes();

  // 初始化 Git hooks (如果需要)
  if (existsSync(".git")) {
    console.log("📝 Setting up Git hooks...");
    // 可以在这里添加 Git hooks 设置
  }

  console.log("✅ Setup complete!");
  console.log("\nNext steps:");
  console.log("1. Run 'bun dev' to start development server");
  console.log("2. Run 'bun test' to run tests");
  console.log("3. Run 'bun build' to build for production");
}

async function generateTypes() {
  console.log("📝 Generating TypeScript types...");

  // 生成自动导入类型
  const autoImportsContent = `// Auto-generated file
export {}
declare global {
  // Vue imports
  const ref: typeof import('vue')['ref']
  const computed: typeof import('vue')['computed']
  const reactive: typeof import('vue')['reactive']
  const onMounted: typeof import('vue')['onMounted']
  const watch: typeof import('vue')['watch']
  
  // Vue Router
  const useRouter: typeof import('vue-router')['useRouter']
  const useRoute: typeof import('vue-router')['useRoute']
  
  // Pinia
  const defineStore: typeof import('pinia')['defineStore']
  
  // VueUse
  const useLocalStorage: typeof import('@vueuse/core')['useLocalStorage']
  const useDark: typeof import('@vueuse/core')['useDark']
}
`;

  await Bun.write("src/types/auto-imports.d.ts", autoImportsContent);

  // 生成组件类型
  const componentsContent = `// Auto-generated file
export {}
declare module 'vue' {
  export interface GlobalComponents {
    ElButton: typeof import('element-plus')['ElButton']
    ElCard: typeof import('element-plus')['ElCard']
    ElTable: typeof import('element-plus')['ElTable']
    // Add more components as needed
  }
}
`;

  await Bun.write("src/types/components.d.ts", componentsContent);

  console.log("✅ Types generated successfully");
}

// 运行设置
if (import.meta.main) {
  await setup();
}