import { invoke } from "@tauri-apps/api/core";
import { isMacOS } from "@/services/platform";

/**
 * 多设备同步引擎。
 *
 * 设计沿用「一个同步码 = 账号 + 密钥」：
 *
 *   同步码 (32 字节主密钥的 base32)
 *     ├─ room_id = HKDF(master, "xpaste-room-v1")     公开，给中继路由用
 *     ├─ key     = HKDF(master, "xpaste-content-v1")  AES-256-GCM 内容密钥，永不出本机
 *     └─ proof   = HKDF(master, "xpaste-auth-v1")     鉴权，中继只存它的 SHA-256
 *
 * 中继只见到 room_id、密文、IV、时间戳。它既解不开内容，也算不出 proof，
 * 所以没法冒充任何一台设备写入。
 *
 * 为什么加密和 HTTP 放在前端而不是 Rust：WebCrypto 与 fetch 是运行时就有的能力，
 * 省掉在 Rust 侧引入 reqwest / aes-gcm 一整条依赖链，加密实现还能和网页版
 * （xpaste-sync）逐字节对齐，方便互测。
 */

const STORAGE_KEY = "xpaste.sync.v1";

/**
 * 默认中继地址。
 *
 * 刻意不写死在源码里：中继若部署在带 sharecode 的托管平台上，那个 sharecode
 * 就是访问凭据，写进公开仓库等于把服务额度送人。构建时通过环境变量注入，
 * 自建中继同样走这条路 —— 换地址不用改代码。
 *
 *   VITE_XPASTE_RELAY="https://relay.example.com/"  npm run build
 *
 * 留空时同步不可用，设置页会提示"中继地址未配置"，不影响其它功能。
 */
export const DEFAULT_RELAY = String(import.meta.env.VITE_XPASTE_RELAY ?? "").trim();

/** 单次搬运条数与体积上限，与服务端限制对齐。 */
const PUSH_BATCH = 50;
const PULL_BATCH_TIMEOUT_MS = 15000;

/** 首次开启同步时只回推最近 7 天，避免把几年的历史一次性灌进中继。 */
const INITIAL_BACKFILL_MS = 7 * 24 * 3600 * 1000;

export interface SyncConfig {
  enabled: boolean;
  /** 同步码，形如 XP1-XXXX-XXXX-…（空字符串表示还没建过房间） */
  code: string;
  /** 中继地址，可以带 ?sharecode= */
  relay: string;
  /** 已经拉到的中继序号 */
  cursor: number;
  /** 已经推上去的本地时间戳水位（created_at） */
  pushedUpTo: number;
  deviceName: string;
  lastSyncAt: number;
  lastError: string;
  lastPulled: number;
  lastPushed: number;
}

export interface SyncStatus {
  running: boolean;
  busy: boolean;
  lastSyncAt: number;
  lastPulled: number;
  lastPushed: number;
  lastError: string;
}

interface Keys {
  roomId: string;
  key: CryptoKey;
  proof: string;
}

interface ExportedItem {
  id: number;
  format: string;
  category: string;
  text: string | null;
  html: string | null;
  color: string | null;
  filePath: string | null;
  createdAt: number;
  copyCount: number;
}

interface RemoteRecord {
  seq: number;
  id: string;
  iv: string;
  ct: string;
  ts: number;
}

const enc = new TextEncoder();
const dec = new TextDecoder();

const B32_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

// ── 配置持久化 ────────────────────────────────────────────────

function defaultConfig(): SyncConfig {
  return {
    enabled: false,
    code: "",
    relay: DEFAULT_RELAY,
    cursor: 0,
    pushedUpTo: 0,
    deviceName: isMacOS ? "Mac" : "PC",
    lastSyncAt: 0,
    lastError: "",
    lastPulled: 0,
    lastPushed: 0
  };
}

export function loadConfig(): SyncConfig {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return defaultConfig();
    const parsed = JSON.parse(raw) as Partial<SyncConfig>;
    return { ...defaultConfig(), ...parsed };
  } catch {
    // localStorage 不可用时退化为"未启用"，不影响应用其他功能
    return defaultConfig();
  }
}

export function saveConfig(patch: Partial<SyncConfig>): SyncConfig {
  const next = { ...loadConfig(), ...patch };
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    // 写不进去就只在本次会话生效
  }
  window.dispatchEvent(new CustomEvent("xpaste:sync-config", { detail: next }));
  return next;
}

