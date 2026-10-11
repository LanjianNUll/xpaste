#!/usr/bin/env node
/**
 * 按「上次发版 tag → 当前 HEAD」的提交记录生成更新日志。
 *
 * 用法：
 *   node scripts/gen-release-notes.mjs                     # 用 package.json 的版本号，生成 release-notes/<版本>.md
 *   node scripts/gen-release-notes.mjs --stdout            # 只打印不写文件（发版工作流用这个）
 *   node scripts/gen-release-notes.mjs --version 2.0.9     # 指定版本号
 *   node scripts/gen-release-notes.mjs --summary "本版本重点实现 xx 功能"  # 写死一句话总结
 *   node scripts/gen-release-notes.mjs --from v2.0.7        # 手动指定对比起点 tag
 *   node scripts/gen-release-notes.mjs --force             # 覆盖已存在的日志文件
 *
 * 为什么需要它：一条提交的信息往往太短，直接贴进 Release 日志既没有重点也不成结构。
 * 这里按 Conventional Commits 的类型分组、去重、清洗标点，产出可直接发版的结构化日志。
 */
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";
import { pathToFileURL } from "node:url";

/** 分组定义：提交类型 → 展示用小节标题，顺序即日志里的先后 */
const GROUPS = [
  { title: "✨ 新增功能", types: ["feat"] },
  { title: "🛠 修复与优化", types: ["fix", "perf", "refactor", "style"] },
  { title: "🔧 工程与构建", types: ["build", "ci", "chore", "test", "upd"] },
  { title: "📝 文档", types: ["docs"] }
];

/** 提交类型与「类型(范围)」的识别范围 */
const TYPE_PATTERN = /(feat|fix|perf|refactor|style|docs|chore|build|ci|test|upd)(\([^)]*\))?[:：]\s*/g;

/** 执行 git 命令并返回去掉首尾空白的输出 */
function git(args) {
  return execFileSync("git", args, { encoding: "utf8" }).trim();
}

/** 把 v2.0.8 这类 tag 解析成 [2,0,8]，解析不出来返回 null */
function parseTag(tag) {
  const matched = /^v?(\d+)\.(\d+)\.(\d+)/.exec(String(tag).trim());
  return matched ? [Number(matched[1]), Number(matched[2]), Number(matched[3])] : null;
}

/** a 是否严格小于 b（只比前三段版本号） */
function isLowerThan(a, b) {
  for (let i = 0; i < 3; i += 1) {
    if (a[i] !== b[i]) return a[i] < b[i];
  }
  return false;
}

/** 找出所有正式版本号 tag 中，严格小于当前版本的最大那个 */
function findPreviousTag(version) {
  const current = parseTag(version);
  const tags = git(["tag", "-l"]).split(/\r?\n/).filter(Boolean);
  let best = null;
  for (const tag of tags) {
    const parsed = parseTag(tag);
    if (!parsed || (current && !isLowerThan(parsed, current))) continue;
    if (!best || isLowerThan(best.parsed, parsed)) best = { tag, parsed };
  }
  return best?.tag ?? null;
}

/** 去掉句尾标点、多余空格与半角/全角混排的杂字符 */
function cleanText(text) {
  return String(text)
    .replace(/\s+/g, " ")
    .replace(/[，,、；;]+$/, "")
    .trim();
}

/**
 * 把一条提交标题拆成多条待展示的条目。
 * 兼容 `feat: A feat: B feat: C` 这种一次提交里塞了多条改动的情况。
 */
function parseSubject(subject) {
  const regex = new RegExp(TYPE_PATTERN.source, "g");
  const items = [];
  let matched;
  let last = null;
  while ((matched = regex.exec(subject)) !== null) {
    if (last && matched.index > last.end) {
      items.push({ type: last.type, scope: last.scope, text: cleanText(subject.slice(last.end, matched.index)) });
    }
    last = { type: matched[1], scope: matched[2] ? matched[2].slice(1, -1) : "", end: regex.lastIndex };
  }
  if (last) items.push({ type: last.type, scope: last.scope, text: cleanText(subject.slice(last.end)) });
  return items.filter((item) => item.text);
}

/** 读取对比区间内的提交（不含合并提交），按时间从旧到新返回 */
function readCommits(from, to) {
  const raw = git(["log", "--no-merges", "--reverse", "--pretty=format:%s", `${from}..${to}`]);
  if (!raw) return [];
  return raw.split(/\r?\n/).map(parseSubject).flat();
}

