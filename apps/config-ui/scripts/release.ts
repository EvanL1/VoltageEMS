#!/usr/bin/env bun

import { $ } from "bun";
import { readFileSync, writeFileSync } from "fs";
import { join } from "path";

interface ReleaseOptions {
  version?: string;
  skipTests?: boolean;
  skipTag?: boolean;
  dryRun?: boolean;
}

async function release(options: ReleaseOptions = {}) {
  console.log("🚀 Starting release process...\n");

  // 检查工作目录是否干净
  const gitStatus = await $`git status --porcelain`.text();
  if (gitStatus.trim() && !options.dryRun) {
    console.error("❌ Working directory is not clean. Please commit or stash changes.");
    process.exit(1);
  }

  // 获取当前版本
  const packagePath = "package.json";
  const packageJson = JSON.parse(readFileSync(packagePath, "utf-8"));
  const currentVersion = packageJson.version;

  // 确定新版本
  const newVersion = options.version || await promptVersion(currentVersion);
  console.log(`\n📦 Releasing version: ${newVersion}`);

  if (!options.skipTests) {
    // 运行测试
    console.log("\n🧪 Running tests...");
    try {
      await $`bun test`;
      console.log("✅ Tests passed");
    } catch (error) {
      console.error("❌ Tests failed");
      process.exit(1);
    }

    // 类型检查
    console.log("\n🔍 Type checking...");
    try {
      await $`bunx tsc --noEmit`;
      console.log("✅ Type check passed");
    } catch (error) {
      console.error("❌ Type check failed");
      process.exit(1);
    }
  }

  // 更新版本号
  console.log("\n📝 Updating version numbers...");
  await updateVersion(newVersion, options.dryRun);

  // 构建应用
  console.log("\n🏗️ Building application...");
  if (!options.dryRun) {
    await $`bun run scripts/build.ts`;
  }

  // 生成更新日志
  console.log("\n📝 Generating changelog...");
  await generateChangelog(newVersion, options.dryRun);

  // 创建 Git 标签
  if (!options.skipTag && !options.dryRun) {
    console.log("\n🏷️ Creating Git tag...");
    await $`git add .`;
    await $`git commit -m "release: v${newVersion}"`;
    await $`git tag -a v${newVersion} -m "Release v${newVersion}"`;
    console.log(`✅ Created tag: v${newVersion}`);
  }

  // 完成
  console.log("\n✅ Release complete!");
  console.log("\nNext steps:");
  console.log(`1. Push changes: git push origin main --tags`);
  console.log(`2. Create GitHub release for v${newVersion}`);
  console.log(`3. Upload build artifacts`);

  if (options.dryRun) {
    console.log("\n⚠️ This was a dry run. No changes were made.");
  }
}

async function promptVersion(currentVersion: string): Promise<string> {
  console.log(`Current version: ${currentVersion}`);
  console.log("\nSelect version bump:");
  console.log("1. Patch (x.x.X)");
  console.log("2. Minor (x.X.0)");
  console.log("3. Major (X.0.0)");
  console.log("4. Custom");

  const choice = prompt("Enter choice (1-4): ");
  
  const [major, minor, patch] = currentVersion.split(".").map(Number);

  switch (choice) {
    case "1":
      return `${major}.${minor}.${patch + 1}`;
    case "2":
      return `${major}.${minor + 1}.0`;
    case "3":
      return `${major + 1}.0.0`;
    case "4":
      const custom = prompt("Enter version: ");
      if (!custom || !/^\d+\.\d+\.\d+$/.test(custom)) {
        console.error("Invalid version format");
        process.exit(1);
      }
      return custom;
    default:
      console.error("Invalid choice");
      process.exit(1);
  }
}

async function updateVersion(version: string, dryRun: boolean) {
  // 更新 package.json
  const packagePath = "package.json";
  const packageJson = JSON.parse(readFileSync(packagePath, "utf-8"));
  packageJson.version = version;
  
  if (!dryRun) {
    writeFileSync(packagePath, JSON.stringify(packageJson, null, 2) + "\n");
  }
  console.log(`✅ Updated ${packagePath}`);

  // 更新 Cargo.toml
  const cargoPath = "src-tauri/Cargo.toml";
  let cargoContent = readFileSync(cargoPath, "utf-8");
  cargoContent = cargoContent.replace(
    /version = "\d+\.\d+\.\d+"/,
    `version = "${version}"`
  );
  
  if (!dryRun) {
    writeFileSync(cargoPath, cargoContent);
  }
  console.log(`✅ Updated ${cargoPath}`);
}

async function generateChangelog(version: string, dryRun: boolean) {
  const date = new Date().toISOString().split("T")[0];
  const changelogEntry = `
## [${version}] - ${date}

### Added
- 

### Changed
- 

### Fixed
- 

### Removed
- 

`;

  const changelogPath = "CHANGELOG.md";
  const existingChangelog = readFileSync(changelogPath, "utf-8").trim();
  
  if (!existingChangelog) {
    const newChangelog = `# Changelog\n\nAll notable changes to this project will be documented in this file.\n${changelogEntry}`;
    if (!dryRun) {
      writeFileSync(changelogPath, newChangelog);
    }
  } else {
    const newChangelog = existingChangelog.replace(
      "# Changelog",
      `# Changelog\n${changelogEntry}`
    );
    if (!dryRun) {
      writeFileSync(changelogPath, newChangelog);
    }
  }
  
  console.log(`✅ Updated ${changelogPath}`);
}

// CLI 参数解析
if (import.meta.main) {
  const args = process.argv.slice(2);
  const options: ReleaseOptions = {};

  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case "--version":
      case "-v":
        options.version = args[++i];
        break;
      case "--skip-tests":
        options.skipTests = true;
        break;
      case "--skip-tag":
        options.skipTag = true;
        break;
      case "--dry-run":
        options.dryRun = true;
        break;
      case "--help":
      case "-h":
        console.log("Usage: bun run scripts/release.ts [options]");
        console.log("\nOptions:");
        console.log("  -v, --version <version>  Specify version");
        console.log("  --skip-tests            Skip running tests");
        console.log("  --skip-tag              Skip creating git tag");
        console.log("  --dry-run               Perform a dry run");
        console.log("  -h, --help              Show help");
        process.exit(0);
    }
  }

  await release(options);
}