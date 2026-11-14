<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { ElMessage } from "element-plus";
import { CopyDocument, Refresh, Warning } from "@element-plus/icons-vue";
import {
  disableSync,
  enableSync,
  getStatus,
  loadConfig,
  normalizeCode,
  rotateCode,
  subscribeConfig,
  subscribeStatus,
  syncNow,
  type SyncConfig,
  type SyncStatus
} from "@/services/sync";

/**
 * 多设备同步面板。
 *
 * 交互只做三件事：开关、看/抄同步码、手动同步。同步码本身就是账号+密钥，
 * 所以界面上不出现"账号""密码"这类词——用户只需要知道抄走这串码，
 * 在另一台设备粘贴进去，两边就开始共享同一份历史。
 */

const config = ref<SyncConfig>(loadConfig());
const status = ref<SyncStatus>(getStatus());
const joinInput = ref("");
const busy = ref(false);
const revealing = ref(false);

let unsubscribeStatus: (() => void) | null = null;
let unsubscribeConfig: (() => void) | null = null;

onMounted(() => {
  unsubscribeStatus = subscribeStatus((value) => {
    status.value = value;
  });
  unsubscribeConfig = subscribeConfig((value) => {
    config.value = value;
  });
});

onBeforeUnmount(() => {
  unsubscribeStatus?.();
  unsubscribeConfig?.();
});

/** 同步码比较长，默认只露前 9 位，避免截屏时整串泄露 */
const maskedCode = computed(() => {
  const code = config.value.code;
  if (!code) return "";
  if (revealing.value) return code;
  return `${code.slice(0, 9)}••••••••••••`;
});

const statusText = computed(() => {
  if (!config.value.enabled) return "未启用";
  if (status.value.busy) return "同步中…";
  if (status.value.lastError) return status.value.lastError;
  if (!status.value.lastSyncAt) return "等待首次同步";
  const time = new Date(status.value.lastSyncAt).toLocaleTimeString();
  return `上次同步 ${time} · 收到 ${status.value.lastPulled} 条 · 发出 ${status.value.lastPushed} 条`;
});

async function handleToggle(value: boolean) {
  try {
    if (value) {
      await enableSync();
      config.value = loadConfig();
      ElMessage.success("已开启同步，正在和另一端对齐");
    } else {
      disableSync();
      config.value = loadConfig();
    }
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : "开启失败");
  }
}

async function handleJoin() {
  const normalized = normalizeCode(joinInput.value);
  if (!normalized) {
    ElMessage.error("同步码格式不正确，应为 XP1- 开头的一串字符");
    return;
  }
  busy.value = true;
  try {
    await enableSync(normalized);
    config.value = loadConfig();
    joinInput.value = "";
    ElMessage.success("已加入，另一端的历史会陆续同步过来");
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : "加入失败");
  } finally {
    busy.value = false;
  }
}

async function handleCopy() {
  if (!config.value.code) return;
  try {
    await navigator.clipboard.writeText(config.value.code);
    ElMessage.success("同步码已复制，别发到公开的地方");
  } catch {
    ElMessage.error("复制失败，请手动选中复制");
  }
}

function handleRotate() {
  const code = rotateCode();
  config.value = loadConfig();
  revealing.value = true;
  ElMessage.success("已生成新的同步码，旧设备需要用新码重新加入");
  return code;
}

async function handleSyncNow() {
  busy.value = true;
  try {
    await syncNow();
    config.value = loadConfig();
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="settings-group">
    <div class="settings-row">
      <div class="settings-row-main">
        <div class="settings-row-label">多设备同步</div>
        <div class="settings-row-desc">
          用同一串同步码在几台设备间共享剪贴板历史。内容在本机加密后才上传，
          中继服务看到的只有密文
        </div>
      </div>
      <div class="settings-row-control">
        <el-switch :model-value="config.enabled" @update:model-value="handleToggle(Boolean($event))" />
      </div>
    </div>

    <template v-if="config.enabled">
      <div class="settings-row">
        <div class="settings-row-main">
          <div class="settings-row-label">同步码</div>
          <div class="settings-row-desc">
            这就是你的身份，等同于密码。换设备时把它抄过去即可，丢了只能换新的
          </div>
        </div>
        <div class="settings-row-control">
          <el-button size="small" text @click="revealing = !revealing">
            {{ revealing ? "隐藏" : "显示" }}
          </el-button>
          <code class="sync-code">{{ maskedCode }}</code>
          <el-button :icon="CopyDocument" size="small" @click="handleCopy">复制</el-button>
          <el-button :icon="Refresh" size="small" @click="handleRotate">换新</el-button>
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row-main">
          <div class="settings-row-label">同步状态</div>
          <div class="settings-row-desc">
            {{ statusText }}
            <template v-if="status.lastError">
              <el-icon class="sync-warn"><Warning /></el-icon>
            </template>
          </div>
        </div>
        <div class="settings-row-control">
          <el-button :loading="busy || status.busy" @click="handleSyncNow">立即同步</el-button>
        </div>
      </div>
    </template>

    <div class="settings-row">
      <div class="settings-row-main">
        <div class="settings-row-label">在另一台设备加入</div>
        <div class="settings-row-desc">
          在新设备上粘贴已有的同步码，它就会和历史合并到同一个房间
        </div>
      </div>
      <div class="settings-row-control">
        <el-input
          v-model="joinInput"
          placeholder="XP1-XXXX-XXXX-…"
          style="width: 260px"
          @keyup.enter="handleJoin"
        />
        <el-button type="primary" :loading="busy" @click="handleJoin">加入</el-button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.sync-code {
  display: inline-block;
  max-width: 260px;
  overflow: hidden;
  text-overflow: ellipsis;
  padding: 4px 10px;
  border-radius: 6px;
  background: var(--el-fill-color-light);
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 12px;
  letter-spacing: 0.4px;
  vertical-align: middle;
}

.sync-warn {
  margin-left: 6px;
  color: var(--el-color-warning);
  vertical-align: -2px;
}
</style>
