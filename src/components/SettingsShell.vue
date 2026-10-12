<script setup lang="ts">
import { ref, watch } from "vue";
import { Brush, Tools } from "@element-plus/icons-vue";
import ThemeSettings from "@/components/ThemeSettings.vue";

/**
 * 设置面板内容：左侧图标导航 + 右侧内容区（布局参照 WorkBuddy 的设置弹窗）。
 *
 * 组件自己持有导航状态与"通用"分组的表单，父组件只负责把外部状态
 * （快捷键、开机启动、更新状态）传进来，并把用户的操作抛出去。
 */
const props = defineProps<{
  /** 设置对话框是否展开，展开时外观页需要重新读取存储 */
  active: boolean;
  /** 当前生效的快捷键 */
  currentHotkey: string;
  /** 开机自启是否开启 */
  autostart: boolean;
  /** 是否正在检查更新 */
  updateChecking: boolean;
  /** 更新状态描述文案 */
  updateStatusText: string;
  /** 更新按钮上的文案：已下载完成时变成"重启并安装" */
  updateActionText: string;
}>();

const emit = defineEmits<{
  (event: "update:currentHotkey", value: string): void;
  (event: "update:autostart", value: boolean): void;
  (event: "clear-data"): void;
  (event: "update-action"): void;
}>();

type NavId = "appearance" | "general";

const navItems: Array<{ id: NavId; label: string; icon: unknown }> = [
  { id: "appearance", label: "外观", icon: Brush },
  { id: "general", label: "通用", icon: Tools }
];

const activeNav = ref<NavId>("appearance");

/** 快捷键输入框的草稿值：用户输入时不立刻污染已保存的快捷键 */
const hotkeyDraft = ref(props.currentHotkey);

/** 弹窗打开时同步一次外部快捷键，保证输入框与实际生效值一致 */
watch(
  () => props.active,
  (visible) => {
    if (visible) syncDraft();
  },
  { immediate: true }
);

function selectNav(id: NavId) {
  activeNav.value = id;
}

function handleHotkeyInput(value: string) {
  hotkeyDraft.value = value;
  emit("update:currentHotkey", value);
}

function syncDraft() {
  hotkeyDraft.value = props.currentHotkey;
}
</script>

<template>
  <div class="settings-shell">
    <nav class="settings-nav">
      <button
        v-for="item in navItems"
        :key="item.id"
        type="button"
        class="settings-nav-item"
        :class="{ 'is-active': activeNav === item.id }"
        @click="selectNav(item.id)"
      >
        <el-icon :size="15"><component :is="item.icon" /></el-icon>
        <span>{{ item.label }}</span>
      </button>
    </nav>

    <section class="settings-content">
      <!-- 外观：主题模式、皮肤、背景动效等，改动即时生效 -->
      <ThemeSettings v-if="activeNav === 'appearance'" :active="props.active" />

      <!-- 通用：快捷键、开机启动、数据管理、软件更新 -->
      <template v-else>
        <div class="settings-card">
          <div class="settings-card-title">快捷键</div>
          <el-input
            :model-value="hotkeyDraft"
            placeholder="例如：Win+V, Ctrl+Shift+V"
            @update:model-value="handleHotkeyInput"
          />
          <div class="settings-tip">
            当前快捷键：<span class="hotkey-value">{{ currentHotkey }}</span>
          </div>
          <div class="settings-tip">
            支持的修饰键：Ctrl、Alt、Shift、Win；按键：A-Z。修改后可能需要重启应用生效。
          </div>
        </div>

        <div class="settings-card">
          <div class="settings-card-title">
            开机启动
            <span class="settings-card-hint">应用随系统自动启动</span>
          </div>
          <el-switch
            :model-value="autostart"
            @update:model-value="emit('update:autostart', Boolean($event))"
          />
        </div>

        <div class="settings-card">
          <div class="settings-card-title">
            数据管理
            <span class="settings-card-hint">清空所有剪贴板历史记录</span>
          </div>
          <el-button type="danger" plain @click="emit('clear-data')">清理存储</el-button>
        </div>

        <div class="settings-card">
          <div class="settings-card-title">
            软件更新
            <span class="settings-card-hint">{{ updateStatusText }}</span>
          </div>
          <el-button :loading="updateChecking" @click="emit('update-action')">
            {{ updateActionText }}
          </el-button>
        </div>
      </template>
    </section>
  </div>
</template>