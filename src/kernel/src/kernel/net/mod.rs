//! 网络子系统（net.md）
//! 套接字是IO流内核对象；TCP/UDP协议栈在内核；支持tap虚拟网卡
use crate::sync::Mutex;
use crate::glue_layer::SysErr;
use crate::ipc::WaitQueue;
use hashbrown::HashMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SocketType {
    Tcp,
    Udp,
    Raw,
    Tap, // 虚拟网卡（对接虚拟机 vm.md）
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SocketState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

/// 套接字对象（net.md：IO流内核对象）
#[derive(Debug)]
pub struct Socket {
    pub id: u64,
    pub sock_type: SocketType,
    pub state: Mutex<SocketState>,
    pub local_addr: Mutex<[u8; 4]>,
    pub local_port: Mutex<u16>,
    pub remote_addr: Mutex<[u8; 4]>,
    pub remote_port: Mutex<u16>,
    /// 接收队列（IPC等待可读条目）
    pub rx_queue: WaitQueue,
    /// 发送队列
    pub tx_queue: WaitQueue,
    /// 接收缓冲区
    pub rx_buf: Mutex<VecDeque<u8>>,
    /// 发送缓冲区
    pub tx_buf: Mutex<VecDeque<u8>>,
    /// 关联的进程PID
    pub owner_pid: Mutex<u64>,
}

use alloc::collections::VecDeque;

impl Socket {
    pub fn new(id: u64, sock_type: SocketType) -> Self {
        Self {
            id,
            sock_type,
            state: Mutex::new(SocketState::Closed),
            local_addr: Mutex::new([0; 4]),
            local_port: Mutex::new(0),
            remote_addr: Mutex::new([0; 4]),
            remote_port: Mutex::new(0),
            rx_queue: WaitQueue::new(),
            tx_queue: WaitQueue::new(),
            rx_buf: Mutex::new(VecDeque::new()),
            tx_buf: Mutex::new(VecDeque::new()),
            owner_pid: Mutex::new(0),
        }
    }

    /// 对象路径：/object/net/sock/<id>
    pub fn object_path(&self) -> String {
        alloc::format!("/object/net/sock/{}", self.id)
    }

    /// 读取数据（阻塞等待IPC事件）
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, SysErr> {
        let mut rx = self.rx_buf.lock();
        if rx.is_empty() {
            // 阻塞：加入等待队列，进程休眠
            // self.rx_queue.push(current_pid);
            // schedule_block();
            return Err(SysErr::EINVAL); // 简化：非阻塞返回
        }
        let mut count = 0;
        while count < buf.len() && let Some(b) = rx.pop_front() {
            buf[count] = b;
            count += 1;
        }
        Ok(count)
    }

    /// 写入数据
    pub fn write(&self, data: &[u8]) -> Result<usize, SysErr> {
        let mut tx = self.tx_buf.lock();
        for b in data {
            tx.push_back(*b);
        }
        // 唤醒发送处理
        self.tx_queue.wake_one();
        Ok(data.len())
    }
}

/// 网络管理器
pub struct NetManager {
    pub sockets: Mutex<HashMap<u64, Arc<Socket>>>,
    pub next_sock_id: Mutex<u64>,
    /// tap虚拟网卡表（对接虚拟机）
    pub tap_interfaces: Mutex<HashMap<String, Arc<Socket>>>,
}

impl NetManager {
    pub fn new() -> Self {
        Self {
            sockets: Mutex::new(HashMap::new()),
            next_sock_id: Mutex::new(0),
            tap_interfaces: Mutex::new(HashMap::new()),
        }
    }

    /// 创建套接字（返回内核对象，用户态通过对象API操作）
    pub fn create_socket(&self, sock_type: SocketType) -> Arc<Socket> {
        let mut id = self.next_sock_id.lock();
        *id += 1;
        let sock_id = *id;
        let sock = Arc::new(Socket::new(sock_id, sock_type));
        self.sockets.lock().insert(sock_id, sock.clone());
        // 创建内核对象 /object/net/sock/<id>
        let _ = crate::object::create_object(&sock.object_path());
        sock
    }

    /// 创建tap虚拟网卡（vm.md虚拟机使用）
    pub fn create_tap(&self, name: &str) -> Result<Arc<Socket>, SysErr> {
        let sock = self.create_socket(SocketType::Tap);
        self.tap_interfaces.lock().insert(String::from(name), sock.clone());
        crate::log::info(&alloc::format!("tap interface created: {}", name));
        Ok(sock)
    }

    /// 关闭套接字
    pub fn close_socket(&self, id: u64) -> Result<(), SysErr> {
        let sock = self.sockets.lock().remove(&id).ok_or(SysErr::ENOENT)?;
        *sock.state.lock() = SocketState::Closed;
        Ok(())
    }
}

use crate::sync::Once;
static NET_MGR: Once<NetManager> = Once::new();

pub fn init() {
    let mgr = NetManager::new();
    NET_MGR.call_once(|| mgr);
    PROTO_STACK.call_once(|| ProtocolStack::new());
    let _ = crate::object::create_object("/object/net");
    let _ = crate::object::create_object("/object/net/sock");
    crate::println!("network subsystem ready, /object/net/sock created");
}

pub fn manager() -> &'static NetManager {
    NET_MGR.get().expect("net manager not initialized")
}

