<template>
  <div class="editor-shell">
    <!-- 与主窗口、快捷面板共用同一套皮肤与背景粒子 -->
    <AmbientLayer />

    <!-- 无边框窗口：这条顶栏同时是拖拽区，点空白处可以挪窗口 -->
    <div class="editor-bar" data-tauri-drag-region>
      <span class="editor-title" data-tauri-drag-region>编辑器</span>
      <span class="editor-count">{{ draft.length }} 字</span>
      <button
        type="button"
        class="editor-bar-btn"
        :class="{ 'is-active': sidebarOpen }"
        :title="sidebarOpen ? '收起历史记录（⌘\\）' : '展开历史记录，挑素材插入（⌘\\）'"
        @click="toggleSidebar"
      >
        <el-icon :size="13"><Clock /></el-icon>
      </button>
      <button type="button" class="editor-bar-btn editor-close" title="关闭（Esc）" @click="handleClose">
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M0 0l10 10M10 0L0 10" /></svg>
      </button>
    </div>

    <!-- 来源图例：插进来的内容带底色，一眼能看出哪些是挑来的、哪些是自己写的 -->
    <div v-if="hasChips" class="editor-legend">
      <span class="legend-item"><i class="legend-dot legend-dot--panel"></i>面板选中</span>
      <span class="legend-item"><i class="legend-dot legend-dot--history"></i>历史插入</span>
      <span class="legend-note">改动后自动合并为普通文字</span>
    </div>

    <div class="editor-body">
      <div class="editor-main">
        <div
          ref="editorRef"
          class="editor-area"
          contenteditable="true"
          spellcheck="false"
          role="textbox"
          aria-multiline="true"
          :data-placeholder="placeholder"
          :data-empty="!hasContent"
          @input="handleInput"
          @paste="handlePaste"
          @drop="handleDrop"
        ></div>

        <div class="editor-actions">
          <button type="button" class="editor-primary" :disabled="pasting" @click="submit('paste')">
            粘贴到当前应用
          </button>
          <button type="button" class="editor-ghost" @click="submit('copy')">仅复制</button>
          <button type="button" class="editor-ghost" @click="submit('save')">存为记录</button>
          <button
            type="button"
            class="editor-link"
            :class="{ 'is-confirming': clearing }"
            @click="clearDraft"
          >
            {{ clearing ? "确认清空" : "清空" }}
          </button>
          <span class="editor-hint" title="⌘↵ 粘贴到当前应用 · Esc 关闭编辑器">⌘↵ 粘贴</span>
        </div>
      </div>

      <!--
        右侧历史栏：默认收起；展开时后端把窗口一并加宽（贴着面板的那条边不动），
        所以这里的宽度是固定的 260，不靠撑开主编辑区来让位。
      -->
      <aside v-if="sidebarOpen" class="editor-sidebar">
        <div class="hist-head">
          <span class="hist-title">历史记录</span>
          <span v-if="sidebarItems.length > 0" class="hist-count">{{ sidebarItems.length }}</span>
        </div>

        <div class="hist-search">
          <input
            v-model="sidebarKeyword"
            type="text"
            class="hist-input"
            placeholder="搜索历史…"
            spellcheck="false"
            @input="scheduleSidebarLoad"
          />
        </div>

        <div class="hist-list">
          <div
            v-for="item in sidebarItems"
            :key="item.id"
            class="hist-item"
            :class="{ 'is-picked': pickedId === item.id }"
            role="button"
            tabindex="0"
            :title="itemPreview(item)"
            @click="pickSidebarItem(item)"
            @keydown.enter.prevent="pickSidebarItem(item)"
            @keydown.space.prevent="pickSidebarItem(item)"
          >
            <span class="hist-text">{{ itemPreview(item) }}</span>
            <span class="hist-meta">
              <span>{{ itemTypeLabel(item) }}</span>
              <span>{{ itemTimeLabel(item.createdAt) }}</span>
            </span>
          </div>

          <p v-if="sidebarLoading && sidebarItems.length === 0" class="hist-empty">加载中…</p>
          <p v-else-if="sidebarFailed" class="hist-empty">历史记录读取失败，改一下关键词再试</p>
          <p v-else-if="sidebarItems.length === 0" class="hist-empty">
            {{ sidebarKeyword.trim() ? "没有匹配的记录" : "还没有可插入的记录" }}
          </p>
        </div>
      </aside>
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { ElMessage } from "element-plus";
import { Clock } from "@element-plus/icons-vue";
import type { ClipboardItem } from "@/types";
import AmbientLayer from "@/components/AmbientLayer.vue";
import {
  closeEditor,
  copyText,
  fetchHistory,
  getItemText,
  pasteText,
  saveClipboardText,
  setEditorSidebar,
  subscribeEditorInsert,
  subscribeEditorOpened,
  subscribeEditorSeed
} from "@/services/api";