export function subscribeConfig(listener: (config: SyncConfig) => void): () => void {
  const handler = (event: Event) => listener((event as CustomEvent<SyncConfig>).detail);
  window.addEventListener("xpaste:sync-config", handler);
  return () => window.removeEventListener("xpaste:sync-config", handler);
}

// ── 同步码 ────────────────────────────────────────────────────

function base32Encode(bytes: Uint8Array): string {
  let bits = 0;
  let value = 0;
  let out = "";
  for (const byte of bytes) {
    value = (value << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      out += B32_ALPHABET[(value >>> (bits - 5)) & 31];
      bits -= 5;
    }
  }
  if (bits > 0) {
    out += B32_ALPHABET[(value << (5 - bits)) & 31];
  }
  return out;
}

function base32Decode(text: string): Uint8Array {
  let bits = 0;
  let value = 0;
  const out: number[] = [];
  for (const char of text) {
    const index = B32_ALPHABET.indexOf(char);
    if (index < 0) continue;
    value = (value << 5) | index;
    bits += 5;
    if (bits >= 8) {
      out.push((value >>> (bits - 8)) & 0xff);
      bits -= 8;
    }
  }
  return new Uint8Array(out);
}

/** 生成新的同步码：32 字节随机 → base32 → 按 4 位分组，方便人眼抄写。 */
export function generateCode(): string {
  const master = crypto.getRandomValues(new Uint8Array(32));
  const raw = base32Encode(master);
  const groups = raw.match(/.{1,4}/g) ?? [];
  return `XP1-${groups.join("-")}`;
}

/** 归一化用户粘贴的同步码；格式不对返回 null。 */
export function normalizeCode(input: string): string | null {
  const cleaned = input
    .toUpperCase()
    .replace(/^XP1-?/, "")
    .replace(/[^A-Z2-7]/g, "");
  if (cleaned.length === 0) return null;
  const bytes = base32Decode(cleaned);
  if (bytes.length !== 32) return null;
  const groups = cleaned.match(/.{1,4}/g) ?? [];
  return `XP1-${groups.join("-")}`;
}

// ── 密钥推导与加解密 ──────────────────────────────────────────

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function toBase64(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return window.btoa(binary);
}

function fromBase64(text: string): Uint8Array {
  const binary = window.atob(text);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) out[i] = binary.charCodeAt(i);
  return out;
}

async function deriveKeys(code: string): Promise<Keys> {
  const cleaned = code.toUpperCase().replace(/^XP1-?/, "").replace(/[^A-Z2-7]/g, "");
  const master = base32Decode(cleaned);
  if (master.length !== 32) {
    throw new Error("同步码无效");
  }

  const base = await crypto.subtle.importKey("raw", master, "HKDF", false, [
    "deriveBits",
    "deriveKey"
  ]);

  const roomBits = new Uint8Array(
    await crypto.subtle.deriveBits(
      {
        name: "HKDF",
        hash: "SHA-256",
        salt: enc.encode("xpaste-room-v1"),
        info: new Uint8Array(0)
      },
      base,
      128
    )
  );
  const roomId = toHex(roomBits);

  const contentKey = await crypto.subtle.deriveKey(
    {
      name: "HKDF",
      hash: "SHA-256",
      salt: roomBits,
      info: enc.encode("xpaste-content-v1")
    },
    base,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"]
  );

  const proof = toHex(
    new Uint8Array(
      await crypto.subtle.deriveBits(
        {
          name: "HKDF",
          hash: "SHA-256",
          salt: enc.encode("xpaste-auth-v1"),
          info: enc.encode(roomId)
        },
        base,
        256
      )
    )
  );

  return { roomId, key: contentKey, proof };
}

async function sealItem(keys: Keys, item: ExportedItem, deviceName: string) {
  const id = crypto.randomUUID();
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const payload = {
    v: 1,
    format: item.format,
    category: item.category,
    text: item.text,
    html: item.html,
    color: item.color,
    filePath: item.filePath,
    createdAt: item.createdAt,
    copyCount: item.copyCount,
    device: deviceName
  };
  const ct = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv, additionalData: enc.encode(`${keys.roomId}|${id}`) },
    keys.key,
    enc.encode(JSON.stringify(payload))
  );
  return {
    id,
    kind: "text",
    iv: toBase64(iv),
    ct: toBase64(new Uint8Array(ct)),
    ts: Date.now()
  };
}

