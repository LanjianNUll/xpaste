<script setup lang="ts">
import { ref, watch } from "vue";
import { Brush, Tools } from "@element-plus/icons-vue";
import ThemeSettings from "@/components/ThemeSettings.vue";
import { hotkeyPlaceholder, modifierHint } from "@/services/platform";

/**
 * 设置面板内容：左侧导航 + 右侧内容区。
 *
 * 布局上左栏固定、右侧独立滚动——弹窗整体限高，滚动的永远只有内容列，
 * 导航不会被卷走。样式是"行式"设置（分组标题 + 分隔线），不再套一层层卡片。
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

      <!-- 通用：快捷键、开机启动、数据管理、软件更新（行式分组） -->
      <div v-else class="settings-group">
        <div class="settings-row">
          <div class="settings-row-main">
            <div class="settings-row-label">快捷键</div>
            <div class="settings-row-desc">
              当前 {{ currentHotkey }}。支持的修饰键：{{ modifierHint }}，按键：A-Z。修改后可能需要重启应用生效。
            </div>
          </div>
          <div class="settings-row-control">
            <el-input
              :model-value="hotkeyDraft"
              :placeholder="hotkeyPlaceholder"
              style="width: 200px"
              @update:model-value="handleHotkeyInput"
            />
          </div>
        </div>

        <div class="settings-row">
          <div class="settings-row-main">
            <div class="settings-row-label">开机启动</div>
            <div class="settings-row-desc">登录系统后自动在后台运行</div>
          </div>
          <div class="settings-row-control">
            <el-switch
              :model-value="autostart"
              @update:model-value="emit('update:autostart', Boolean($event))"
            />
          </div>
        </div>

        <div class="settings-row">
          <div class="settings-row-main">
            <div class="settings-row-label">数据管理</div>
            <div class="settings-row-desc">清空所有剪贴板历史记录，操作不可恢复</div>
          </div>
          <div class="settings-row-control">
            <el-button type="danger" plain @click="emit('clear-data')">清理存储</el-button>
          </div>
        </div>

        <div class="settings-row">
          <div class="settings-row-main">
            <div class="settings-row-label">软件更新</div>
            <div class="settings-row-desc">{{ updateStatusText }}</div>
          </div>
          <div class="settings-row-control">
            <el-button :loading="updateChecking" @click="emit('update-action')">
              {{ updateActionText }}
            </el-button>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>