/** 与快捷面板共用同一个草稿：两个窗口来回切时内容不会断 */
const DRAFT_KEY = "xpaste.composeDraft";
/** 草稿的结构化副本：记录每段内容来自"面板"还是"历史"，重开窗口后高亮不丢 */
const SEGMENTS_KEY = "xpaste.composeDraftSegments";
/** 后端列表把 text 截到 4096 字符，到达这个长度就得回源取全文 */
const TEXT_TRUNCATE_LIMIT = 4096;
/** 侧栏一次取多少条：够翻一阵，又不至于把长文本全拉进内存 */
const SIDEBAR_LIMIT = 60;

/** 插入来源：面板选中与历史插入用不同底色区分 */
type InsertSource = "panel" | "history";

interface SavedSegment {
  type: "text" | "chip";
  text: string;
  source?: InsertSource;
  merged?: boolean;
}

const placeholder =
  "点左侧面板或右侧历史里的任一条，内容会插到光标这里；也可以直接写。⌘↵ 粘贴到当前应用。";

const draft = ref("");
const pasting = ref(false);
/** 编辑区有没有内容：contenteditable 没有 v-model，占位符要自己控制 */
const hasContent = ref(false);
/** 是否还有未合并的高亮片段：有才显示图例，避免日常输入时占一行 */
const hasChips = ref(false);
const editorRef = ref<HTMLDivElement | null>(null);
let unlistenInsert: (() => void) | null = null;
let unlistenOpened: (() => void) | null = null;
let unlistenSeed: (() => void) | null = null;

const sidebarOpen = ref(false);
const sidebarKeyword = ref("");
const sidebarItems = ref<ClipboardItem[]>([]);
const sidebarLoading = ref(false);
const sidebarFailed = ref(false);
const pickedId = ref(-1);
let sidebarTimer: number | null = null;
let sidebarRequestId = 0;

/** 清空草稿要按两下：这个窗口里弹确认框太重，误触一次就丢一屏文字更糟 */
const clearing = ref(false);
let clearTimer: number | null = null;

const categoryLabel: Record<ClipboardItem["category"], string> = {
  link: "链接",
  image: "图片",
  text: "文本",
  file: "文件",
  folder: "文件夹"
};

const formatLabel: Record<ClipboardItem["format"], string> = {
  text: "文本",
  image: "图片",
  html: "HTML",
  file: "文件",
  color: "颜色"
};

/** 把编辑区的 DOM 拆成可持久化的段落：文字一段、每个高亮片段一段。 */
function serializeEditor(): SavedSegment[] {
  const editor = editorRef.value;
  if (!editor) return [];
  const segments: SavedSegment[] = [];
  editor.childNodes.forEach((node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      const text = node.textContent ?? "";
      if (text) segments.push({ type: "text", text });
      return;
    }
    if (node.nodeType !== Node.ELEMENT_NODE) return;
    const element = node as HTMLElement;
    if (element.classList.contains("editor-chip")) {
      segments.push({
        type: "chip",
        text: element.textContent ?? "",
        source: element.dataset.source === "history" ? "history" : "panel",
        merged: element.classList.contains("is-merged")
      });
      return;
    }
    // 兜底：浏览器插入的其它节点按纯文本收集，避免丢内容
    const text = element.textContent ?? "";
    if (text) segments.push({ type: "text", text });
  });
  return segments;
}

