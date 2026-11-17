import { invoke } from "@tauri-apps/api/core";
import { isMacOS } from "@/services/platform";

/**
 * 多设备同步引擎。
 *
 * 设计沿用「一个同步码 = 账号 + 密钥」：
 *
 *   同步码 (6 位数字)
 *     └─ master  = PBKDF2("xpaste-code-v2:<code>", 12 万次)
 *          ├─ room_id = HKDF(master, "xpaste-room-v1")     公开，给中继路由用
 *          ├─ key     = HKDF(master, "xpaste-content-v1")  AES-256-GCM 内容密钥，永不出本机
 *          └─ proof   = HKDF(master, "xpaste-auth-v1")     鉴权，中继只存它的 SHA-256
 *
 * 中继只见到 room_id、密文、IV、时间戳。它既解不开内容，也算不出 proof，
 * 所以没法冒充任何一台设备写入。
 *
 * 6 位数字是为了方便在设备间口述/手输；代价是密钥空间只有 100 万，
 * 因此用 PBKDF2 把每次猜测拖慢，并且同步码等同于密码，不要公开。
 *
 * 为什么加密和 HTTP 放在前端而不是 Rust：WebCrypto 与 fetch 是运行时就有的能力，
 * 省掉在 Rust 侧引入 reqwest / aes-gcm 一整条依赖链，加密实现还能和网页版
 * （xpaste-sync）逐字节对齐，方便互测。
 */

const STORAGE_KEY = "xpaste.sync.v1";

/** 同步码长度：6 位数字，口述和手输都不费劲。 */
export const CODE_LENGTH = 6;

/** 6 位数字空间小，靠 PBKDF2 迭代把每次猜测拖慢到几十毫秒。 */
const CODE_ITERATIONS = 120_000;

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
/** 拉取是纯文本查询，慢一点就该放弃；推送带图片时要留足上传时间。 */
const PULL_BATCH_TIMEOUT_MS = 15000;
const PUSH_BATCH_TIMEOUT_MS = 45000;
/** 单次推送的密文体积上限：超出就拆到下一轮，避免中继直接拒绝一个大请求。 */
const PUSH_MAX_BYTES = 8 * 1024 * 1024;

/** 首次开启同步时只回推最近 7 天，避免把几年的历史一次性灌进中继。 */
const INITIAL_BACKFILL_MS = 7 * 24 * 3600 * 1000;

/**
 * 同步方向。
 *
 * - `both`：双向，默认
 * - `send`：只把本机内容发出去，不接收对端
 * - `receive`：只接收对端内容，不把本机内容发出去
 *
 * 两种单向模式都**不推进另一侧的水位**：只收时 `pushedUpTo` 原地不动，
 * 切回双向会把这段时间攒下的本地内容补发出去；只发时 `cursor` 不动，
 * 切回双向会补拉这段时间对端产生的内容。不会悄悄丢东西。
 */
export type SyncMode = "both" | "send" | "receive";

export interface SyncConfig {
  enabled: boolean;
  /** 同步码：6 位数字（空字符串表示还没建过房间） */
  code: string;
  /** 同步方向，默认双向 */
  mode: SyncMode;
  /** 中继地址，可以带 ?sharecode= */
  relay: string;
  /**
   * 只在局域网内直连，不走中继。
   *
   * 中继要过公网，偶尔会被网络环境拦掉（WebView 里只报一句 Load failed）。
   * 打开这个开关后两台同 Wi-Fi 的设备直接互传：发现用 UDP 组播，传输用 TCP，
   * 内容依旧在本机加密后才发出去。
   */
  lanOnly: boolean;
  /**
   * 手动登记的对端地址（`ip:port`）。
   *
   * 自动发现靠 UDP 广播，跨网段、开了 AP 隔离的 Wi-Fi、公司 VLAN 里都可能
   * 一个包都收不到。这些地址走纯 TCP 直连，不依赖广播——是"一定要同步上"的保底路径。
   */
  manualPeers: string[];
  /** 已经拉到的中继序号 */
  cursor: number;
  /** 已经推上去的本地时间戳水位（created_at） */
  pushedUpTo: number;
  /**
   * 局域网通道自己的发送水位。
   *
   * 为什么要和中继分开：两条通道的可用性相互独立——A 走局域网、B 走中继时，
   * 共用一个水位就会出现"谁的通道先成功谁把水位推走，另一条通道一条也发不出去"，
   * 结果是两边都以为自己发了、对方却什么都没收到。分开记之后任一通道通了内容就能到，
   * 重复投递由入库时的内容指纹去重（`INSERT OR IGNORE`）。
   */
  lanPushedUpTo: number;
  deviceName: string;
  /** 是否连图片一起同步：默认关闭，图片压缩后才上传，体积仍然明显大于文字 */
  syncImages: boolean;
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
  /** 局域网里发现了几个同房间设备（0 表示没有对端，或当前走的是中继） */
  lanPeers: number;
  /** 邻居设备名，用来在状态行里说清"连上的是谁" */
  lanPeerNames: string[];
  /**
   * 探不通的邻居说明：发现靠广播、传输走 TCP，两者可能一个通一个不通，
   * 界面上必须把"看得见但传不过去"单独说出来。
   */
  lanUnreachable: string[];
  /** 这一轮收到但解不开的条数：不为 0 通常意味着两台同步码其实不一致 */
  skipped: number;
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
  /** 图片的 PNG Base64：只有开启「同步图片」时后端才会填充 */
  image?: string | null;
  createdAt: number;
  copyCount: number;
}

