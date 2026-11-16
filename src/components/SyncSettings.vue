<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { CopyDocument, Refresh, Warning } from "@element-plus/icons-vue";
import {
  disableSync,
  enableSync,
  getStatus,
  loadConfig,
  normalizeCode,
  rotateCode,
  setLanOnly,
  setSyncImages,
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
 * 所以界面上不出现"账号""密码"这类词——用户只需要知道抄走这 6 位数字，
 * 在另一台设备输入进去，两边就开始共享同一份历史。
 */

const config = ref<SyncConfig>(loadConfig());
const status = ref<SyncStatus>(getStatus());
const joinInput = ref("");
const busy = ref(false);

/** 输入时只留数字并自动按 3+3 分组，粘贴带空格的长码也不会被拒 */
watch(joinInput, (value) => {
  const digits = value.replace(/\D/g, "").slice(0, 6);
  const formatted = digits.length > 3 ? `${digits.slice(0, 3)} ${digits.slice(3)}` : digits;
  if (formatted !== value) joinInput.value = formatted;
});

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

/** 6 位数字拆成 3+3 展示，口述和核对都更轻松 */
const formattedCode = computed(() => {
  const code = config.value.code;
  if (!code) return "";
  return `${code.slice(0, 3)} ${code.slice(3)}`;
});

/** 中继地址只在构建时注入，缺了它就是截图里那条"同步异常" */
const relayMissing = computed(() => !config.value.relay);

/** 当前实际走的是哪条通道：手动开了局域网，或者压根没有中继可用 */
const lanMode = computed(() => config.value.lanOnly || !config.value.relay);

const statusText = computed(() => {
  if (!config.value.enabled) return "未启用";
  if (status.value.busy) return "同步中…";
  if (status.value.lastError) return status.value.lastError;

  if (lanMode.value) {
    const peers = status.value.lanPeers;
    if (peers === 0) {
      return "局域网内还没有发现同房间设备：确认两台设备连的是同一个 Wi-Fi，并填入同一个同步码";
    }
    const base = `局域网直连 ${peers} 台设备`;
    if (!status.value.lastSyncAt) return `${base} · 等待首次同步`;
    const time = new Date(status.value.lastSyncAt).toLocaleTimeString();
    return `${base} · 上次同步 ${time} · 收到 ${status.value.lastPulled} 条 · 发出 ${status.value.lastPushed} 条`;
  }

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
    ElMessage.error("同步码应为 6 位数字，例如 123 456");
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
  ElMessage.success(`已换成新同步码 ${code}，旧设备需要用新码重新加入`);
}

/** 局域网开关：打开后不再依赖公网，同 Wi-Fi 设备直连 */
function handleLanToggle(value: boolean) {
  config.value = setLanOnly(value);
  ElMessage.success(
    value ? "已切换为局域网直连，同一 Wi-Fi 下的设备会互相发现" : "已切回中继同步"
  );
}

/** 图片同步开关：默认关闭，打开后回溯 7 天并立刻推一轮 */
function handleImagesToggle(value: boolean) {
  config.value = setSyncImages(value);
  ElMessage.success(
    value ? "已开启图片同步，会补推最近 7 天的图片" : "已关闭图片同步，只同步文字"
  );
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
          用同一个 6 位同步码在几台设备间共享剪贴板历史。内容在本机加密后才上传，
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
            6 位数字，等同于密码：在另一台设备输入即可；泄露后点「换新」重新生成
          </div>
        </div>
        <div class="settings-row-control">
          <code class="sync-code sync-code--short">{{ formattedCode }}</code>
          <el-button :icon="CopyDocument" size="small" @click="handleCopy">复制</el-button>
          <el-button :icon="Refresh" size="small" @click="handleRotate">换新</el-button>
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row-main">
          <div class="settings-row-label">局域网直连</div>
          <div class="settings-row-desc">
            不经过中继，只和同一个 Wi-Fi 下的设备互传。中继连不上时打开它，
            发现设备后每 5 秒自动同步一次
          </div>
        </div>
        <div class="settings-row-control">
          <el-switch
            :model-value="config.lanOnly"
            @update:model-value="handleLanToggle(Boolean($event))"
          />
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row-main">
          <div class="settings-row-label">同步图片</div>
          <div class="settings-row-desc">
            默认只同步文字。开启后图片会压缩到 1280px 再加密上传，流量和等待时间都会明显增加
          </div>
        </div>
        <div class="settings-row-control">
          <el-switch
            :model-value="config.syncImages"
            @update:model-value="handleImagesToggle(Boolean($event))"
          />
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

    <div v-if="relayMissing && !config.lanOnly" class="settings-row">
      <div class="settings-row-main">
        <div class="settings-row-label">同步服务未配置</div>
        <div class="settings-row-desc">
          当前安装包没有注入中继地址，同步无法连接。请使用官方最新安装包，或自建中继后重新构建
        </div>
      </div>
    </div>

    <div class="settings-row">
      <div class="settings-row-main">
        <div class="settings-row-label">在另一台设备加入</div>
        <div class="settings-row-desc">
          在新设备上输入这 6 位数字，它就会和历史合并到同一个房间
        </div>
      </div>
      <div class="settings-row-control">
        <el-input
          v-model="joinInput"
          placeholder="6 位数字，例如 123 456"
          inputmode="numeric"
          style="width: 180px"
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

.sync-code--short {
  max-width: none;
  padding: 6px 14px;
  font-size: 16px;
  font-weight: 600;
  letter-spacing: 2px;
}

.sync-warn {
  margin-left: 6px;
  color: var(--el-color-warning);
  vertical-align: -2px;
}
</style>