async function openRecord(keys: Keys, record: RemoteRecord) {
  const iv = fromBase64(record.iv);
  const plain = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv, additionalData: enc.encode(`${keys.roomId}|${record.id}`) },
    keys.key,
    fromBase64(record.ct)
  );
  const payload = JSON.parse(dec.decode(new Uint8Array(plain))) as {
    format?: string;
    category?: string;
    text?: string | null;
    html?: string | null;
    color?: string | null;
    filePath?: string | null;
    createdAt?: number;
    copyCount?: number;
  };
  return {
    format: payload.format ?? "text",
    category: payload.category ?? "text",
    text: payload.text ?? null,
    html: payload.html ?? null,
    color: payload.color ?? null,
    filePath: payload.filePath ?? null,
    createdAt: payload.createdAt ?? record.ts ?? Date.now(),
    copyCount: payload.copyCount ?? 1
  };
}

// ── 传输 ──────────────────────────────────────────────────────

/**
 * 拼中继地址。
 *
 * 中继部署在托管平台上时要带 `?sharecode=`，而且**每个**请求都得带，
 * 不能只在首页带一次。这里把 sharecode 从配置里拆出来单独拼到每个
 * 请求上，base 本身可以是带 query 的完整链接。
 */
function endpoint(relay: string, path: string): string {
  const base = new URL(relay);
  const share = base.searchParams.get("sharecode");
  let url = base.origin + path;
  if (share) {
    url += (path.includes("?") ? "&" : "?") + "sharecode=" + encodeURIComponent(share);
  }
  return url;
}

async function request(
  relay: string,
  path: string,
  init: { method: string; proof: string; body?: string }
): Promise<any> {
  const controller = new AbortController();
  const timer = window.setTimeout(() => controller.abort(), PULL_BATCH_TIMEOUT_MS);
  try {
    const response = await fetch(endpoint(relay, path), {
      method: init.method,
      body: init.body,
      signal: controller.signal,
      headers: {
        "content-type": "application/json",
        "x-xp-proof": init.proof
      }
    });
    const text = await response.text();
    let body: any = null;
    try {
      body = text ? JSON.parse(text) : null;
    } catch {
      body = null;
    }
    if (!response.ok) {
      const reason = body?.error ?? `HTTP ${response.status}`;
      throw new Error(
        response.status === 403 ? "同步码与房间不匹配" : `中继返回：${reason}`
      );
    }
    return body;
  } finally {
    window.clearTimeout(timer);
  }
}

// ── 同步主流程 ────────────────────────────────────────────────

let timer: number | null = null;
let busy = false;
let status: SyncStatus = {
  running: false,
  busy: false,
  lastSyncAt: 0,
  lastPulled: 0,
  lastPushed: 0,
  lastError: ""
};

function setStatus(patch: Partial<SyncStatus>) {
  status = { ...status, ...patch };
  window.dispatchEvent(new CustomEvent("xpaste:sync-status", { detail: status }));
}

export function getStatus(): SyncStatus {
  return status;
}

export function subscribeStatus(listener: (value: SyncStatus) => void): () => void {
  const handler = (event: Event) => listener((event as CustomEvent<SyncStatus>).detail);
  window.addEventListener("xpaste:sync-status", handler);
  return () => window.removeEventListener("xpaste:sync-status", handler);
}

async function pushOnce(config: SyncConfig, keys: Keys): Promise<number> {
  const batch = await invoke<ExportedItem[]>("sync_export_items", {
    sinceTs: config.pushedUpTo,
    limit: PUSH_BATCH
  });
  if (batch.length === 0) return 0;

  const items = [];
  for (const item of batch) {
    // 图片是 RGBA 原始字节，一条动辄几 MB，第一版同步不搬图片。
    if (item.format === "image") continue;
    items.push(await sealItem(keys, item, config.deviceName));
  }

  // 水位永远推进到本批最后一条，哪怕这批全是图片。
  // 否则遇到"连续 50 条图片"就会每 5 秒重复导出同一批，后面的文本永远推不上去。
  const watermark = batch[batch.length - 1].createdAt;
  saveConfig({ pushedUpTo: watermark });

  if (items.length === 0) return 0;

  await request(config.relay, "/api/push", {
    method: "POST",
    proof: keys.proof,
    body: JSON.stringify({ room: keys.roomId, items })
  });

  return items.length;
}

