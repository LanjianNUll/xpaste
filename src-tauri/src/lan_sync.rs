//! 局域网直连同步。
//!
//! 为什么要有这条通道：中继走公网，家里/公司网络偶尔会把请求拦掉
//! （WebView 里表现为 `Load failed`），而两台设备明明就在同一个 Wi-Fi 下。
//! 这里用「UDP 子网广播发现 + TCP 直传」把同一房间的设备连起来，不依赖任何服务器。
//!
//! 分工刻意和前端保持一致：Rust 只搬字节，拿到的是已经用房间密钥加密好的
//! `{id, iv, ct}`，既解不开内容也不知道明文。房间号是同步码派生出来的哈希，
//! 只用来做路由过滤——同网段的陌生设备即使收到包，房间对不上也不会收。
//!
//! 广播再稳也有收不到的时候（跨网段、AP 隔离、VLAN）。所以还留了一条纯手动的
//! 路径：`add_peer` 直接登记一个 `ip:port`，跳过发现只走 TCP。只要两台机器能
//! 互相路由，这条路就能通——它是"一定要同步上"时的保底手段。
//!
//! 发现与传输端口固定（见 `DISCOVERY_PORT` / `SERVICE_PORT`），两台设备
//! 无需任何配置即可互找。传输端口刻意**独占**绑定：宁可明确报错，也不能让
//! 两个进程分走连接后各自静默拒绝。

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

/// 探测用：只想知道"通不通"，等太久会拖慢整轮同步，1.5 秒足够。
const PROBE_CONNECT_TIMEOUT: Duration = Duration::from_millis(1500);
/// 真正推送用：包可能带图片（几百 KB），握手也要给足时间——
/// 在拥挤的 Wi-Fi 上手握 1 秒多很正常，超时会白白丢掉一整批内容。
const PUBLISH_CONNECT_TIMEOUT: Duration = Duration::from_secs(4);

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
#[serde(rename_all = "camelCase")]
pub struct LanPeer {
    pub addr: String,
    pub device: String,
    pub last_seen_ms: i64,
    /// 用户手动填的地址：不参与 TTL 淘汰，广播被挡住时全靠这条链路
    pub manual: bool,
}

/// 一次连通性探测的结果。
///
/// 发现靠 UDP 广播，传输走 TCP：能"看见"对方，不代表能连上对方
/// （Windows 防火墙默认拦入站、AP 隔离、端口被占都会这样）。界面上
/// 把这两件事分开说，用户才不会被"1 台设备 + 0 条"绕晕。
#[derive(Clone, Serialize)]
pub struct PeerProbe {
    pub addr: String,
    pub device: String,
    pub ok: bool,
    pub error: String,
}

