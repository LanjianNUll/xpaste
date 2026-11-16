//! 局域网直连同步。
//!
//! 为什么要有这条通道：中继走公网，家里/公司网络偶尔会把请求拦掉
//! （WebView 里表现为 `Load failed`），而两台设备明明就在同一个 Wi-Fi 下。
//! 这里用「UDP 组播发现 + TCP 直传」把同一房间的设备连起来，不依赖任何服务器。
//!
//! 分工刻意和前端保持一致：Rust 只搬字节，拿到的是已经用房间密钥加密好的
//! `{id, iv, ct}`，既解不开内容也不知道明文。房间号是同步码派生出来的哈希，
//! 只用来做路由过滤——同网段的陌生设备即使收到包，房间对不上也不会收。
//!
//! 发现与传输端口固定（见 `DISCOVERY_PORT` / `SERVICE_PORT`），两台设备
//! 无需任何配置即可互找；`SO_REUSEADDR` + `SO_REUSEPORT` 让同一台机器上
//! 开发时的多个实例也能共存。

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};

/// 发现端口：组播包只在这上面收发。
pub const DISCOVERY_PORT: u16 = 47831;
/// 传输端口：接收对端推来的密文记录。
pub const SERVICE_PORT: u16 = 47832;

/// 链路本地组播组，TTL 固定 1，路由器不会把它转发出局域网。
const GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 72, 31);

/// 每隔多久喊一次「我在」。3 秒足够让对端在下一轮同步（5 秒）前看到自己。
const ANNOUNCE_INTERVAL: Duration = Duration::from_secs(3);
/// 多久没听到某台设备就认为它离线了：留 4 个宣告周期，避免偶发丢包就抖动。
const PEER_TTL: Duration = Duration::from_secs(12);

/// 收件箱上限：一次同步搬几十条，正常远达不到；超了丢最旧的，防止内存无限涨。
const INBOX_MAX_RECORDS: usize = 2000;
const INBOX_MAX_BYTES: usize = 96 * 1024 * 1024;

/// 一条传输记录。字段与前端 `RemoteRecord` 完全对齐，Rust 不解释内容。
#[derive(Clone, Serialize, Deserialize)]
pub struct LanRecord {
    pub id: String,
    #[serde(default)]
    pub kind: String,
    pub iv: String,
    pub ct: String,
    #[serde(default)]
    pub ts: i64,
}

/// 前端看到的邻居设备。
#[derive(Clone, Serialize)]
pub struct LanPeer {
    pub addr: String,
    pub device: String,
    pub last_seen_ms: i64,
}

#[derive(Serialize, Deserialize)]
struct Packet {
    /// `announce` / `push`
    op: String,
    #[serde(default)]
    room: String,
    #[serde(default)]
    device: String,
    #[serde(default)]
    port: u16,
    /// 进程实例号：用来忽略自己发出的组播（同一台机器多实例时也不会认错）
    #[serde(default)]
    iid: String,
    #[serde(default)]
    items: Vec<LanRecord>,
}

#[derive(Serialize, Deserialize)]
struct Reply {
    ok: bool,
    #[serde(default)]
    count: usize,
}

struct Peer {
    device: String,
    last_seen: Instant,
}

struct Inner {
    /// 房间号（同步码派生），空表示未启用
    room: String,
    device: String,
    iid: String,
    peers: HashMap<String, Peer>,
    inbox: Vec<LanRecord>,
    inbox_bytes: usize,
    /// 最近一次传输失败的原因，给前端做提示
    last_error: String,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            room: String::new(),
            device: "Device".to_string(),
            iid: format!("{}-{}", now_ms(), std::process::id()),
            peers: HashMap::new(),
            inbox: Vec::new(),
            inbox_bytes: 0,
            last_error: String::new(),
        }
    }
}

/// 全局状态。线程只启动一次，靠 `room` 是否为空来决定要不要收发，
/// 这样换同步码时不用重启监听，也就不会有端口占用问题。
pub struct LanSync {
    inner: Mutex<Inner>,
    started: AtomicBool,
}

impl Default for LanSync {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            started: AtomicBool::new(false),
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl LanSync {
    /// 记录当前房间并确保后台线程已启动。房间没变时是纯赋值，开销可忽略。
    pub fn configure(&self, room: &str, device: &str) -> Result<(), String> {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.room = room.to_string();
            inner.device = device.to_string();
            if inner.room.is_empty() {
                inner.peers.clear();
                inner.inbox.clear();
                inner.inbox_bytes = 0;
            }
        }
        self.ensure_threads();
        Ok(())
    }