function persistDraft() {
  try {
    window.localStorage.setItem(DRAFT_KEY, draft.value);
    window.localStorage.setItem(SEGMENTS_KEY, JSON.stringify(serializeEditor()));
  } catch {
    // 存不下不影响当前这次编辑
  }
}

function loadDraft() {
  try {
    draft.value = window.localStorage.getItem(DRAFT_KEY) ?? "";
  } catch {
    draft.value = "";
  }
}

/** 读取上次保存的段落；和纯文本对不上说明草稿在别处改过，按纯文本重建。 */
function readSegments(): SavedSegment[] | null {
  try {
    const raw = window.localStorage.getItem(SEGMENTS_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as SavedSegment[];
    if (!Array.isArray(parsed)) return null;
    const joined = parsed.map((segment) => segment.text ?? "").join("");
    return joined === draft.value ? parsed : null;
  } catch {
    return null;
  }
}

function createChip(text: string, source: InsertSource, merged = false): HTMLSpanElement {
  const chip = document.createElement("span");
  chip.className = `editor-chip editor-chip--${source}${merged ? " is-merged" : ""}`;
  chip.dataset.origin = text;
  chip.dataset.source = source;
  chip.textContent = text;
  return chip;
}

/** 把保存的段落还原成 DOM；没有结构化数据时退回一整段纯文本。 */
function renderEditor() {
  const editor = editorRef.value;
  if (!editor) return;
  editor.innerHTML = "";
  const segments = readSegments();
  if (segments) {
    for (const segment of segments) {
      if (!segment.text) continue;
      if (segment.type === "chip") {
        editor.appendChild(
          createChip(segment.text, segment.source === "history" ? "history" : "panel", segment.merged)
        );
      } else {
        editor.appendChild(document.createTextNode(segment.text));
      }
    }
  } else if (draft.value) {
    editor.appendChild(document.createTextNode(draft.value));
  }
  syncFromEditor();
}

/** 编辑器 → 数据：更新纯文本、空状态、图例显示，并持久化。 */
function syncFromEditor() {
  const editor = editorRef.value;
  if (!editor) return;
  draft.value = editor.textContent ?? "";
  hasContent.value = draft.value.length > 0;
  hasChips.value = editor.querySelector(".editor-chip:not(.is-merged)") != null;
  persistDraft();
}

/** 内容被改动过的片段立即去掉底色，和后面的普通文字"合"在一起。 */
function markEditedChips() {
  const editor = editorRef.value;
  if (!editor) return;
  editor.querySelectorAll<HTMLElement>(".editor-chip:not(.is-merged)").forEach((chip) => {
    if ((chip.textContent ?? "") !== (chip.dataset.origin ?? "")) {
      chip.classList.add("is-merged");
    }
  });
}

/** 一敲键盘就当作"刚才没打算清空"，把两段确认复位 */
function handleInput() {
  markEditedChips();
  syncFromEditor();
  if (!clearing.value) return;
  clearing.value = false;
  if (clearTimer != null) {
    window.clearTimeout(clearTimer);
    clearTimer = null;
  }
}

/** 当前光标范围；光标不在编辑区（比如刚点了侧栏）时落到末尾。 */
function currentRange(): Range | null {
  const editor = editorRef.value;
  if (!editor) return null;
  const selection = window.getSelection();
  if (selection && selection.rangeCount > 0) {
    const range = selection.getRangeAt(0);
    if (editor.contains(range.commonAncestorContainer)) return range;
  }
  const range = document.createRange();
  range.selectNodeContents(editor);
  range.collapse(false);
  return range;
}

/** 在光标处插入一串节点，并把光标放到最后一个节点之后。 */
function insertNodes(nodes: Node[]) {
  const editor = editorRef.value;
  if (!editor || nodes.length === 0) return;
  const range = currentRange();
  if (!range) return;
  range.deleteContents();
  const fragment = document.createDocumentFragment();
  for (const node of nodes) fragment.appendChild(node);
  const last = fragment.lastChild;
  range.insertNode(fragment);
  if (last) {
    const caret = document.createRange();
    caret.setStartAfter(last);
    caret.collapse(true);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(caret);
  }
  syncFromEditor();
}

function focusAtEnd() {
  void nextTick(() => {
    const editor = editorRef.value;
    if (!editor) return;
    editor.focus();
    const range = document.createRange();
    range.selectNodeContents(editor);
    range.collapse(false);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);
  });
}