// ==================== 协议层（方案B：从 Go netstack 移植，net.md）====================

/// 标准 Internet 校验和（一补码）
pub fn internet_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < data.len() {
        sum += ((data[i] as u32) << 8) | (data[i + 1] as u32);
        i += 2;
    }
    if data.len() % 2 == 1 {
        sum += (data[data.len() - 1] as u32) << 8;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// IPv4 协议号
pub const PROTO_ICMP: u8 = 1;
pub const PROTO_TCP: u8 = 6;
pub const PROTO_UDP: u8 = 17;

/// IPv4 头（RFC 791，20 字节）
#[derive(Debug, Clone, Copy, Default)]
pub struct IPv4Header {
    pub tos: u8,
    pub total_len: u16,
    pub id: u16,
    pub flags_frag: u16,
    pub ttl: u8,
    pub protocol: u8,
    pub checksum: u16,
    pub src_ip: [u8; 4],
    pub dst_ip: [u8; 4],
}

impl IPv4Header {
    /// 编码为完整 IPv4 报文
    pub fn encode(&self, payload: &[u8]) -> Vec<u8> {
        let mut pkt = vec![0u8; 20 + payload.len()];
        pkt[0] = 0x45;
        pkt[1] = self.tos;
        pkt[2] = (self.total_len >> 8) as u8;
        pkt[3] = (self.total_len & 0xFF) as u8;
        pkt[4] = (self.id >> 8) as u8;
        pkt[5] = (self.id & 0xFF) as u8;
        pkt[6] = (self.flags_frag >> 8) as u8;
        pkt[7] = (self.flags_frag & 0xFF) as u8;
        pkt[8] = self.ttl;
        pkt[9] = self.protocol;
        pkt[12..16].copy_from_slice(&self.src_ip);
        pkt[16..20].copy_from_slice(&self.dst_ip);
        let c = internet_checksum(&pkt[..20]);
        pkt[10..12].copy_from_slice(&c.to_be_bytes());
        pkt[20..].copy_from_slice(payload);
        pkt
    }

    /// 解析 IPv4 报文 -> (header, payload)
    pub fn parse(pkt: &[u8]) -> Option<(IPv4Header, &[u8])> {
        if pkt.len() < 20 {
            return None;
        }
        let h = IPv4Header {
            tos: pkt[1],
            total_len: u16::from_be_bytes([pkt[2], pkt[3]]),
            id: u16::from_be_bytes([pkt[4], pkt[5]]),
            flags_frag: u16::from_be_bytes([pkt[6], pkt[7]]),
            ttl: pkt[8],
            protocol: pkt[9],
            checksum: u16::from_be_bytes([pkt[10], pkt[11]]),
            src_ip: [pkt[12], pkt[13], pkt[14], pkt[15]],
            dst_ip: [pkt[16], pkt[17], pkt[18], pkt[19]],
        };
        let ihl = ((pkt[0] & 0x0F) as usize) * 4;
        if ihl < 20 || pkt.len() < ihl {
            return None;
        }
        Some((h, &pkt[ihl..]))
    }
}

/// UDP 头（RFC 768，8 字节）
#[derive(Debug, Clone, Copy, Default)]
pub struct UDPHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
}