/** 两条通道共用的密文记录形状：局域网直连只是不带 `seq`。 */
interface SealedRecord {
  id: string;
  iv: string;
  ct: string;
  ts: number;
}

interface RemoteRecord extends SealedRecord {
  seq: number;
}

/** 局域网通道的记录形态：与中继记录同构，只是没有 `seq`（直连不需要游标）。 */
interface LanRecord {
  id: string;
  kind: string;
  iv: string;
  ct: string;
  ts: number;
}

interface LanPeer {
  addr: string;
  device: string;
  lastSeenMs: number;
  /** 用户手填的地址：不随广播超时被清掉 */
  manual?: boolean;
}

/** 一键体检的返回结构，字段与 Rust `LanDiagnosis` 一一对应 */
export interface LanDiagnosis {
  started: boolean;
  roomConfigured: boolean;
  device: string;
  servicePort: number;
  discoveryPort: number;
  localAddrs: string[];
  lastError: string;
  peers: LanProbe[];
  inbox: number;
}

/** 手动添加设备的结果：`message` 已经是给用户看的话术 */
export interface AddPeerResult {
  addr: string;
  ok: boolean;
  message: string;
}

const enc = new TextEncoder();
const dec = new TextDecoder();

const B32_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

// ── 配置持久化 ────────────────────────────────────────────────