/**
 * 在光标处插入一段素材。
 *
 * 前后各补一个换行（只在会粘到已有文字时才补）：连着插好几条时不会糊成一坨，
 * 用户想拆开也只需删掉空行。插入的内容先带来源底色，改动后自动去掉。
 */
function insertText(text: string, source: InsertSource) {
  if (!text) return;
  const editor = editorRef.value;
  if (!editor) return;
  // 打开编辑器时已经默认带入了"选中的那条"，再点一次同一条不该变成两份
  if (draft.value.trim() === text.trim()) return;

  const range = currentRange();
  if (!range) return;
  const beforeRange = range.cloneRange();
  beforeRange.selectNodeContents(editor);
  beforeRange.setEnd(range.startContainer, range.startOffset);
  const before = beforeRange.toString();
  const afterRange = range.cloneRange();
  afterRange.selectNodeContents(editor);
  afterRange.setStart(range.endContainer, range.endOffset);
  const after = afterRange.toString();

  const nodes: Node[] = [];
  if (before.length > 0 && !before.endsWith("\n")) nodes.push(document.createTextNode("\n"));
  nodes.push(createChip(text, source));
  if (after.length > 0 && !after.startsWith("\n")) nodes.push(document.createTextNode("\n"));

  // 焦点不在编辑器时不要抢回来：用户会连着点好几条，抢焦点会让两个窗口来回闪
  const hadFocus = document.hasFocus();
  insertNodes(nodes);
  if (hadFocus) editor.focus();
}

/** 插入纯文本（粘贴、回车换行都走这里，保证 DOM 里只有文本节点）。 */
function insertPlainText(text: string) {
  if (!text) return;
  insertNodes([document.createTextNode(text)]);
}

function handlePaste(event: ClipboardEvent) {
  event.preventDefault();
  const text = event.clipboardData?.getData("text/plain") ?? "";
  insertPlainText(text.replace(/\r\n/g, "\n"));
}

function handleDrop(event: DragEvent) {
  event.preventDefault();
  const text = event.dataTransfer?.getData("text/plain") ?? "";
  insertPlainText(text.replace(/\r\n/g, "\n"));
}

/**
 * 打开编辑器时的默认内容：左面板里高亮的那一条。
 *
 * 只在草稿为空时采用——手上还有没送出去的文字时，清掉它比"没默认带入"更糟。
 * 粘贴成功后草稿会清空，所以下一次打开编辑器又是干净的默认态。
 */
function applySeed(text: string) {
  if (!text) return;
  if (draft.value.trim().length > 0) return;
  const editor = editorRef.value;
  if (!editor) return;
  if (editor.childNodes.length > 0) return;
  // 只挪光标，不抢焦点：面板那边可能正在连着挑素材
  const range = document.createRange();
  range.selectNodeContents(editor);
  range.collapse(false);
  const selection = window.getSelection();
  selection?.removeAllRanges();
  selection?.addRange(range);
  insertText(text, "panel");
}

// ── 右侧历史栏 ────────────────────────────────────────────────────────────