impl UDPHeader {
    pub fn encode(&self, payload: &[u8]) -> Vec<u8> {
        let total = 8 + payload.len();
        let mut b = vec![0u8; total];
        b[0..2].copy_from_slice(&self.src_port.to_be_bytes());
        b[2..4].copy_from_slice(&self.dst_port.to_be_bytes());
        b[4..6].copy_from_slice(&(total as u16).to_be_bytes());
        b[6..8].copy_from_slice(&self.checksum.to_be_bytes());
        b[8..].copy_from_slice(payload);
        b
    }
    pub fn parse(b: &[u8]) -> Option<(UDPHeader, &[u8])> {
        if b.len() < 8 {
            return None;
        }
        let h = UDPHeader {
            src_port: u16::from_be_bytes([b[0], b[1]]),
            dst_port: u16::from_be_bytes([b[2], b[3]]),
            length: u16::from_be_bytes([b[4], b[5]]),
            checksum: u16::from_be_bytes([b[6], b[7]]),
        };
        Some((h, &b[8..]))
    }
}

/// UDP 伪头校验和（RFC 768）
pub fn udp_checksum(src: [u8; 4], dst: [u8; 4], uh: &[u8], payload: &[u8]) -> u16 {
    let udp_len = uh.len() + payload.len();
    let mut ph = [0u8; 12];
    ph[0..4].copy_from_slice(&src);
    ph[4..8].copy_from_slice(&dst);
    ph[9] = PROTO_UDP;
    ph[10..12].copy_from_slice(&(udp_len as u16).to_be_bytes());
    let mut sum: u32 = 0;
    let mut add = |d: &[u8]| {
        let mut i = 0;
        while i + 1 < d.len() {
            sum += ((d[i] as u32) << 8) | (d[i + 1] as u32);
            i += 2;
        }
        if d.len() % 2 == 1 {
            sum += (d[d.len() - 1] as u32) << 8;
        }
    };
    add(&ph);
    add(uh);
    add(payload);
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    let c = !(sum as u16);
    if c == 0 {
        0xFFFF
    } else {
        c
    }
}

/// TCP 控制位（RFC 793 byte13）
pub const TCP_FIN: u8 = 0x01;
pub const TCP_SYN: u8 = 0x02;
pub const TCP_RST: u8 = 0x04;
pub const TCP_PSH: u8 = 0x08;
pub const TCP_ACK: u8 = 0x10;

/// ARP 表项（IP -> MAC）
#[derive(Debug, Clone, Copy)]
pub struct ARPEntry {
    pub ip: [u8; 4],
    pub mac: [u8; 6],
}

pub struct ARPTable {
    pub entries: Vec<ARPEntry>,
}

impl ARPTable {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }
    pub fn lookup(&self, ip: [u8; 4]) -> Option<[u8; 6]> {
        for e in &self.entries {
            if e.ip == ip {
                return Some(e.mac);
            }
        }
        None
    }
    pub fn insert(&mut self, ip: [u8; 4], mac: [u8; 6]) {
        for e in &mut self.entries {
            if e.ip == ip {
                e.mac = mac;
                return;
            }
        }
        self.entries.push(ARPEntry { ip, mac });
    }
}

pub const ARP_HW_ETHERNET: u8 = 1;
pub const ARP_PROTO_IPV4: u16 = 0x0800;
pub const ARP_REQUEST: u16 = 1;
pub const ARP_REPLY: u16 = 2;

/// 构造 ARP 报文（28 字节）
pub fn arp_encode(op: u16, sha: [u8; 6], spa: [u8; 4], tha: [u8; 6], tpa: [u8; 4]) -> Vec<u8> {
    let mut p = vec![0u8; 28];
    p[1] = ARP_HW_ETHERNET;
    p[2..4].copy_from_slice(&ARP_PROTO_IPV4.to_be_bytes());
    p[4] = 6;
    p[5] = 4;
    p[6..8].copy_from_slice(&op.to_be_bytes());
    p[8..14].copy_from_slice(&sha);
    p[14..18].copy_from_slice(&spa);
    p[18..24].copy_from_slice(&tha);
    p[24..28].copy_from_slice(&tpa);
    p
}

/// 内核网络协议栈分发（net.md §1）：以太网帧 -> ARP/IP -> 传输层 -> socket rx
pub struct ProtocolStack {
    pub arp: Mutex<ARPTable>,
    pub local_ip: [u8; 4],
}

impl ProtocolStack {
    pub fn new() -> Self {
        Self {
            arp: Mutex::new(ARPTable::new()),
            local_ip: [10, 0, 2, 15],
        }
    }