/// 一键体检的结果。
///
/// "同步不了"有五六种完全不同的原因（没开同步、端口被占、广播被拦、防火墙拦入站、
/// 两台码不一致、本机没有本地网络权限），界面上只显示一句"0 条"根本分不清。
/// 这里把每一条判据都摆出来，用户（和我们）照着看就知道卡在哪一步。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanDiagnosis {
    /// 后台线程是否真的起来了（false 基本等于端口被旧版本占着）
    pub started: bool,
    pub room_configured: bool,
    pub device: String,
    pub service_port: u16,
    pub discovery_port: u16,
    /// 本机可被对方填写的地址
    pub local_addrs: Vec<String>,
    /// 最近一次失败原因
    pub last_error: String,
    /// 逐个邻居的 TCP 握手结果
    pub peers: Vec<PeerProbe>,
    /// 收件箱里还没被前端取走的条数
    pub inbox: usize,
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
    /// 手动添加的设备：即使暂时没收到它的宣告也一直保留
    manual: bool,
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
        let mut list: Vec<LanPeer> = inner
            .peers
            .iter()
            .map(|(addr, peer)| LanPeer {
                addr: addr.clone(),
                device: peer.device.clone(),
                last_seen_ms: now_ms() - peer.last_seen.elapsed().as_millis() as i64,
                manual: peer.manual,
            })
            .collect();
        // 手动填的排前面：出问题时用户最先要看的是自己指定的那条链路
        list.sort_by(|a, b| b.manual.cmp(&a.manual).then(a.addr.cmp(&b.addr)));
        list
    }

    /// 手动登记一台设备。
    ///
    /// 为什么需要它：UDP 广播在跨网段、开了 AP 隔离的 Wi-Fi、公司 VLAN 里都可能
    /// 完全收不到，这时"自动发现"永远 0 台，界面再准也没用。填一个 IP 就走
    /// 纯 TCP 直连，只要两台机器能互相路由就一定能传——这是保底路径。
    pub fn add_peer(&self, addr: &str) -> Result<String, String> {
        let parsed = parse_peer_addr(addr)?;
        if parsed.ip().is_loopback() {
            return Err("不能填本机地址，请填另一台电脑的 IP".to_string());
        }
        if is_local_ip(parsed.ip()) {
            return Err("这是本机网卡地址，请填另一台电脑的 IP".to_string());
        }
        let mut inner = self.inner.lock().unwrap();
        let key = parsed.to_string();
        inner
            .peers
            .entry(key.clone())
            .and_modify(|peer| {
                peer.manual = true;
                peer.last_seen = Instant::now();
            })
            .or_insert(Peer {
                device: String::new(),
                last_seen: Instant::now(),
                manual: true,
            });
        Ok(key)
    }

    /// 撤销一条手动登记。
    pub fn remove_peer(&self, addr: &str) -> bool {
        let Ok(parsed) = parse_peer_addr(addr) else {
            return false;
        };
        let mut inner = self.inner.lock().unwrap();
        match inner.peers.get_mut(&parsed.to_string()) {
            Some(peer) if peer.manual => {
                // 只摘掉"手动"标记；它若还在广播，下一轮会自动以自动发现的身份回来
                inner.peers.remove(&parsed.to_string());
                true
            }
            _ => false,
        }
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
        let mut failures: Vec<String> = Vec::new();
        for addr in targets {
            // 局域网里第一次握手偶尔会撞上对端 accept 队列满/短暂忙，重试一次
            // 比直接报"发送失败"划算得多——用户看到的差别是"同步上了"和"又坏了"。
            let mut result = send_line_with(addr, &line, PUBLISH_CONNECT_TIMEOUT);
            if result.is_err() {
                std::thread::sleep(Duration::from_millis(150));
                result = send_line_with(addr, &line, PUBLISH_CONNECT_TIMEOUT);
            }
            match result {
                Ok(true) => {
                    accepted += 1;
                    if let Ok(mut inner) = self.inner.lock() {
                        if let Some(peer) = inner.peers.get_mut(&addr.to_string()) {
                            peer.last_seen = Instant::now();
                        }
                    }
                }
                Ok(false) => failures.push(format!("{addr} 拒绝接收（同步码可能不一致）")),
                Err(err) => failures.push(format!("{addr} {err}")),
            }
        }

        let mut inner = self.inner.lock().unwrap();
        inner.last_error = if accepted == 0 {
            failures.join("；")
        } else {
            String::new()
        };
        Ok(accepted)
    }

    /// 探一遍每个邻居的传输端口。
    ///
    /// 发的是空 items 的 push：对端只会回应 ok，不会往列表里写任何东西。
    /// 这一步能把"看得见但连不上"和"能连上但没内容"彻底区分开。
    pub fn probe(&self) -> Vec<PeerProbe> {
        let targets = {
            let mut inner = self.inner.lock().unwrap();
            inner.prune();
            inner
                .peers
                .iter()
                .filter_map(|(addr, peer)| {
                    addr.parse::<SocketAddr>()
                        .ok()
                        .map(|parsed| (parsed, peer.device.clone()))
                })
                .collect::<Vec<_>>()
        };
        if targets.is_empty() {
            return Vec::new();
        }

        let (room, device) = {
            let inner = self.inner.lock().unwrap();
            (inner.room.clone(), inner.device.clone())
        };
        if room.is_empty() {
            return Vec::new();
        }

        let packet = Packet {
            op: "push".to_string(),
            room,
            device,
            port: SERVICE_PORT,
            iid: String::new(),
            items: Vec::new(),
        };
        let Ok(line) = serde_json::to_string(&packet) else {
            return Vec::new();
        };

        targets
            .into_iter()
            .map(|(addr, device)| {
                let result = send_line(addr, &line);
                PeerProbe {
                    addr: addr.to_string(),
                    device,
                    ok: matches!(result, Ok(true)),
                    error: match result {
                        Ok(true) => String::new(),
                        Ok(false) => "对端拒绝接收：两台的同步码可能不一致".to_string(),
                        Err(err) => err,
                    },
                }
            })
            .collect()
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

    /// 后台线程出错时记一笔，前端能通过 lan_sync_error 读到。
    pub fn note_error(&self, message: String) {
        self.inner.lock().unwrap().last_error = message;
    }

    pub fn last_error(&self) -> String {
        self.inner.lock().unwrap().last_error.clone()
    }

    /// 体检：把"能不能同步"拆成几个可判断的事实。
    pub fn diagnose(&self) -> LanDiagnosis {
        let (room_configured, device, last_error, inbox) = {
            let inner = self.inner.lock().unwrap();
            (
                !inner.room.is_empty(),
                inner.device.clone(),
                inner.last_error.clone(),
                inner.inbox.len(),
            )
        };
        LanDiagnosis {
            started: self.started.load(Ordering::SeqCst),
            room_configured,
            device,
            service_port: SERVICE_PORT,
            discovery_port: DISCOVERY_PORT,
            local_addrs: local_addresses(),
            last_error,
            // 没开房间时别去握手，免得对端看到一个空房间的探测
            peers: if room_configured { self.probe() } else { Vec::new() },
            inbox,
        }
    }

    fn ensure_threads(&self) {
        if self.started.load(Ordering::SeqCst) {
            return;
        }
        // 先探一下端口：被另一个实例占着就别抢，直接把话说清楚。
        // 复制管理器常年挂在菜单栏，用户装完新版本往往没退旧版本，
        // 两个实例抢同一个端口时连接会被分流，表现就是时好时坏、多数 0 条。
        if let Ok(stream) = TcpStream::connect_timeout(
            &SocketAddr::from(([127, 0, 0, 1], SERVICE_PORT)),
            Duration::from_millis(200),
        ) {
            drop(stream);
            self.note_error(format!(
                "端口 {SERVICE_PORT} 被另一个 Xpaste 占用：请退出旧版本（菜单栏图标 → 退出）后再开启同步"
            ));
            return;
        }
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }
        self.inner.lock().unwrap().last_error.clear();
        // 两个线程都活到进程结束：一个守 TCP，一个喊话 + 收话。
        std::thread::Builder::new()
            .name("xpaste-lan-server".into())
            .spawn(move || {
                if let Err(err) = serve_loop() {
                    eprintln!("[lan-sync] {err}");
                    global().note_error(err);
                }
            })
            .ok();
        std::thread::Builder::new()
            .name("xpaste-lan-discovery".into())
            .spawn(move || {
                if let Err(err) = discovery_loop() {
                    eprintln!("[lan-sync] {err}");
                    global().note_error(err);
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
        // 手动登记的地址不按 TTL 淘汰：它存在的意义就是"广播收不到时也能连"，
        // 如果它自己也被广播的节奏决定生死，这条保底路径就形同虚设。
        self.peers
            .retain(|_, peer| peer.manual || peer.last_seen.elapsed() < PEER_TTL);
    }

    /// 手动登记的设备地址：宣告也要单播给它们，让对端反向认识我们。
    fn manual_addrs(&self) -> Vec<SocketAddr> {
        self.peers
            .iter()
            .filter(|(_, peer)| peer.manual)
            .filter_map(|(addr, _)| addr.parse::<SocketAddr>().ok())
            .collect()
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
    send_line_with(addr, line, PROBE_CONNECT_TIMEOUT)
}

fn send_line_with(
    addr: SocketAddr,
    line: &str,
    connect_timeout: Duration,
) -> Result<bool, String> {
    let target = addr;
    let mut stream = TcpStream::connect_timeout(&target, connect_timeout)
        .map_err(|err| format!("连接失败：{err}"))?;
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
    // 只开 SO_REUSEADDR（重启时不必等 TIME_WAIT），刻意不开 SO_REUSEPORT：
    // 一旦允许两个实例同时监听，内核会把连接分给别的实例——旧版本还在菜单栏
    // 里跑着时，推过去的数据就被那个"不认识当前房间"的进程静默拒绝，
    // 表现正是"发现了设备却一条也同步不过去"。
    socket
        .set_reuse_address(true)
        .map_err(|err| format!("设置端口复用失败：{err}"))?;
    socket
        .bind(&SocketAddr::from(([0, 0, 0, 0], port)).into())
        .map_err(|_err| format!("传输端口 {port} 被占用，可能还有另一个 Xpaste 在运行（旧版本没退出）"))?;
    socket
        .listen(64)
        .map_err(|err| format!("监听端口 {port} 失败：{err}"))?;
    Ok(socket.into())
}

// ── UDP 发现 ──────────────────────────────────────────────────

/// 这个地址是不是本机自己的。
///
/// 回环地址直接算本机；其余按网卡地址比对——同机多实例发出的广播会带上网卡 IP
/// （macOS 的同机广播不回环到监听端，但显式单播和组播回环会），两者都要挡住。
fn is_local_ip(ip: std::net::IpAddr) -> bool {
    if ip.is_loopback() {
        return true;
    }
    if_addrs::get_if_addrs()
        .map(|interfaces| interfaces.into_iter().any(|iface| iface.ip() == ip))
        .unwrap_or(false)
}

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

/// 解析用户手填的对端地址。
///
/// 允许 `192.168.1.8`（端口默认给 SERVICE_PORT）和 `192.168.1.8:47832` 两种写法，
/// 端口写错的情况也留着——总比"格式不对"让人无从下手好。
fn parse_peer_addr(raw: &str) -> Result<SocketAddr, String> {
    let text = raw.trim();
    if text.is_empty() {
        return Err("请填写对方的 IP 地址".to_string());
    }
    if let Ok(addr) = text.parse::<SocketAddr>() {
        return Ok(addr);
    }
    match text.parse::<Ipv4Addr>() {
        Ok(ip) => Ok(SocketAddr::from((ip, SERVICE_PORT))),
        Err(_) => Err("地址格式不对：应为 192.168.1.8 或 192.168.1.8:47832".to_string()),
    }
}

/// 本机在局域网里的 IPv4 地址（对方要填的就是这个）。
fn local_addresses() -> Vec<String> {
    let mut list = Vec::new();
    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for interface in interfaces {
            if interface.is_loopback() {
                continue;
            }
            if let if_addrs::IfAddr::V4(v4) = interface.addr {
                let ip = v4.ip.to_string();
                if !list.contains(&ip) {
                    list.push(ip);
                }
            }
        }
    }
    list.sort();
    list
}

fn discovery_loop() -> Result<(), String> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
        .map_err(|err| format!("创建发现套接字失败：{err}"))?;
    socket
        .set_reuse_address(true)
        .map_err(|err| format!("设置端口复用失败：{err}"))?;
    socket
        .bind(&SocketAddr::from(([0, 0, 0, 0], DISCOVERY_PORT)).into())
        .map_err(|_err| format!("发现端口 {DISCOVERY_PORT} 被占用，可能还有另一个 Xpaste 在运行"))?;
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
                // 手动登记的设备再单播一份：跨网段/AP 隔离时广播出不去，
                // 但单播只要路由能通就能到，让对端也能反向发现我们。
                for peer in global().inner.lock().unwrap().manual_addrs() {
                    let _ = udp.send_to(payload.as_bytes(), SocketAddr::new(peer.ip(), DISCOVERY_PORT));
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

                // 同一台机器上可能同时跑着多个实例（开发版和正式版、或者旧包没退），
                // 它们会互相收到对方的宣告。不挡掉的话界面会显示"发现 1 台设备"，
                // 实际对端就是自己——收发永远是 0 条，看起来像同步坏了。
                if is_local_ip(from.ip()) {
                    continue;
                }

                let mut inner = global().inner.lock().unwrap();
                if inner.room.is_empty() || packet.room != inner.room || packet.iid == inner.iid {
                    continue;
                }
                let addr = SocketAddr::new(from.ip(), packet.port).to_string();
                let is_new = !inner.peers.contains_key(&addr);
                // 之前手动填过同一台设备：保留 manual 标记，免得下一轮被 TTL 清掉
                let manual = inner.peers.get(&addr).map(|peer| peer.manual).unwrap_or(false);
                let device = if packet.device.is_empty() {
                    inner
                        .peers
                        .get(&addr)
                        .map(|peer| peer.device.clone())
                        .unwrap_or_default()
                } else {
                    packet.device.clone()
                };
                inner.peers.insert(
                    addr.clone(),
                    Peer {
                        device,
                        last_seen: Instant::now(),
                        manual,
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

/// 探测每个邻居的传输端口，返回逐个结果。
#[tauri::command]
pub fn lan_sync_probe() -> Vec<PeerProbe> {
    global().probe()
}

/// 手动登记一台设备（广播发现不到时的保底路径）。返回规范化后的 `ip:port`。
#[tauri::command]
pub fn lan_sync_add_peer(addr: String) -> Result<String, String> {
    global().add_peer(&addr)
}

/// 撤销一条手动登记。
#[tauri::command]
pub fn lan_sync_remove_peer(addr: String) -> bool {
    global().remove_peer(&addr)
}

/// 本机在局域网里的地址列表，界面上显示出来方便对方手填。
#[tauri::command]
pub fn lan_sync_local_addrs() -> Vec<String> {
    local_addresses()
}

/// 一键体检。
#[tauri::command]
pub fn lan_sync_diagnose() -> LanDiagnosis {
    global().diagnose()
}

/// 本机设备名，供同步记录打上"来自哪台电脑"的标签。
///
/// 优先用系统主机名（两台 Mac 默认都叫 "Mac" 就分不清了），拿不到再退回
/// 平台名。不改主机名的话它是稳定的，用来做标识正合适。
#[tauri::command]
pub fn device_label() -> String {
    let raw = std::env::var("COMPUTERNAME")
        .ok()
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .and_then(|out| String::from_utf8(out.stdout).ok())
        })
        .unwrap_or_default();

    let cleaned = raw.trim().trim_end_matches(".local").trim();
    let label: String = cleaned.chars().take(24).collect();
    if label.is_empty() {
        if cfg!(target_os = "macos") {
            "Mac".to_string()
        } else if cfg!(windows) {
            "PC".to_string()
        } else {
            "Device".to_string()
        }
    } else {
        label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 收端解析与回包的协议自测：同一房间收下、不同房间拒绝。
    ///
    /// 这条链路一旦破了，界面表现就是"发现了设备但一条也传不过去"，
    /// 和用户报的现象完全一样，所以必须有测试盯着。
    /// 用随机端口而不是 SERVICE_PORT，避免和正在运行的实例抢端口。
    #[test]
    fn push_respects_room() {
        global().configure("room-a", "Tester").unwrap();

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("绑定随机端口");
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                if let Ok(stream) = stream {
                    handle_conn(stream);
                }
            }
        });

        let record = LanRecord {
            id: "r1".to_string(),
            kind: "text".to_string(),
            iv: "AAAA".to_string(),
            ct: "BBBB".to_string(),
            ts: now_ms(),
        };

        let foreign = serde_json::to_string(&Packet {
            op: "push".to_string(),
            room: "room-b".to_string(),
            device: "Other".to_string(),
            port: SERVICE_PORT,
            iid: String::new(),
            items: vec![record.clone()],
        })
        .unwrap();
        assert_eq!(
            send_line(addr, &foreign),
            Ok(false),
            "房间不一致的记录必须被拒绝"
        );

        let same = serde_json::to_string(&Packet {
            op: "push".to_string(),
            room: "room-a".to_string(),
            device: "Other".to_string(),
            port: SERVICE_PORT,
            iid: String::new(),
            items: vec![record.clone()],
        })
        .unwrap();
        assert_eq!(send_line(addr, &same), Ok(true), "同房间的记录必须被接收");

        let taken = global().take();
        assert_eq!(taken.len(), 1, "收件箱里应当正好有一条");
        assert_eq!(taken[0].id, "r1");
    }

    #[test]
    fn device_label_is_not_empty() {
        assert!(!device_label().trim().is_empty());
    }

    /// 手填地址是"广播被拦"时的唯一保底路径，解析错一次就等于同步不了。
    #[test]
    fn parse_peer_addr_accepts_both_forms() {
        assert_eq!(
            parse_peer_addr(" 192.168.1.8 ").unwrap().to_string(),
            format!("192.168.1.8:{SERVICE_PORT}")
        );
        assert_eq!(
            parse_peer_addr("10.0.0.9:47900").unwrap().to_string(),
            "10.0.0.9:47900"
        );
        assert!(parse_peer_addr("").is_err());
        assert!(parse_peer_addr("mac-mini.local").is_err());
    }

    /// 手动登记的地址不能被 TTL 淘汰：它本来就是给"收不到广播"的场景兜底的。
    #[test]
    fn manual_peer_survives_prune() {
        let mut inner = Inner::default();
        inner.peers.insert(
            "192.168.1.9:47832".to_string(),
            Peer {
                device: String::new(),
                // 超过 PEER_TTL，模拟它没有在广播
                last_seen: Instant::now() - PEER_TTL - Duration::from_secs(1),
                manual: true,
            },
        );
        inner.prune();
        assert!(inner.peers.contains_key("192.168.1.9:47832"));

        inner.peers.insert(
            "192.168.1.10:47832".to_string(),
            Peer {
                device: "Auto".to_string(),
                last_seen: Instant::now() - PEER_TTL - Duration::from_secs(1),
                manual: false,
            },
        );
        inner.prune();
        assert!(!inner.peers.contains_key("192.168.1.10:47832"));
    }
}