function itemPreview(item: ClipboardItem) {
  if (item.format === "image") return "[图片]";
  const raw = item.text ?? item.html ?? item.filePath ?? item.color ?? "";
  const text = raw
    .replace(/<[^>]+>/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  if (!text) return "[空]";
  return text.length > 160 ? `${text.slice(0, 160)}…` : text;
}

function itemTypeLabel(item: ClipboardItem) {
  if (item.category === "folder") return "文件夹";
  return `${categoryLabel[item.category]} / ${formatLabel[item.format]}`;
}

/** 当天的只显示时分，往前显示日期：侧栏窄，一行要放得下 */
function itemTimeLabel(ts: number) {
  const date = new Date(ts);
  const now = new Date();
  const clock = `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
  if (date.toDateString() === now.toDateString()) return clock;
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (date.toDateString() === yesterday.toDateString()) return `昨天 ${clock}`;
  if (date.getFullYear() === now.getFullYear()) {
    return `${date.getMonth() + 1}月${date.getDate()}日`;
  }
  return `${date.getFullYear()}/${date.getMonth() + 1}/${date.getDate()}`;
}

async function loadSidebar() {
  const requestId = ++sidebarRequestId;
  sidebarLoading.value = true;
  sidebarFailed.value = false;
  try {
    const list = await fetchHistory(sidebarKeyword.value.trim(), SIDEBAR_LIMIT);
    if (requestId !== sidebarRequestId) return;
    // 图片没有可编辑的文本形态，留在列表里只会点出一句"不支持"
    sidebarItems.value = list.filter((item) => item.format !== "image");
  } catch {
    if (requestId !== sidebarRequestId) return;
    sidebarItems.value = [];
    sidebarFailed.value = true;
  } finally {
    if (requestId === sidebarRequestId) sidebarLoading.value = false;
  }
}

function scheduleSidebarLoad() {
  if (sidebarTimer != null) window.clearTimeout(sidebarTimer);
  sidebarTimer = window.setTimeout(() => {
    sidebarTimer = null;
    void loadSidebar();
  }, 180);
}

/**
 * 展开 / 收起侧栏。
 *
 * 窗口宽度由后端一起改（向右长，左上角不动），这里只负责列表内容：先翻状态让
 * 内容立刻出现，宽度稍后跟上；后端失败就翻回去，别让界面状态和窗口宽度对不上。
 */
async function toggleSidebar() {
  const next = !sidebarOpen.value;
  sidebarOpen.value = next;
  if (next) void loadSidebar();
  try {
    await setEditorSidebar(next);
  } catch (error) {
    sidebarOpen.value = !next;
    ElMessage.error(typeof error === "string" ? error : "切换历史栏失败");
  }
}

/** 点侧栏条目 = 把这条插到光标处；插完焦点回到编辑区，方便接着写 */
async function pickSidebarItem(item: ClipboardItem) {
  try {
    const inline = item.text ?? item.html ?? item.color ?? item.filePath ?? "";
    const text =
      inline.length >= TEXT_TRUNCATE_LIMIT ? await getItemText(item.id, "history") : inline;
    insertText(text, "history");
    pickedId.value = item.id;
    window.setTimeout(() => {
      if (pickedId.value === item.id) pickedId.value = -1;
    }, 600);
  } catch (error) {
    ElMessage.error(typeof error === "string" ? error : "插入失败");
  }
}

// ── 送出 ──────────────────────────────────────────────────────────────────

/** 清空编辑区与持久化草稿：送出成功后调用，避免残留上一条的内容。 */
function resetEditor() {
  const editor = editorRef.value;
  if (editor) editor.innerHTML = "";
  draft.value = "";
  hasContent.value = false;
  hasChips.value = false;
  persistDraft();
}

async function submit(action: "paste" | "copy" | "save") {
  if (pasting.value) return;
  const text = draft.value;
  if (!text.trim()) {
    ElMessage.warning("编辑区还是空的");
    return;
  }

  pasting.value = true;
  try {
    if (action === "paste") {
      // 后端会把两个窗口一起收起，再把内容投递给原应用
      await pasteText(text);
      // 送出去就算这一轮结束：清空草稿，下次打开编辑器就带不出选中的那条了
      resetEditor();
    } else if (action === "copy") {
      await copyText(text);
      ElMessage.success("已复制到剪贴板");
      focusAtEnd();
    } else {
      await saveClipboardText(text);
      ElMessage.success("已存为剪贴板记录");
      // 刚存的这条就在历史里，侧栏开着就立刻刷出来，不然列表看起来"没存进去"
      if (sidebarOpen.value) void loadSidebar();
      focusAtEnd();
    }
  } catch (error) {
    const message = typeof error === "string" ? error : "操作失败。";
    ElMessage.error(message);
  } finally {
    pasting.value = false;
  }
}

function clearDraft() {
  if (!draft.value) return;
  // 第一下只是"举手"，2.5 秒内再点一下才真的清空——防手滑，又不用弹确认框
  if (!clearing.value) {
    clearing.value = true;
    if (clearTimer != null) window.clearTimeout(clearTimer);
    clearTimer = window.setTimeout(() => {
      clearing.value = false;
      clearTimer = null;
    }, 2500);
    return;
  }
  if (clearTimer != null) {
    window.clearTimeout(clearTimer);
    clearTimer = null;
  }
  clearing.value = false;
  resetEditor();
  focusAtEnd();
}

function handleClose() {
  closeEditor().catch(() => undefined);
}

function handleKeydown(event: KeyboardEvent) {
  // 输入法组字期间的 Esc / ↵ 属于候选框，不能当成快捷键吃掉
  const isComposing =
    (event as KeyboardEvent & { isComposing?: boolean }).isComposing ||
    event.keyCode === 229;
  if (isComposing) return;
  if (event.key === "Escape") {
    event.preventDefault();
    // 侧栏开着时先收侧栏，再按一次才关编辑器
    if (sidebarOpen.value) {
      void toggleSidebar();
      return;
    }
    handleClose();
    return;
  }
  // ⌘\ 开合右侧历史栏（和标题栏那颗按钮同一个动作）
  if ((event.metaKey || event.ctrlKey) && event.key === "\\") {
    event.preventDefault();
    void toggleSidebar();
    return;
  }
  // ⌘E 再按一次也收起编辑器，和打开它的手势对称
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "e") {
    event.preventDefault();
    handleClose();
    return;
  }
  // ⌘↵（Windows 上是 Ctrl+↵）送出
  if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
    event.preventDefault();
    void submit("paste");
    return;
  }
  // 普通回车 / Shift+回车：插纯文本换行，避免浏览器生成 <div>/<br> 破坏段落结构
  if (event.key === "Enter" && !event.altKey) {
    const editor = editorRef.value;
    const target = event.target as Node | null;
    if (editor && target && (target === editor || editor.contains(target))) {
      event.preventDefault();
      insertPlainText("\n");
    }
  }
}

onMounted(async () => {
  loadDraft();
  window.addEventListener("keydown", handleKeydown);

  // onMounted 时模板已挂载，直接按保存的段落还原（contenteditable 不能靠 v-model）
  renderEditor();

  try {
    unlistenInsert = await subscribeEditorInsert((text) => insertText(text, "panel"));
    unlistenSeed = await subscribeEditorSeed((text) => applySeed(text));
    unlistenOpened = await subscribeEditorOpened(() => {
      // 每次打开都回到"侧栏收起"的默认态；窗口宽度后端已经设回 420 了
      sidebarOpen.value = false;
      sidebarKeyword.value = "";
      pickedId.value = -1;
      focusAtEnd();
    });
  } catch {
    // 事件订阅失败不致死，仍可手动编辑
  }

  focusAtEnd();
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleKeydown);
  if (sidebarTimer != null) window.clearTimeout(sidebarTimer);
  if (clearTimer != null) window.clearTimeout(clearTimer);
  unlistenInsert?.();
  unlistenSeed?.();
  unlistenOpened?.();
});
</script>

<style scoped>
.editor-shell {
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
  background: var(--panel);
  color: var(--text);
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei",
    sans-serif;
}

.editor-bar {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  gap: 8px;
  height: 30px;
  padding: 0 8px 0 12px;
  border-bottom: 1px solid var(--stroke);
  background: var(--panel-subtle);
}

.editor-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-soft);
  letter-spacing: 0.01em;
}

.editor-count {
  font-size: 11px;
  color: var(--muted);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.editor-bar-btn {
  display: inline-flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--muted);
  cursor: pointer;
  transition: background 120ms ease, color 120ms ease;
}

/* 历史按钮自动贴右，关窗按钮跟在它后面 */
.editor-bar-btn:first-of-type {
  margin-left: auto;
}

.editor-bar-btn:hover {
  background: var(--control-bg-hover);
  color: var(--text);
}

.editor-bar-btn.is-active {
  background: var(--selection-bg);
  color: var(--accent);
}

.editor-close svg {
  width: 8px;
  height: 8px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.4;
  stroke-linecap: round;
}

.editor-body {
  display: flex;
  flex: 1;
  min-height: 0;
}

.editor-main {
  display: flex;
  /* 侧栏展开后窗口是 680：主编辑区 420、侧栏 260。屏幕放不下时两边一起让一点，
     但编辑区不会窄过 260——再窄就不适合写东西了。 */
  flex: 1 1 420px;
  flex-direction: column;
  min-width: 260px;
  min-height: 0;
}

/*
 * 编辑区用 contenteditable 而不是 textarea：只有 DOM 才能给"挑来的素材"
 * 单独加底色，textarea 里的文字只能同一种颜色。
 */
.editor-area {
  position: relative;
  flex: 1;
  min-height: 0;
  padding: 12px 14px;
  overflow-y: auto;
  outline: none;
  color: var(--text);
  font: inherit;
  font-size: 13px;
  line-height: 1.7;
  white-space: pre-wrap;
  overflow-wrap: break-word;
  cursor: text;
}

.editor-area[data-empty="true"]::before {
  content: attr(data-placeholder);
  position: absolute;
  top: 12px;
  right: 14px;
  left: 14px;
  color: var(--muted);
  white-space: normal;
  pointer-events: none;
}

/* 面板选中：主色底；历史插入：琥珀色底。换行时每一行都带上标识 */
.editor-chip {
  padding: 0 2px;
  border-radius: 4px;
  background: color-mix(in srgb, var(--accent) 14%, transparent);
  box-shadow: inset 2px 0 0 var(--accent);
  -webkit-box-decoration-break: clone;
  box-decoration-break: clone;
  transition: background 140ms ease, box-shadow 140ms ease;
}

.editor-chip--history {
  background: color-mix(in srgb, var(--el-color-warning) 18%, transparent);
  box-shadow: inset 2px 0 0 var(--el-color-warning);
}

/* 一旦改动过就褪去底色，和用户自己写的文字合为一体 */
.editor-chip.is-merged {
  padding: 0;
  background: transparent;
  box-shadow: none;
}

.editor-legend {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  gap: 10px;
  height: 22px;
  padding: 0 14px;
  border-bottom: 1px solid var(--stroke);
  background: var(--panel-subtle);
  font-size: 10.5px;
  color: var(--muted);
}

.legend-item {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  white-space: nowrap;
}

.legend-dot {
  width: 8px;
  height: 8px;
  border-radius: 2px;
  background: var(--accent);
}

.legend-dot--history {
  background: var(--el-color-warning);
}

.legend-note {
  margin-left: auto;
  white-space: nowrap;
}

.editor-actions {
  display: flex;
  flex-shrink: 0;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  padding: 8px 10px;
  border-top: 1px solid var(--stroke);
  background: var(--panel-subtle);
}

.editor-primary,
.editor-ghost,
.editor-link {
  border: 0;
  border-radius: var(--radius-sm);
  font-family: inherit;
  font-size: 12px;
  line-height: 1;
  cursor: pointer;
  transition: background 120ms ease, opacity 120ms ease;
}

.editor-primary {
  padding: 7px 12px;
  background: var(--accent);
  color: var(--accent-contrast);
  font-weight: 600;
}

.editor-primary:hover:not(:disabled) {
  background: var(--accent-hover);
}

.editor-primary:disabled {
  opacity: 0.6;
  cursor: default;
}

.editor-ghost {
  padding: 7px 10px;
  border: 1px solid var(--stroke);
  background: var(--control-bg);
  color: var(--text-soft);
}

.editor-ghost:hover {
  background: var(--control-bg-hover);
}

.editor-link {
  padding: 7px 4px;
  background: transparent;
  color: var(--muted);
}

.editor-link:hover {
  color: var(--text);
}

.editor-link.is-confirming {
  color: var(--el-color-danger);
  font-weight: 600;
}

.editor-link.is-confirming:hover {
  color: var(--el-color-danger);
}

.editor-hint {
  margin-left: auto;
  font-size: 10.5px;
  color: var(--muted);
  white-space: nowrap;
}

/* ── 右侧历史栏 ───────────────────────────────────────────────────────── */

.editor-sidebar {
  display: flex;
  flex: 1 1 260px;
  flex-direction: column;
  min-width: 200px;
  max-width: 260px;
  min-height: 0;
  border-left: 1px solid var(--stroke);
  background: var(--panel-subtle);
  animation: hist-in 140ms ease both;
}

@keyframes hist-in {
  from {
    opacity: 0;
    transform: translateX(6px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

.hist-head {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 12px;
  border-bottom: 1px solid var(--stroke);
}

.hist-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-soft);
  letter-spacing: 0.02em;
}

.hist-count {
  font-size: 10px;
  color: var(--muted);
  font-variant-numeric: tabular-nums;
}

.hist-search {
  flex-shrink: 0;
  padding: 8px 10px 6px;
}

.hist-input {
  width: 100%;
  height: 26px;
  padding: 0 8px;
  border: 1px solid var(--stroke);
  border-radius: var(--radius-sm);
  outline: none;
  background: var(--control-bg);
  color: var(--text);
  font: inherit;
  font-size: 12px;
}

.hist-input::placeholder {
  color: var(--muted);
}

.hist-input:focus {
  border-color: var(--selection-border);
  box-shadow: 0 0 0 2px var(--selection-bg);
}

.hist-list {
  flex: 1;
  min-height: 0;
  padding: 2px 6px 6px;
  overflow-y: auto;
}

/*
 * 注意别把 .hist-item 写成 flex 容器：它是 <button>，一旦建立 flex 上下文，
 * 子元素会被 blockify（display:-webkit-box 会被算成 flow-root），
 * `-webkit-line-clamp` 随即失效，行高塌成几个像素、文字只剩一条边。
 * 这里用最普通的 block 布局 + 内层 flex，Chrome / WKWebView 表现一致。
 */
.hist-item {
  display: block;
  width: 100%;
  padding: 6px 7px;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: inherit;
  text-align: left;
  cursor: pointer;
  transition: background 110ms ease;
}

.hist-item:hover {
  background: var(--hover-bg);
}

.hist-item:focus-visible {
  outline: 2px solid var(--selection-border);
  outline-offset: -2px;
}

.hist-item.is-picked {
  background: var(--selection-bg);
}

.hist-text {
  display: -webkit-box;
  margin-bottom: 3px;
  overflow: hidden;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  font-size: 12px;
  line-height: 1.45;
  color: var(--text-soft);
  word-break: break-word;
}

.hist-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  font-size: 10px;
  color: var(--muted);
}

.hist-empty {
  margin: 10px 6px;
  font-size: 11px;
  color: var(--muted);
  text-align: center;
}
</style>