    /// 停止收发：清空房间与邻居，线程保留（下次开启不必重新绑定端口）。
    pub fn stop(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.room.clear();
        inner.peers.clear();
        inner.inbox.clear();
        inner.inbox_bytes = 0;
        inner.last_error.clear();
    }

    /// 当前在线的同房间设备。
    pub fn peers(&self) -> Vec<LanPeer> {
        let mut inner = self.inner.lock().unwrap();
        inner.prune();
        inner
            .peers
            .iter()
            .map(|(addr, peer)| LanPeer {
                addr: addr.clone(),
                device: peer.device.clone(),
                last_seen_ms: now_ms() - peer.last_seen.elapsed().as_millis() as i64,
            })
            .collect()
    }

    /// 把密文记录推给所有邻居，返回成功接收的设备数。
    pub fn publish(&self, items: Vec<LanRecord>) -> Result<usize, String> {
        if items.is_empty() {
            return Ok(0);
        }
        let targets = {
            let mut inner = self.inner.lock().unwrap();
            if inner.room.is_empty() {
                return Err("局域网同步未开启".to_string());
            }
            inner.prune();
            inner
                .peers
                .keys()
                .filter_map(|addr| addr.parse::<SocketAddr>().ok())
                .collect::<Vec<_>>()
        };
        if targets.is_empty() {
            return Ok(0);
        }

        let (room, device) = {
            let inner = self.inner.lock().unwrap();
            (inner.room.clone(), inner.device.clone())
        };
        let packet = Packet {
            op: "push".to_string(),
            room,
            device,
            port: SERVICE_PORT,
            iid: String::new(),
            items,
        };
        let Ok(line) = serde_json::to_string(&packet) else {
            return Err("记录序列化失败".to_string());
        };

        let mut accepted = 0usize;
        let mut last_error = String::new();
        for addr in targets {
            match send_line(addr, &line) {
                Ok(true) => accepted += 1,
                Ok(false) => last_error = "对端拒绝接收".to_string(),
                Err(err) => last_error = err,
            }
        }

        let mut inner = self.inner.lock().unwrap();
        inner.last_error = if accepted == 0 { last_error } else { String::new() };
        Ok(accepted)
    }

    /// 取走并清空收件箱。前端拿到后自行解密、入库。
    pub fn take(&self) -> Vec<LanRecord> {
        let mut inner = self.inner.lock().unwrap();
        inner.inbox_bytes = 0;
        std::mem::take(&mut inner.inbox)
    }

    /// 收件箱待处理条数（前端用来判断要不要立刻拉一轮）。
    pub fn pending(&self) -> usize {
        self.inner.lock().unwrap().inbox.len()
    }

    pub fn last_error(&self) -> String {
        self.inner.lock().unwrap().last_error.clone()
    }

    fn ensure_threads(&self) {
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }
        // 两个线程都活到进程结束：一个守 TCP，一个喊话 + 收话。
        std::thread::Builder::new()
            .name("xpaste-lan-server".into())
            .spawn(move || {
                if let Err(err) = serve_loop() {
                    eprintln!("[lan-sync] 接收端口不可用：{err}");
                }
            })
            .ok();
        std::thread::Builder::new()
            .name("xpaste-lan-discovery".into())
            .spawn(move || {
                if let Err(err) = discovery_loop() {
                    eprintln!("[lan-sync] 发现端口不可用：{err}");
                }
            })
            .ok();
    }
}

/// 进程级单例：线程里要访问状态，避免把 `State` 传进裸线程。
static GLOBAL: OnceLock<&'static LanSync> = OnceLock::new();

pub fn global() -> &'static LanSync {
    GLOBAL.get_or_init(|| Box::leak(Box::new(LanSync::default())))
}

impl Inner {
    fn prune(&mut self) {
        self.peers.retain(|_, peer| peer.last_seen.elapsed() < PEER_TTL);
    }

    fn push_inbox(&mut self, items: Vec<LanRecord>) {
        for item in items {
            self.inbox_bytes += item.ct.len();
            self.inbox.push(item);
        }
        while self.inbox.len() > INBOX_MAX_RECORDS {
            if let Some(old) = self.inbox.first() {
                let size = old.ct.len();
                self.inbox.remove(0);
                self.inbox_bytes = self.inbox_bytes.saturating_sub(size);
            } else {
                break;
            }
        }
        while self.inbox_bytes > INBOX_MAX_BYTES && !self.inbox.is_empty() {
            let old = self.inbox.remove(0);
            self.inbox_bytes = self.inbox_bytes.saturating_sub(old.ct.len());
        }
    }
}