async function pullOnce(config: SyncConfig, keys: Keys): Promise<number> {
  const result = await request(
    config.relay,
    `/api/pull?room=${encodeURIComponent(keys.roomId)}&since=${config.cursor}`,
    { method: "GET", proof: keys.proof }
  );

  const records: RemoteRecord[] = Array.isArray(result?.items) ? result.items : [];
  if (records.length > 0) {
    const incoming = [];
    for (const record of records) {
      try {
        incoming.push(await openRecord(keys, record));
      } catch {
        // 单条解不开（多半是别的版本写的）就跳过，不能让一条坏记录卡住整轮同步
      }
    }
    if (incoming.length > 0) {
      await invoke<number>("sync_import_items", { items: incoming });
    }
  }

  const nextCursor = Number(result?.seq ?? config.cursor) || config.cursor;
  saveConfig({ cursor: nextCursor });
  return records.length;
}

/** 跑一轮完整同步：先推本地新增，再拉对端更新。 */
export async function syncNow(): Promise<SyncStatus> {
  const config = loadConfig();
  if (!config.enabled || !config.code) {
    return status;
  }
  // 没配中继时不要空转：直接给出可执行的提示，比 fetch 一个空 URL 报错清楚
  if (!config.relay) {
    const message = "同步中继地址未配置（构建时未设置 VITE_XPASTE_RELAY）";
    saveConfig({ lastError: message });
    setStatus({ busy: false, lastError: message });
    return status;
  }
  if (busy) return status;

  busy = true;
  setStatus({ busy: true });
  try {
    const keys = await deriveKeys(config.code);
    await request(config.relay, "/api/room", {
      method: "POST",
      proof: keys.proof,
      body: JSON.stringify({ room: keys.roomId })
    });

    let pushed = 0;
    // 一次最多推 3 批，避免首次启用时把界面卡在长时间的网络往返里
    for (let round = 0; round < 3; round += 1) {
      const count = await pushOnce(loadConfig(), keys);
      pushed += count;
      if (count === 0) break;
    }
    const pulled = await pullOnce(loadConfig(), keys);

    const now = Date.now();
    saveConfig({ lastSyncAt: now, lastError: "", lastPulled: pulled, lastPushed: pushed });
    setStatus({
      busy: false,
      lastSyncAt: now,
      lastPulled: pulled,
      lastPushed: pushed,
      lastError: ""
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    saveConfig({ lastError: message });
    setStatus({ busy: false, lastError: message });
  } finally {
    busy = false;
  }
  return status;
}

/** 启用同步：没有同步码时先生成一个，然后立刻同步一次。 */
export async function enableSync(code?: string): Promise<void> {
  const current = loadConfig();
  const nextCode = code ?? current.code ?? "";
  const normalized = nextCode ? normalizeCode(nextCode) : generateCode();
  if (!normalized) {
    throw new Error("同步码格式无效");
  }

  const isNewRoom = !current.code || current.code !== normalized;
  saveConfig({
    enabled: true,
    code: normalized,
    // 换房间等于换身份：游标和水位必须一起归零，否则会把旧房间的进度套到新房间上
    cursor: isNewRoom ? 0 : current.cursor,
    pushedUpTo: isNewRoom ? Date.now() - INITIAL_BACKFILL_MS : current.pushedUpTo,
    lastError: ""
  });
  startSync();
  void syncNow();
}

export function disableSync(): void {
  stopSync();
  saveConfig({ enabled: false, lastError: "" });
}

/** 换一个全新同步码：旧房间的数据不会跟过来，等于换了身份。 */
export function rotateCode(): string {
  const code = generateCode();
  saveConfig({ code, cursor: 0, pushedUpTo: Date.now() - INITIAL_BACKFILL_MS, lastError: "" });
  if (loadConfig().enabled) {
    startSync();
    void syncNow();
  }
  return code;
}

const POLL_INTERVAL_MS = 5000;

export function startSync(): void {
  const config = loadConfig();
  if (!config.enabled || !config.code) {
    stopSync();
    return;
  }
  // 中继没配就别起轮询，省得每 5 秒白跑一次
  if (!config.relay) {
    stopSync();
    setStatus({ lastError: "同步中继地址未配置（构建时未设置 VITE_XPASTE_RELAY）" });
    return;
  }
  setStatus({ running: true });
  if (timer != null) return;
  timer = window.setInterval(() => {
    void syncNow();
  }, POLL_INTERVAL_MS);
  void syncNow();
}

export function stopSync(): void {
  if (timer != null) {
    window.clearInterval(timer);
    timer = null;
  }
  setStatus({ running: false, busy: false });
}