/** 把条目按分组归类，同一小节内去重 */
function groupItems(commits) {
  const grouped = GROUPS.map((group) => ({ title: group.title, items: [] }));
  const typeToGroup = new Map();
  GROUPS.forEach((group, index) => group.types.forEach((type) => typeToGroup.set(type, index)));

  for (const commit of commits) {
    const index = typeToGroup.get(commit.type);
    if (index === undefined) continue;
    const bucket = grouped[index];
    // 同一小节里描述完全相同的条目只保留第一条
    if (bucket.items.some((item) => item.text === commit.text)) continue;
    bucket.items.push(commit);
  }
  return grouped.filter((group) => group.items.length > 0);
}

/** 取远程仓库地址，用于生成对比链接 */
function getRepoUrl() {
  try {
    const remote = git(["remote", "get-url", "origin"]);
    return remote
      .replace(/^git@github\.com:/, "https://github.com/")
      .replace(/\.git$/, "");
  } catch {
    return "";
  }
}

/** 组装成最终 Markdown */
function renderNotes({ version, tag, previousTag, summary, groups }) {
  const lines = [`## 🎉 Xpaste v${version}`, ""];
  lines.push(summary || "（一句话说明这个版本的重点，可编辑本文件补充）");
  lines.push("");

  for (const group of groups) {
    lines.push(`### ${group.title}`, "");
    for (const item of group.items) {
      const scope = item.scope ? `（${item.scope}）` : "";
      lines.push(`- ${scope}${item.text}`);
    }
    lines.push("");
  }

  const repoUrl = getRepoUrl();
  if (previousTag && repoUrl) {
    lines.push(`**完整对比**：${repoUrl}/compare/${previousTag}...${tag}`);
  }
  return `${lines.join("\n").replace(/\n{3,}/g, "\n\n").trim()}\n`;
}

/** 解析命令行参数 */
function parseArgs(argv) {
  const args = { stdout: false, force: false, version: null, from: null, to: "HEAD", summary: "", out: null };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--stdout") args.stdout = true;
    else if (arg === "--force") args.force = true;
    else if (arg === "--version") args.version = argv[(i += 1)];
    else if (arg === "--from") args.from = argv[(i += 1)];
    else if (arg === "--to") args.to = argv[(i += 1)];
    else if (arg === "--summary") args.summary = argv[(i += 1)];
    else if (arg === "--out") args.out = argv[(i += 1)];
  }
  return args;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const version = args.version ?? JSON.parse(readFileSync("package.json", "utf8")).version;
  const tag = `v${version}`;
  const previousTag = args.from ?? findPreviousTag(version);

  if (!previousTag) {
    console.error(`✗ 找不到 ${version} 之前的历史 tag，无法确定对比区间`);
    process.exit(1);
  }

  const commits = readCommits(previousTag, args.to);
  if (commits.length === 0) {
    console.error(`✗ ${previousTag}..${args.to} 之间没有提交，没有可生成的日志`);
    process.exit(1);
  }

  const groups = groupItems(commits);
  if (groups.length === 0) {
    console.error(`✗ ${previousTag}..${args.to} 之间的提交都不符合 Conventional Commits 格式，无法归类`);
    process.exit(1);
  }

  const notes = renderNotes({ version, tag, previousTag, summary: args.summary, groups });

  if (args.stdout) {
    process.stdout.write(notes);
    return;
  }

  const outFile = args.out ?? `release-notes/${version}.md`;
  if (existsSync(outFile) && !args.force) {
    console.error(`✗ ${outFile} 已存在，加 --force 才会覆盖`);
    process.exit(1);
  }
  mkdirSync(dirname(outFile), { recursive: true });
  writeFileSync(outFile, notes, "utf8");

  const count = groups.reduce((sum, group) => sum + group.items.length, 0);
  console.log(`✓ 已按 ${previousTag}..${args.to} 的 ${count} 条改动生成 ${outFile}`);
  for (const group of groups) console.log(`  ${group.title} ${group.items.length} 条`);
}

// 直接运行时才执行主流程，被测试脚本 import 时只导出纯函数
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}

export { parseSubject, parseTag, isLowerThan, findPreviousTag, groupItems, renderNotes, parseArgs };