// ── TCP 接收 ──────────────────────────────────────────────────

fn serve_loop() -> Result<(), String> {
    let listener = bind_listener(SERVICE_PORT)?;
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("设置非阻塞失败：{err}"))?;

    loop {
        match listener.accept() {
            Ok((stream, _addr)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
                std::thread::spawn(move || handle_conn(stream));
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(120));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

fn handle_conn(stream: TcpStream) {
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(clone) => clone,
        Err(_) => return,
    });
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let Ok(packet) = serde_json::from_str::<Packet>(&line) else {
        reply(&stream, false, 0);
        return;
    };
    if packet.op != "push" {
        reply(&stream, false, 0);
        return;
    }

    let mut inner = global().inner.lock().unwrap();
    // 房间对不上就直接拒绝：同网段的陌生设备不该往我们的列表里写东西
    if inner.room.is_empty() || packet.room != inner.room {
        drop(inner);
        reply(&stream, false, 0);
        return;
    }
    let count = packet.items.len();
    inner.push_inbox(packet.items);
    drop(inner);
    reply(&stream, true, count);
}

fn reply(stream: &TcpStream, ok: bool, count: usize) {
    let payload = serde_json::to_string(&Reply { ok, count }).unwrap_or_else(|_| "{\"ok\":false}".into());
    let mut writer = stream;
    let _ = writer.write_all(payload.as_bytes());
    let _ = writer.write_all(b"\n");
    let _ = writer.flush();
}

fn send_line(addr: SocketAddr, line: &str) -> Result<bool, String> {
    let target = SocketAddr::new(addr.ip(), SERVICE_PORT);
    let mut stream = TcpStream::connect_timeout(&target, Duration::from_millis(1200))
        .map_err(|err| format!("连接 {target} 失败：{err}"))?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(2500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(2500)));
    stream
        .write_all(format!("{line}\n").as_bytes())
        .map_err(|err| format!("发送失败：{err}"))?;
    stream.flush().ok();

    let mut response = String::new();
    let mut reader = BufReader::new(stream);
    if reader.read_line(&mut response).is_err() {
        return Ok(false);
    }
    match serde_json::from_str::<Reply>(&response) {
        Ok(reply) => Ok(reply.ok),
        Err(_) => Ok(false),
    }
}

fn bind_listener(port: u16) -> Result<TcpListener, String> {
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
        .map_err(|err| format!("创建套接字失败：{err}"))?;
    socket
        .set_reuse_address(true)
        .map_err(|err| format!("设置端口复用失败：{err}"))?;
    #[cfg(unix)]
    let _ = socket.set_reuse_port(true);
    socket
        .bind(&SocketAddr::from(([0, 0, 0, 0], port)).into())
        .map_err(|err| format!("绑定端口 {port} 失败：{err}"))?;
    socket
        .listen(64)
        .map_err(|err| format!("监听端口 {port} 失败：{err}"))?;
    Ok(socket.into())
}

// ── UDP 发现 ──────────────────────────────────────────────────

/// 宣告要发往的地址。
///
/// 优先按网卡逐个发子网广播（`192.168.1.255:47831`），这条路在家用 Wi-Fi、
/// 公司有线网、虚拟机的 NAT 网段里都成立；组播和受限广播只是顺带试一下，
/// 失败也无所谓——它们只是让发现更快，不是唯一手段。
fn announce_targets() -> Vec<SocketAddr> {
    let mut targets = Vec::new();
    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for interface in interfaces {
            if interface.is_loopback() {
                continue;
            }
            if let if_addrs::IfAddr::V4(v4) = interface.addr {
                if let Some(broadcast) = v4.broadcast {
                    targets.push(SocketAddr::from((broadcast, DISCOVERY_PORT)));
                }
            }
        }
    }
    targets.push(SocketAddr::from((Ipv4Addr::BROADCAST, DISCOVERY_PORT)));
    targets.push(SocketAddr::from((GROUP, DISCOVERY_PORT)));
    targets.sort_by_key(|addr| addr.to_string());
    targets.dedup_by_key(|addr| addr.to_string());
    targets
}