function defaultConfig(): SyncConfig {
  return {
    enabled: false,
    code: "",
    mode: "both",
    relay: DEFAULT_RELAY,
    lanOnly: false,
    manualPeers: [],
    cursor: 0,
    pushedUpTo: 0,
    lanPushedUpTo: 0,
    deviceName: isMacOS ? "Mac" : "PC",
    syncImages: false,
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
    const merged = { ...defaultConfig(), ...parsed };

    // 旧版本可能把空中继存进了配置；新安装包已注入默认地址时就补上，
    // 否则升级后仍会一直提示"同步服务未配置"。
    if (!merged.relay && DEFAULT_RELAY) {
      merged.relay = DEFAULT_RELAY;
    }
    // 手填地址可能是旧版本/手工改过的脏数据，非数组就当空
    if (!Array.isArray(merged.manualPeers)) {
      merged.manualPeers = [];
    }
    // 老配置没有独立水位：从"已经推过的位置"接着走，别把几年历史一次性灌出去
    if (typeof parsed.lanPushedUpTo !== "number" || !parsed.lanPushedUpTo) {
      merged.lanPushedUpTo = merged.pushedUpTo || Date.now() - INITIAL_BACKFILL_MS;
    }

    const normalized = merged.code ? normalizeCode(merged.code) : "";
    if (merged.code && !normalized) {
      // 配置里的码既不是 6 位数字也不是旧版长码：当作没建过房间，让用户重新加入
      merged.code = "";
      merged.enabled = false;
      merged.cursor = 0;
    } else if (normalized && normalized !== merged.code) {
      // 旧版长码平滑迁移成 6 位数字：房间随之改变，游标与水位必须归零重来
      merged.code = normalized;
      merged.cursor = 0;
      merged.pushedUpTo = Date.now() - INITIAL_BACKFILL_MS;
      try {
        window.localStorage.setItem(STORAGE_KEY, JSON.stringify(merged));
      } catch {
        // 写不进去就只在本次会话使用迁移后的值
      }
    }
    return merged;
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

/** FNV-1a：旧版长码迁移成 6 位数字时用的稳定哈希，两台设备算出的结果一致。 */
function fnv1a(bytes: Uint8Array): number {
  let hash = 0x811c9dc5;
  for (const byte of bytes) {
    hash ^= byte;
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash;
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

/** 生成新的同步码：6 位数字，带前导零（000123 也是合法同步码）。 */
export function generateCode(): string {
  const buffer = crypto.getRandomValues(new Uint32Array(1));
  return String(buffer[0] % 10 ** CODE_LENGTH).padStart(CODE_LENGTH, "0");
}

/**
 * 归一化用户输入的同步码；格式不对返回 null。
 *
 * 兼容旧版 XP1- 长码：按原主密钥算出 6 位数字，这样两台老设备升级后
 * 即使各自迁移，也仍然落在同一个房间。
 */
export function normalizeCode(input: string): string | null {
  const trimmed = input.trim();
  if (!trimmed) return null;

  const legacyBody = trimmed.replace(/^XP1-?/i, "").replace(/[^A-Za-z2-7]/g, "");
  const looksLegacy = /^XP1/i.test(trimmed) || (/[A-Za-z]/.test(trimmed) && legacyBody.length >= 16);
  if (looksLegacy) {
    const master = base32Decode(legacyBody.toUpperCase());
    if (master.length !== 32) return null;
    return String(fnv1a(master) % 10 ** CODE_LENGTH).padStart(CODE_LENGTH, "0");
  }

  // 手输时允许带空格 / 连字符，例如「123 456」「123-456」
  const digits = trimmed.replace(/\D/g, "");
  return digits.length === CODE_LENGTH ? digits : null;
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

/**
 * 密钥派生结果缓存：PBKDF2 迭代 12 万次，每次同步都重算会明显卡顿，
 * 而同步码在一次会话里不会变，缓存命中后只是一次 Map 查询。
 */
const keyCache = new Map<string, Promise<Keys>>();

function deriveKeys(code: string): Promise<Keys> {
  const cached = keyCache.get(code);
  if (cached) return cached;
  const task = deriveKeysUncached(code);
  keyCache.set(code, task);
  // 派生失败不留下坏缓存，下次还能重试
  task.catch(() => keyCache.delete(code));
  return task;
}

async function deriveKeysUncached(code: string): Promise<Keys> {
  if (!/^\d{6}$/.test(code)) {
    throw new Error("同步码无效，应为 6 位数字");
  }

  // 6 位数字空间有限：先 PBKDF2 拉伸，再交给 HKDF 分出房间号、内容密钥与鉴权凭证
  const material = await crypto.subtle.importKey(
    "raw",
    enc.encode(`xpaste-code-v2:${code}`),
    "PBKDF2",
    false,
    ["deriveBits"]
  );
  const master = new Uint8Array(
    await crypto.subtle.deriveBits(
      {
        name: "PBKDF2",
        hash: "SHA-256",
        salt: enc.encode("xpaste-salt-v2"),
        iterations: CODE_ITERATIONS
      },
      material,
      256
    )
  );

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
    image: item.image ?? null,
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

async function openRecord(keys: Keys, record: SealedRecord) {
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
    image?: string | null;
    createdAt?: number;
    copyCount?: number;
    device?: string | null;
  };
  return {
    format: payload.format ?? "text",
    category: payload.category ?? "text",
    text: payload.text ?? null,
    html: payload.html ?? null,
    color: payload.color ?? null,
    filePath: payload.filePath ?? null,
    image: payload.image ?? null,
    createdAt: payload.createdAt ?? record.ts ?? Date.now(),
    copyCount: payload.copyCount ?? 1,
    // 来源设备名参与加密，落库后列表上就能标出"这条来自哪台电脑"
    device: payload.device ?? null
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
  init: { method: string; proof: string; body?: string; timeout?: number }
): Promise<any> {
  const controller = new AbortController();
  const timer = window.setTimeout(
    () => controller.abort(),
    init.timeout ?? PULL_BATCH_TIMEOUT_MS
  );
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
  lastError: "",
  lanPeers: 0,
  lanPeerNames: [],
  lanUnreachable: [],
  skipped: 0
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

/**
 * 导出并加密一批本地新增内容。
 *
 * 中继和局域网两条通道都从这里取数，差别只在怎么发出去、以及从哪个水位开始；
 * `watermark` 是这批里最后一条的时间戳，真正送达之后由调用方推进对应的水位。
 */
async function collectSealed(
  config: SyncConfig,
  keys: Keys,
  since: number
): Promise<{ items: LanRecord[]; watermark: number }> {
  const batch = await invoke<ExportedItem[]>("sync_export_items", {
    sinceTs: since,
    limit: PUSH_BATCH,
    includeImages: config.syncImages
  });
  const items: LanRecord[] = [];
  let bytes = 0;
  let watermark = since;
  for (const item of batch) {
    // 图片默认不同步；开启后后端会先压到 1280px 再给 Base64，
    // 体积仍然超限的图片会被丢弃（image 为空），这里一并跳过。
    if (item.format === "image" && (!config.syncImages || !item.image)) {
      watermark = item.createdAt;
      continue;
    }
    const sealed = await sealItem(keys, item, config.deviceName);
    // 单次请求设一个体积上限：图片再多也不至于一次撑爆通道，
    // 剩下的留到下一轮（水位只推进到真正处理过的这一条）。
    if (items.length > 0 && bytes + sealed.ct.length > PUSH_MAX_BYTES) break;
    items.push(sealed);
    bytes += sealed.ct.length;
    watermark = item.createdAt;
  }
  return { items, watermark };
}

async function pushOnce(config: SyncConfig, keys: Keys): Promise<number> {
  const { items, watermark } = await collectSealed(config, keys, config.pushedUpTo);

  // 一批全是不同步的图片时，水位也要往前走，否则会每 5 秒重复导出同一批
  if (items.length === 0) {
    if (watermark !== config.pushedUpTo) saveConfig({ pushedUpTo: watermark });
    return 0;
  }

  await request(config.relay, "/api/push", {
    method: "POST",
    proof: keys.proof,
    body: JSON.stringify({ room: keys.roomId, items }),
    timeout: config.syncImages ? PUSH_BATCH_TIMEOUT_MS : PULL_BATCH_TIMEOUT_MS
  });

  // 推送成功后再推进水位：失败时下一轮会原样重试，不会把内容悄悄丢掉
  if (watermark !== config.pushedUpTo) {
    saveConfig({ pushedUpTo: watermark });
  }

  return items.length;
}

/**
 * 解密并入库一批记录。
 *
 * 返回解开多少条、跳过多少条：跳过数不为 0 时要让用户看见，
 * 否则"收到了却一条也没进来"看起来和"什么都没收到"一模一样。
 */
async function importRecords(
  keys: Keys,
  records: SealedRecord[]
): Promise<{ imported: number; skipped: number }> {
  if (records.length === 0) return { imported: 0, skipped: 0 };
  const incoming = [];
  let skipped = 0;
  for (const record of records) {
    try {
      incoming.push(await openRecord(keys, record));
    } catch {
      // 单条解不开（多半是别的版本写的）就跳过，不能让一条坏记录卡住整轮同步
      skipped += 1;
    }
  }
  if (incoming.length === 0) return { imported: 0, skipped };
  await invoke<number>("sync_import_items", { items: incoming });
  return { imported: incoming.length, skipped };
}

async function pullOnce(
  config: SyncConfig,
  keys: Keys
): Promise<{ imported: number; skipped: number }> {
  const result = await request(
    config.relay,
    `/api/pull?room=${encodeURIComponent(keys.roomId)}&since=${config.cursor}`,
    { method: "GET", proof: keys.proof }
  );

  const records: RemoteRecord[] = Array.isArray(result?.items) ? result.items : [];
  const { imported, skipped } = await importRecords(keys, records);

  const nextCursor = Number(result?.seq ?? config.cursor) || config.cursor;
  saveConfig({ cursor: nextCursor });
  return { imported, skipped };
}

// ── 局域网直连 ────────────────────────────────────────────────

/**
 * 告诉 Rust 当前房间，并取回在线邻居。
 *
 * 每次都调一次而不是只在开启时调：换同步码后房间号会变，Rust 侧只做赋值，
 * 重复调用不重启监听、不会抢端口。
 */
async function lanPeersOf(config: SyncConfig, keys: Keys): Promise<LanPeer[]> {
  await invoke("lan_sync_configure", { room: keys.roomId, device: config.deviceName });
  // 每轮都重放一次手填地址：Rust 侧换房间/重启后不会丢，重复调用只是改写已有条目
  await applyManualPeers(config);
  return invoke<LanPeer[]>("lan_sync_peers");
}

interface LanProbe {
  addr: string;
  device: string;
  ok: boolean;
  error: string;
}

/** 探测结果缓存：握手本身很轻，但没必要每 5 秒一轮都做 */
let lastProbeAt = 0;
let lastProbe: LanProbe[] = [];

/** 把底层错误翻成用户能行动的说法 */
function explainProbeError(raw: string): string {
  const text = raw || "";
  if (/refused|拒绝/i.test(text)) return "对端端口拒绝连接：那台设备可能没在运行、没开启同步，或被防火墙拦截";
  if (/timed? ?out|超时/i.test(text)) return "连接超时，多半是防火墙或网络隔离（AP 隔离）";
  if (/unreachable|No route|网络不可达/i.test(text)) return "网络不可达，两台设备可能不在同一网段";
  if (/拒绝接收/.test(text)) return "被对端拒绝，两台的同步码可能不一致";
  return text || "连接失败";
}

/**
 * 试探每个邻居的传输端口。
 *
 * 发现是 UDP 广播、传输是 TCP，两者完全独立：能收到宣告不代表连得上，
 * 这正是"显示 1 台设备却同步 0 条"最常见的成因。
 */
async function probePeers(peers: LanPeer[]): Promise<LanProbe[]> {
  if (peers.length === 0) {
    lastProbeAt = 0;
    lastProbe = [];
    return [];
  }
  if (Date.now() - lastProbeAt < 20_000) return lastProbe;
  lastProbeAt = Date.now();
  lastProbe = await invoke<LanProbe[]>("lan_sync_probe").catch(() => []);
  return lastProbe;
}

/** 探测结果带缓存，用户刚加了地址必须立刻看到真实结果，所以手动清一次 */
function invalidateProbeCache() {
  lastProbeAt = 0;
  lastProbe = [];
}

/**
 * 把手填地址灌进 Rust 侧。
 *
 * 失败一律忽略：地址可能来自旧网络（换了 Wi-Fi 后 192.168.1.x 不再可达），
 * 或者本身就是脏数据。让它安静失败，不该拖累整轮同步。
 */
async function applyManualPeers(config: SyncConfig): Promise<void> {
  for (const addr of config.manualPeers ?? []) {
    await invoke("lan_sync_add_peer", { addr }).catch(() => {});
  }
}

/**
 * 手动添加一台设备，并立刻握一次手。
 *
 * 这是"广播发现不到"时的唯一保底手段：只要两台机器能互相路由，
 * 纯 TCP 直连就一定能传，不受 AP 隔离、跨网段、组播被拦的影响。
 */
export async function addManualPeer(input: string): Promise<AddPeerResult> {
  const config = loadConfig();
  if (!config.enabled || !config.code) {
    throw new Error("请先开启同步，再加入设备");
  }
  // 先把自己这边的房间和监听配好：否则对端连过来会被拒，探测也只能拿到空结果
  const keys = await deriveKeys(config.code);
  await invoke("lan_sync_configure", { room: keys.roomId, device: config.deviceName });
  const addr = await invoke<string>("lan_sync_add_peer", { addr: input });
  if (!config.manualPeers.includes(addr)) {
    saveConfig({ manualPeers: [...config.manualPeers, addr] });
  }
  invalidateProbeCache();

  const probes = await invoke<LanProbe[]>("lan_sync_probe").catch(() => []);
  const mine = probes.find((probe) => probe.addr === addr);
  if (!mine) {
    return {
      addr,
      ok: false,
      message: `已添加 ${addr}，但对方还没回应：确认那台设备也打开了「局域网直连」并填入同一个同步码`
    };
  }
  return {
    addr,
    ok: mine.ok,
    message: mine.ok ? `已连上 ${mine.device || addr}` : explainProbeError(mine.error)
  };
}

/** 撤销一条手填地址。 */
export function removeManualPeer(addr: string): SyncConfig {
  void invoke("lan_sync_remove_peer", { addr }).catch(() => {});
  invalidateProbeCache();
  const next = (loadConfig().manualPeers ?? []).filter((item) => item !== addr);
  return saveConfig({ manualPeers: next });
}

/** 本机在局域网里的 IPv4 地址，对方要手填的就是它。 */
export async function lanLocalAddrs(): Promise<string[]> {
  return invoke<string[]>("lan_sync_local_addrs").catch(() => []);
}

/** 一键体检：端口、房间、邻居握手逐项查一遍。 */
export async function lanDiagnose(): Promise<LanDiagnosis | null> {
  return invoke<LanDiagnosis>("lan_sync_diagnose").catch(() => null);
}

/**
 * 跑一轮局域网同步：先把收件箱里对端推来的记录入库，再把本地新增推出去。
 *
 * 没有邻居时不推进水位——宁可下一轮重复导出，也不能让内容在"当时没人收"的
 * 情况下被悄悄标记成已发送。
 */
async function lanRound(
  config: SyncConfig,
  keys: Keys,
  peers: LanPeer[],
  base: { pulled?: number; pushed?: number; tolerant?: boolean } = {}
): Promise<void> {
  const probes = await probePeers(peers);
  const unreachable = probes
    .filter((probe) => !probe.ok)
    .map((probe) => `${probe.device} · ${probe.addr.split(":")[0]}：${explainProbeError(probe.error)}`);

  // 只发送时把收件箱原样留着不动：这里取走就等于承认"已接收"，
  // 切回双向后那些内容就再也补不回来了。
  let pulled = 0;
  let skipped = 0;
  if (config.mode !== "send") {
    const inbox = await invoke<LanRecord[]>("lan_sync_take");
    const result = await importRecords(keys, inbox);
    pulled = result.imported;
    skipped = result.skipped;
  }

  let pushed = 0;
  if (config.mode !== "receive" && peers.length > 0) {
    const { items, watermark } = await collectSealed(config, keys, config.lanPushedUpTo);
    if (items.length === 0) {
      // 这一批全是"不同步的图片"：水位也得往前走，否则每 5 秒重复导出同一批
      if (watermark !== config.lanPushedUpTo) saveConfig({ lanPushedUpTo: watermark });
    } else {
      const accepted = await invoke<number>("lan_sync_publish", { items });
      if (accepted > 0) {
        pushed = items.length;
        if (watermark !== config.lanPushedUpTo) saveConfig({ lanPushedUpTo: watermark });
      } else if (!base.tolerant) {
        // 一个设备都没收下：优先用探测到的具体原因，比"发出 0 条"有用得多
        const reason = await invoke<string>("lan_sync_error").catch(() => "");
        const detail = unreachable[0] || explainProbeError(reason) || "对端没有响应";
        throw new Error(`发送失败：${detail}`);
      }
    }
  }

  const now = Date.now();
  const totalPulled = (base.pulled ?? 0) + pulled;
  const totalPushed = (base.pushed ?? 0) + pushed;
  saveConfig({ lastSyncAt: now, lastError: "", lastPulled: totalPulled, lastPushed: totalPushed });
  setStatus({
    busy: false,
    lastSyncAt: now,
    lastPulled: totalPulled,
    lastPushed: totalPushed,
    lastError: "",
    lanPeers: peers.length,
    lanPeerNames: peers.map((peer) => peer.device || "设备"),
    lanUnreachable: unreachable,
    skipped
  });
}

/** 走中继的一轮：建房间 → 推本地新增 → 拉对端更新。返回本轮收发条数。 */
async function relayRound(config: SyncConfig, keys: Keys): Promise<{ pulled: number; pushed: number }> {
  await request(config.relay, "/api/room", {
    method: "POST",
    proof: keys.proof,
    body: JSON.stringify({ room: keys.roomId })
  });

  let pushed = 0;
  if (config.mode !== "receive") {
    // 一次最多推 3 批，避免首次启用时把界面卡在长时间的网络往返里
    for (let round = 0; round < 3; round += 1) {
      const count = await pushOnce(loadConfig(), keys);
      pushed += count;
      if (count === 0) break;
    }
  }
  let pulled = 0;
  let skipped = 0;
  if (config.mode !== "send") {
    const result = await pullOnce(loadConfig(), keys);
    pulled = result.imported;
    skipped = result.skipped;

    // 只要这台设备以前用过局域网，对端就可能仍在往收件箱里推。
    // 中继正常时也不能把这些记录晾着，否则用户会看到"对方说发了、这边没有"。
    const inbox = await invoke<LanRecord[]>("lan_sync_take").catch(() => [] as LanRecord[]);
    if (inbox.length > 0) {
      const local = await importRecords(keys, inbox);
      pulled += local.imported;
      skipped += local.skipped;
    }
  }

  const now = Date.now();
  saveConfig({ lastSyncAt: now, lastError: "", lastPulled: pulled, lastPushed: pushed });
  setStatus({
    busy: false,
    lastSyncAt: now,
    lastPulled: pulled,
    lastPushed: pushed,
    lastError: "",
    lanPeers: 0,
    lanPeerNames: [],
    lanUnreachable: [],
    skipped
  });
  return { pulled, pushed };
}

/** 跑一轮完整同步：中继与局域网并行，哪条通都能收发。 */
export async function syncNow(): Promise<SyncStatus> {
  const config = loadConfig();
  if (!config.enabled || !config.code) {
    return status;
  }
  // 没配中继又没开局域网时不要空转：直接给出可执行的提示，比 fetch 空 URL 报错清楚
  if (!config.lanOnly && !config.relay) {
    const message = "同步服务未配置：构建时没有注入中继地址（VITE_XPASTE_RELAY）";
    saveConfig({ lastError: message });
    setStatus({ busy: false, lastError: message });
    return status;
  }
  if (busy) return status;

  busy = true;
  setStatus({ busy: true });
  try {
    const keys = await deriveKeys(config.code);
    const useRelay = !config.lanOnly && Boolean(config.relay);

    // 无论走哪条通道，都先把局域网监听与邻居拉起来：
    // 对端可能正从中继模式的设备手动直连过来，只有监听在，47832 才不是"拒绝连接"。
    const peers = await lanPeersOf(config, keys).catch(() => [] as LanPeer[]);

    if (!useRelay) {
      await lanRound(config, keys, peers);
    } else {
      try {
        const relay = await relayRound(config, keys);
        // 邻居在场时局域网也并行跑一轮：对端可能只开了局域网，或它的中继正被网络挡住。
        // 两条通道重复投递由入库时的内容指纹去重，不会产生重复记录。
        if (peers.length > 0) {
          try {
            await lanRound(config, keys, peers, {
              pulled: relay.pulled,
              pushed: relay.pushed,
              tolerant: true
            });
          } catch {
            // 中继这轮已经成功，局域网的问题不该把整轮标成失败
            setStatus({
              lanPeers: peers.length,
              lanPeerNames: peers.map((peer) => peer.device || "设备")
            });
          }
        }
      } catch (error) {
        // 中继被网络环境挡住（WebView 只吐一句 Load failed）时不要直接放弃：
        // 局域网里如果有同房间设备，就改走直连，用户无感。
        if (peers.length === 0) throw error;
        await lanRound(config, keys, peers);
      }
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    saveConfig({ lastError: message });
    setStatus({ busy: false, lastError: message });
  } finally {
    busy = false;
  }
  return status;
}

/**
 * 设备名：早期版本写死 "Mac"/"PC"，同一平台的两台设备在列表里分不清。
 * 这里一次性升级成主机名（Rust 侧取），失败就沿用原值。
 */
let deviceNameResolved = false;
export async function ensureDeviceName(): Promise<string> {
  const current = loadConfig().deviceName;
  if (deviceNameResolved) return current;
  deviceNameResolved = true;
  if (current && !/^(Mac|PC|Device)$/.test(current)) return current;
  const label = await invoke<string>("device_label").catch(() => "");
  if (!label || label === current) return current;
  saveConfig({ deviceName: label });
  return label;
}

/** 启用同步：没有同步码时先生成一个，然后立刻同步一次。 */
export async function enableSync(code?: string): Promise<void> {
  await ensureDeviceName();
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
    lanPushedUpTo: isNewRoom
      ? Date.now() - INITIAL_BACKFILL_MS
      : current.lanPushedUpTo || Date.now() - INITIAL_BACKFILL_MS,
    lastError: ""
  });
  startSync();
  void syncNow();
}

export function disableSync(): void {
  stopSync();
  saveConfig({ enabled: false, lastError: "" });
}

/**
 * 切换图片同步。
 *
 * 关闭时只改开关。开启时把水位往回拨 7 天：之前那些图片在"只同步文字"
 * 阶段已经被水位跳过了，不回溯的话打开开关也永远补不上。
 */
export function setSyncImages(enabled: boolean): SyncConfig {
  const current = loadConfig();
  const floor = Date.now() - INITIAL_BACKFILL_MS;
  const pushedUpTo = enabled ? Math.min(current.pushedUpTo, floor) : current.pushedUpTo;
  const lanPushedUpTo = enabled ? Math.min(current.lanPushedUpTo, floor) : current.lanPushedUpTo;
  const next = saveConfig({ syncImages: enabled, pushedUpTo, lanPushedUpTo });
  if (enabled && next.enabled) {
    void syncNow();
  }
  return next;
}

/** 换一个全新同步码：旧房间的数据不会跟过来，等于换了身份。 */
export function rotateCode(): string {
  const code = generateCode();
  const floor = Date.now() - INITIAL_BACKFILL_MS;
  saveConfig({
    code,
    cursor: 0,
    pushedUpTo: floor,
    lanPushedUpTo: floor,
    lastError: ""
  });
  if (loadConfig().enabled) {
    startSync();
    void syncNow();
  }
  return code;
}

const POLL_INTERVAL_MS = 5000;

export function startSync(): void {
  // 设备名要在第一轮同步之前定下来：它会写进每条加密记录里
  void ensureDeviceName();
  const config = loadConfig();
  if (!config.enabled || !config.code) {
    stopSync();
    return;
  }
  // 中继没配、也没开局域网时别起轮询，省得每 5 秒白跑一次
  if (!config.relay && !config.lanOnly) {
    stopSync();
    setStatus({ lastError: "同步服务未配置：构建时没有注入中继地址（VITE_XPASTE_RELAY）" });
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
  void invoke("lan_sync_stop").catch(() => {
    // 命令不可用（例如浏览器里调试）时忽略，轮询停掉就够了
  });
  setStatus({ running: false, busy: false, lanPeers: 0, lanPeerNames: [], lanUnreachable: [], skipped: 0 });
}

/**
 * 切换「只在局域网内同步」。
 *
 * 打开后立刻跑一轮：如果同 Wi-Fi 下已经有设备用同一个同步码，几秒内就能看到
 * 对方的邻居数变成 1，不需要等下一次轮询。
 */
export function setLanOnly(enabled: boolean): SyncConfig {
  const current = loadConfig();
  const floor = Date.now() - INITIAL_BACKFILL_MS;
  const patch: Partial<SyncConfig> = { lanOnly: enabled, lastError: "" };
  // 重新打开且水位已经很旧时只补最近 7 天：否则半年历史会被一批批慢慢推过去，
  // 对端会连续几十分钟在"补充旧内容"
  if (enabled && (!current.lanPushedUpTo || current.lanPushedUpTo < floor)) {
    patch.lanPushedUpTo = floor;
  }
  const next = saveConfig(patch);
  // 两种方向都保留局域网监听：关掉开关只是重新允许中继，
  // 局域网仍作为并行通道工作，不能让对端的手动直连变成"拒绝连接"。
  void syncNow();
  return next;
}

/**
 * 切换同步方向，立刻跑一轮让界面反映新方向。
 *
 * 这里刻意不动水位和游标：只收期间不推进发送水位，切回双向时这段时间攒下的
 * 本地内容会自动补发；只发期间不推进接收游标，切回双向会补拉对端内容。
 */
export function setSyncMode(mode: SyncMode): SyncConfig {
  const next = saveConfig({ mode, lastError: "" });
  if (next.enabled) {
    void syncNow();
  }
  return next;
}
