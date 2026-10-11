#!/usr/bin/env node
/**
 * 版本号统一更新：一次改好所有记录版本号的位置。
 *
 * 用法：pnpm version:bump 2.0.8
 *
 * 为什么需要它：Tauri 应用的版本号散落在 4 个文件里，
 * 只改一处会导致构建产物版本对不上，CI 里的发版流程也会直接拒绝发布。
 */
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { execFileSync } from "node:child_process";

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version ?? "")) {
  console.error("用法：pnpm version:bump <主版本.次版本.修订号>   例如：pnpm version:bump 2.0.8");
  process.exit(1);
}

const targets = [
  {
    file: "package.json",
    label: "package.json",
    apply: (source) => source.replace(/("version":\s*)"[^"]+"/, (_, prefix) => `${prefix}"${version}"`)
  },
  {
    file: "src-tauri/tauri.conf.json",
    label: "src-tauri/tauri.conf.json（安装包版本号）",
    apply: (source) => source.replace(/("version":\s*)"[^"]+"/, (_, prefix) => `${prefix}"${version}"`)
  },
  {
    file: "src-tauri/Cargo.toml",
    label: "src-tauri/Cargo.toml（exe 文件属性）",
    // 只匹配 [package] 段里行首的 version，不会动到依赖里写的 version
    apply: (source) => source.replace(/^(version = )"[^"]+"/m, (_, prefix) => `${prefix}"${version}"`)
  },
  {
    file: "src-tauri/Cargo.lock",
    label: "src-tauri/Cargo.lock（本包自身的版本记录）",
    apply: (source) =>
      source.replace(
        /(\[\[package\]\]\r?\nname = "paste_app"\r?\nversion = )"[^"]+"/,
        (_, prefix) => `${prefix}"${version}"`
      )
  }
];

let failed = false;
for (const { file, label, apply } of targets) {
  let source;
  try {
    source = readFileSync(file, "utf8");
  } catch {
    console.error(`✗ 读不到 ${file}`);
    failed = true;
    continue;
  }
  const updated = apply(source);
  if (updated === source && !source.includes(`"${version}"`)) {
    console.error(`✗ ${file} 里没找到可替换的版本号，请手动检查`);
    failed = true;
    continue;
  }
  writeFileSync(file, updated, "utf8");
  console.log(`✓ ${label} → ${version}`);
}

if (failed) {
  console.error("\n有文件没能更新，请手动处理后重试。");
  process.exit(1);
}

/**
 * 顺手准备好这个版本的更新日志文件：release-notes/<版本>.md。
 * 发版流程会优先用它作为 Release 说明与"应用内更新提示"里的更新日志；
 * 没有这个文件时会按提交记录自动生成（默认也能用）。
 */
const noteFile = `release-notes/${version}.md`;
if (existsSync(noteFile)) {
  console.log(`✓ 更新日志文件已存在：${noteFile}（发版时会用它）`);
} else {
  mkdirSync("release-notes", { recursive: true });
  let commits = "";
  try {
    const previousTag = execFileSync("git", ["describe", "--tags", "--abbrev=0"], { encoding: "utf8" }).trim();
    commits = execFileSync("git", ["log", "--no-merges", "--pretty=format:- %s", `${previousTag}..HEAD`], {
      encoding: "utf8"
    }).trim();
    console.log(`✓ 已按 ${previousTag}..HEAD 的提交生成 ${noteFile}（可自行润色）`);
  } catch {
    console.log(`✓ 已生成空的 ${noteFile}（可自行填写）`);
  }
  const template = `## 🎉 Xpaste v${version}

一句话说明这个版本的重点。

### ✨ 新增

- 

### 🛠 修复 / 优化

- 

<!-- 下面这段是本版本已有的提交记录，可作参考后删除 -->
${commits ? `\n<!--\n${commits}\n-->\n` : ""}`;
  writeFileSync(noteFile, template, "utf8");
}

console.log(`
版本号已更新到 ${version}，接下来：

  1. 编辑 release-notes/${version}.md 写下这个版本的更新日志（可选，不写则按提交自动生成）
  2. git commit -am "chore: release v${version}"
  3. git push

推送后 GitHub Actions 会自动构建 Windows 安装包 + 自动更新包 + 便携版，
并生成更新日志发布 Release。也可以直接打 tag：

  git tag v${version} && git push origin v${version}
`);