fn discovery_loop() -> Result<(), String> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
        .map_err(|err| format!("创建发现套接字失败：{err}"))?;
    socket
        .set_reuse_address(true)
        .map_err(|err| format!("设置端口复用失败：{err}"))?;
    #[cfg(unix)]
    let _ = socket.set_reuse_port(true);
    socket
        .bind(&SocketAddr::from(([0, 0, 0, 0], DISCOVERY_PORT)).into())
        .map_err(|err| format!("绑定发现端口失败：{err}"))?;
    // 组播在部分网络（虚拟机、关掉 IGMP 的 AP）会直接 "No route to host"，
    // 所以它能加入就加入，发不出去也不影响：真正的发现靠下面的子网广播。
    let _ = socket.join_multicast_v4(&GROUP, &Ipv4Addr::UNSPECIFIED);
    let _ = socket.set_multicast_loop_v4(true);
    let _ = socket.set_multicast_ttl_v4(1);
    socket
        .set_broadcast(true)
        .map_err(|err| format!("开启广播失败：{err}"))?;
    socket
        .set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(|err| format!("设置读超时失败：{err}"))?;
    let udp: UdpSocket = socket.into();

    let mut last_announce = Instant::now() - ANNOUNCE_INTERVAL;
    let mut buf = [0u8; 8192];

    loop {
        // 该喊话了：把自己的房间号与本机端口广播出去
        if last_announce.elapsed() >= ANNOUNCE_INTERVAL {
            let payload = {
                let inner = global().inner.lock().unwrap();
                if inner.room.is_empty() {
                    None
                } else {
                    serde_json::to_string(&Packet {
                        op: "announce".to_string(),
                        room: inner.room.clone(),
                        device: inner.device.clone(),
                        port: SERVICE_PORT,
                        iid: inner.iid.clone(),
                        items: Vec::new(),
                    })
                    .ok()
                }
            };
            if let Some(payload) = payload {
                for target in announce_targets() {
                    let _ = udp.send_to(payload.as_bytes(), target);
                }
            }
            last_announce = Instant::now();
        }

        match udp.recv_from(&mut buf) {
            Ok((size, from)) => {
                if size == 0 || size > buf.len() {
                    continue;
                }
                let Ok(packet) = serde_json::from_slice::<Packet>(&buf[..size]) else {
                    continue;
                };
                if packet.op != "announce" || packet.port == 0 {
                    continue;
                }

                let mut inner = global().inner.lock().unwrap();
                if inner.room.is_empty() || packet.room != inner.room || packet.iid == inner.iid {
                    continue;
                }
                let addr = SocketAddr::new(from.ip(), packet.port).to_string();
                let is_new = !inner.peers.contains_key(&addr);
                inner.peers.insert(
                    addr.clone(),
                    Peer {
                        device: if packet.device.is_empty() {
                            "Device".to_string()
                        } else {
                            packet.device.clone()
                        },
                        last_seen: Instant::now(),
                    },
                );
                inner.prune();
                drop(inner);

                // 新邻居：回一个单播宣告，对方下一轮不用再等 3 秒
                if is_new {
                    let payload = {
                        let inner = global().inner.lock().unwrap();
                        serde_json::to_string(&Packet {
                            op: "announce".to_string(),
                            room: inner.room.clone(),
                            device: inner.device.clone(),
                            port: SERVICE_PORT,
                            iid: inner.iid.clone(),
                            items: Vec::new(),
                        })
                        .ok()
                    };
                    if let Some(payload) = payload {
                        let _ = udp.send_to(payload.as_bytes(), from);
                    }
                }
            }
            Err(err)
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => std::thread::sleep(Duration::from_millis(400)),
        }
    }
}

// ── Tauri 命令 ────────────────────────────────────────────────

/// 开启/更新局域网同步的房间。重复调用只更新房间号，不会重启监听。
#[tauri::command]
pub fn lan_sync_configure(room: String, device: String) -> Result<(), String> {
    let room = room.trim().to_string();
    if room.is_empty() {
        return Err("房间号为空".to_string());
    }
    global().configure(&room, &device)
}

/// 关闭局域网同步：清掉邻居和收件箱，正在传输的连接会自然结束。
#[tauri::command]
pub fn lan_sync_stop() {
    global().stop();
}

/// 当前在线的同房间设备。
#[tauri::command]
pub fn lan_sync_peers() -> Vec<LanPeer> {
    global().peers()
}

/// 把密文记录推给邻居，返回接收成功的设备数。
#[tauri::command]
pub fn lan_sync_publish(items: Vec<LanRecord>) -> Result<usize, String> {
    global().publish(items)
}

/// 取走收件箱里对端推来的密文记录。
#[tauri::command]
pub fn lan_sync_take() -> Vec<LanRecord> {
    global().take()
}

/// 待处理条数：前端判断要不要立刻跑一轮。
#[tauri::command]
pub fn lan_sync_pending() -> usize {
    global().pending()
}

/// 最近一次传输失败的原因（空字符串表示正常）。
#[tauri::command]
pub fn lan_sync_error() -> String {
    global().last_error()
}
