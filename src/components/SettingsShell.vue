<script setup lang="ts">
import { ref, watch } from "vue";
import { Brush, Connection, Tools } from "@element-plus/icons-vue";
import ThemeSettings from "@/components/ThemeSettings.vue";
import SyncSettings from "@/components/SyncSettings.vue";
import { hotkeyPlaceholder, isMacOS, modifierHint } from "@/services/platform";

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
  /** Dock 图标是否可见（仅 macOS 有意义） */
  dockVisible: boolean;
  /** 是否已获得「辅助功能」权限（仅 macOS 有意义） */
  accessibilityGranted: boolean;
  /** 是否正在执行「一键修复」 */
  accessibilityRepairing: boolean;
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
  (event: "update:dockVisible", value: boolean): void;
  (event: "request-accessibility"): void;
  (event: "repair-accessibility"): void;
  (event: "refresh-accessibility"): void;
  (event: "clear-data"): void;
  (event: "update-action"): void;
}>();

type NavId = "appearance" | "sync" | "general";

const navItems: Array<{ id: NavId; label: string; icon: unknown }> = [
  { id: "appearance", label: "外观", icon: Brush },
  { id: "sync", label: "同步", icon: Connection },
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

      <!-- 同步：多设备共享剪贴板历史，端到端加密 -->
      <SyncSettings v-else-if="activeNav === 'sync'" />

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

        <!-- Dock 图标只在 macOS 上存在；Windows 侧对应的是 skipTaskbar，已在配置里关掉 -->
        <div v-if="isMacOS" class="settings-row">
          <div class="settings-row-main">
            <div class="settings-row-label">显示 Dock 图标</div>
            <div class="settings-row-desc">
              关闭后只在菜单栏保留图标，入口仍可用：左键显示主窗口，右键出菜单
            </div>
          </div>
          <div class="settings-row-control">
            <el-switch
              :model-value="dockVisible"
              @update:model-value="emit('update:dockVisible', Boolean($event))"
            />
          </div>
        </div>

        <!-- 辅助功能权限：没有它，合成 ⌘V 会被系统静默丢弃，表现为"选中没反应" -->
        <div v-if="isMacOS" class="settings-row">
          <div class="settings-row-main">
            <div class="settings-row-label">
              自动粘贴权限
              <el-tag
                size="small"
                :type="accessibilityGranted ? 'success' : 'warning'"
                effect="light"
                style="margin-left: 8px; vertical-align: middle"
              >
                {{ accessibilityGranted ? "已授权" : "未授权" }}
              </el-tag>
            </div>
            <div class="settings-row-desc">
              <template v-if="accessibilityGranted">
                已获得「辅助功能」权限，选中条目后会自动粘贴到当前输入框
              </template>
              <template v-else>
                未获得「辅助功能」权限，目前只能复制到剪贴板、需要手动 ⌘V。点「一键修复」会先清掉系统里那条绑着旧签名的失效记录，再用当前版本重新申请
              </template>
            </div>
          </div>
          <div class="settings-row-control">
            <el-button v-if="accessibilityGranted" plain @click="emit('refresh-accessibility')">
              重新检测
            </el-button>
            <el-button
              v-else
              type="primary"
              :loading="accessibilityRepairing"
              @click="emit('repair-accessibility')"
            >
              一键修复
            </el-button>
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