    fn find_udp(&self, dst_port: u16) -> Option<Arc<Socket>> {
        let mgr = manager();
        for (_, s) in mgr.sockets.lock().iter() {
            if s.sock_type == SocketType::Udp && *s.local_port.lock() == dst_port {
                return Some(s.clone());
            }
        }
        None
    }

    /// 路由 IPv4 payload 到传输层与 socket
    pub fn handle_ip_route(&self, proto: u8, src: [u8; 4], _dst: [u8; 4], payload: &[u8]) {
        match proto {
            PROTO_UDP => {
                if let Some((h, data)) = UDPHeader::parse(payload) {
                    if let Some(sock) = self.find_udp(h.dst_port) {
                        *sock.remote_addr.lock() = src;
                        *sock.remote_port.lock() = h.src_port;
                        let mut rx = sock.rx_buf.lock();
                        for &b in data {
                            rx.push_back(b);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// 处理原始以太网帧（net.md: NIC -> stack）
    pub fn handle_ethernet_frame(&self, frame: &[u8]) {
        if frame.len() < 14 {
            return;
        }
        let ether_type = u16::from_be_bytes([frame[12], frame[13]]);
        let payload = &frame[14..];
        match ether_type {
            0x0806 => {
                // ARP
                if payload.len() >= 28 {
                    let op = u16::from_be_bytes([payload[6], payload[7]]);
                    let spa = [payload[14], payload[15], payload[16], payload[17]];
                    if op == ARP_REQUEST {
                        self.arp.lock().insert(spa, [0; 6]);
                    }
                }
            }
            0x0800 => {
                // IPv4
                if let Some((h, data)) = IPv4Header::parse(payload) {
                    self.handle_ip_route(h.protocol, h.src_ip, h.dst_ip, data);
                }
            }
            _ => {}
        }
    }
}

static PROTO_STACK: Once<ProtocolStack> = Once::new();

pub fn proto_stack() -> &'static ProtocolStack {
    PROTO_STACK.get().expect("proto stack not initialized")
}

// ==================== TCP 状态机（RFC 793，移植自 Go tcp.go）====================

impl Socket {
    /// 主动打开：Closed -> SynSent
    pub fn active_open(&self) {
        let mut s = self.state.lock();
        if self.sock_type != SocketType::Tcp || *s != SocketState::Closed {
            return;
        }
        *s = SocketState::SynSent;
    }

    /// 被动打开（监听）：Closed -> Listen
    pub fn passive_open(&self) {
        let mut s = self.state.lock();
        if self.sock_type != SocketType::Tcp || *s != SocketState::Closed {
            return;
        }
        *s = SocketState::Listen;
    }

    /// 应用关闭：Established->FinWait1 / CloseWait->LastAck
    pub fn tcp_close(&self) {
        let mut s = self.state.lock();
        match *s {
            SocketState::Established => *s = SocketState::FinWait1,
            SocketState::CloseWait => *s = SocketState::LastAck,
            _ => {}
        }
    }

    /// 应用 TCP 段到状态机（flags = byte13 控制位）
    pub fn handle_segment(&self, flags: u8) {
        let mut s = self.state.lock();
        let cur = *s;
        match cur {
            SocketState::Listen => {
                if flags & TCP_SYN != 0 {
                    *s = SocketState::SynReceived;
                }
            }
            SocketState::SynSent => {
                if flags & TCP_SYN != 0 && flags & TCP_ACK != 0 {
                    *s = SocketState::Established;
                } else if flags & TCP_RST != 0 {
                    *s = SocketState::Closed;
                }
            }
            SocketState::SynReceived => {
                if flags & TCP_ACK != 0 {
                    *s = SocketState::Established;
                }
            }
            SocketState::Established => {
                if flags & TCP_FIN != 0 {
                    *s = SocketState::CloseWait;
                }
            }
            SocketState::FinWait1 => {
                if flags & TCP_FIN != 0 && flags & TCP_ACK != 0 {
                    *s = SocketState::TimeWait;
                } else if flags & TCP_ACK != 0 {
                    *s = SocketState::FinWait2;
                }
            }
            SocketState::FinWait2 => {
                if flags & TCP_FIN != 0 {
                    *s = SocketState::TimeWait;
                }
            }
            SocketState::LastAck => {
                if flags & TCP_ACK != 0 {
                    *s = SocketState::Closed;
                }
            }
            SocketState::Closing => {
                if flags & TCP_ACK != 0 {
                    *s = SocketState::TimeWait;
                }
            }
            _ => {}
        }
    }
}
