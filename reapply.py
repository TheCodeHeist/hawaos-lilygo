with open("companion/src/main.rs", "r") as f:
    content = f.read()

# 1. Imports
old_import = "use std::net::{SocketAddr, TcpListener, UdpSocket};"
new_import = "use std::net::{SocketAddr, TcpListener, UdpSocket};"
# no change needed for imports

# 2. Helpers and load_config / save_config
old_cfg = """fn load_config() -> CompanionConfig {
    let path = config_file_path();
    if let Ok(bytes) = fs::read(&path) {
        if let Ok(mut cfg) = serde_json::from_slice::<CompanionConfig>(&bytes) {
            cfg.autostart_enabled = is_autostart_installed();
            return cfg;
        }
    }
    CompanionConfig::default()
}

fn save_config(cfg: &CompanionConfig) {
    let path = config_file_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = fs::write(path, json);
    }
}"""

new_cfg = """fn sanitize_ip_str(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(idx) = trimmed.find(|c: char| c.is_whitespace() || c == '|' || c == ',' || c == ';') {
        trimmed[..idx].trim().to_string()
    } else {
        trimmed.to_string()
    }
}

fn get_local_lan_ip() -> String {
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    String::new()
}

fn load_config() -> CompanionConfig {
    let path = config_file_path();
    if let Ok(bytes) = fs::read(&path) {
        if let Ok(mut cfg) = serde_json::from_slice::<CompanionConfig>(&bytes) {
            cfg.target_ip = sanitize_ip_str(&cfg.target_ip);
            cfg.autostart_enabled = is_autostart_installed();
            return cfg;
        }
    }
    CompanionConfig::default()
}

fn save_config(cfg: &CompanionConfig) {
    let mut clean_cfg = cfg.clone();
    clean_cfg.target_ip = sanitize_ip_str(&clean_cfg.target_ip);
    let path = config_file_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(&clean_cfg) {
        let _ = fs::write(path, json);
    }
}"""
assert old_cfg in content, "old_cfg not found"
content = content.replace(old_cfg, new_cfg, 1)

# 3. StatsMsg and MediaMsg
old_wire = """#[derive(Serialize)]
struct StatsMsg<'a> {
    #[serde(rename = "type")]
    msg_type: &'a str,
    ts: u64,
    cpu: u8,
    ram: u8,
    net_up_kbps: u32,
    net_down_kbps: u32,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
struct MediaMsg {
    #[serde(rename = "type")]
    msg_type: String,
    state: String,
    title: String,
    artist: String,
    album: String,
    pos_ms: u32,
    dur_ms: u32,
    ts: u64,
    art: String,
    bg_top: String,
    bg_bottom: String,
    ink: String,
}

impl Default for MediaMsg {
    fn default() -> Self {
        Self {
            msg_type: "media".to_string(),
            state: "stopped".to_string(),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            pos_ms: 0,
            dur_ms: 0,
            ts: 0,
            art: String::new(),
            bg_top: "#071115".to_string(),
            bg_bottom: "#05090b".to_string(),
            ink: "light".to_string(),
        }
    }
}"""

new_wire = """#[derive(Serialize)]
struct StatsMsg<'a> {
    #[serde(rename = "type")]
    msg_type: &'a str,
    ts: u64,
    cpu: u8,
    ram: u8,
    net_up_kbps: u32,
    net_down_kbps: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    host_ip: Option<&'a str>,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
struct MediaMsg {
    #[serde(rename = "type")]
    msg_type: String,
    state: String,
    title: String,
    artist: String,
    album: String,
    pos_ms: u32,
    dur_ms: u32,
    ts: u64,
    art: String,
    bg_top: String,
    bg_bottom: String,
    ink: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    host_ip: String,
}

impl Default for MediaMsg {
    fn default() -> Self {
        Self {
            msg_type: "media".to_string(),
            state: "stopped".to_string(),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            pos_ms: 0,
            dur_ms: 0,
            ts: 0,
            art: String::new(),
            bg_top: "#071115".to_string(),
            bg_bottom: "#05090b".to_string(),
            ink: "light".to_string(),
            host_ip: String::new(),
        }
    }
}"""
assert old_wire in content, "old_wire not found"
content = content.replace(old_wire, new_wire, 1)

# 4. query_mpris_media setting host_ip
old_mpris = """                        bg_top,
                        bg_bottom,
                        ink,
                    },"""
new_mpris = """                        bg_top,
                        bg_bottom,
                        ink,
                        host_ip: get_local_lan_ip(),
                    },"""
assert old_mpris in content, "old_mpris not found"
content = content.replace(old_mpris, new_mpris, 1)

# 5. Connected! IP= and config_ack
old_handle_ip = """    // Check if serial log line reports WiFi connection IP
    if let Some(idx) = trimmed.find("Connected! IP=") {
        let ip_part = trimmed[idx + "Connected! IP=".len()..].trim();
        if !ip_part.is_empty() && ip_part != "0.0.0.0" {
            if let Ok(mut g) = state.lock() {
                g.device_reported_ip = Some(ip_part.to_string());
                g.device_wifi_connected = Some(true);
                g.config.target_ip = ip_part.to_string();
                save_config(&g.config);
                g.push_log(format!(
                    "Device connected to WiFi! Assigned IP: {}",
                    ip_part
                ));
            }
        }
        return;
    }"""

new_handle_ip = """    // Check if serial log line reports WiFi connection IP
    if let Some(idx) = trimmed.find("Connected! IP=") {
        let ip_part = trimmed[idx + "Connected! IP=".len()..].trim();
        let ip_clean = sanitize_ip_str(ip_part);
        if !ip_clean.is_empty() && ip_clean != "0.0.0.0" {
            if let Ok(mut g) = state.lock() {
                g.device_reported_ip = Some(ip_clean.clone());
                g.device_wifi_connected = Some(true);
                g.config.target_ip = ip_clean.clone();
                save_config(&g.config);
                g.push_log(format!(
                    "Device connected to WiFi! Assigned IP: {}",
                    ip_clean
                ));
            }
        }
        return;
    }"""
assert old_handle_ip in content, "old_handle_ip not found"
content = content.replace(old_handle_ip, new_handle_ip, 1)

old_ack_ip = """                if let Some(ip) = pkt.ip {
                    if !ip.is_empty() && ip != "0.0.0.0" {
                        g.device_reported_ip = Some(ip.clone());
                    }
                }"""
new_ack_ip = """                if let Some(ip) = pkt.ip {
                    let ip_clean = sanitize_ip_str(&ip);
                    if !ip_clean.is_empty() && ip_clean != "0.0.0.0" {
                        g.device_reported_ip = Some(ip_clean.clone());
                        g.config.target_ip = ip_clean;
                        save_config(&g.config);
                    }
                }"""
assert old_ack_ip in content, "old_ack_ip not found"
content = content.replace(old_ack_ip, new_ack_ip, 1)

# 6. actions target_addr
old_act_addr = """            for (act, cfg) in actions {
                let target_addr: Option<SocketAddr> =
                    format!("{}:{}", cfg.target_ip.trim(), UDP_PORT)
                        .parse()
                        .ok();"""
new_act_addr = """            for (act, cfg) in actions {
                let clean_ip = sanitize_ip_str(&cfg.target_ip);
                let target_addr: Option<SocketAddr> =
                    format!("{}:{}", clean_ip, UDP_PORT)
                        .parse()
                        .ok();"""
assert old_act_addr in content, "old_act_addr not found"
content = content.replace(old_act_addr, new_act_addr, 1)

# 7. stats_msg target_addr and host_ip
old_stats_send = """                let target_addr: Option<SocketAddr> =
                    format!("{}:{}", target_ip.trim(), UDP_PORT).parse().ok();

                let stats_msg = StatsMsg {
                    msg_type: "stats",
                    ts,
                    cpu: cpu_pct,
                    ram: ram_pct,
                    net_up_kbps,
                    net_down_kbps,
                };"""
new_stats_send = """                let clean_ip = sanitize_ip_str(&target_ip);
                let target_addr: Option<SocketAddr> =
                    format!("{}:{}", clean_ip, UDP_PORT).parse().ok();

                let local_ip_str = get_local_lan_ip();
                let stats_msg = StatsMsg {
                    msg_type: "stats",
                    ts,
                    cpu: cpu_pct,
                    ram: ram_pct,
                    net_up_kbps,
                    net_down_kbps,
                    host_ip: if local_ip_str.is_empty() {
                        None
                    } else {
                        Some(&local_ip_str)
                    },
                };"""
assert old_stats_send in content, "old_stats_send not found"
content = content.replace(old_stats_send, new_stats_send, 1)

# 8. save-target-ip-btn
old_save_ip = """                                        let new_ip =
                                            this.ip_input.read(cx).value().trim().to_string();
                                        if !new_ip.is_empty() {"""
new_save_ip = """                                        let new_ip = sanitize_ip_str(
                                            this.ip_input.read(cx).value().trim(),
                                        );
                                        if !new_ip.is_empty() {"""
assert old_save_ip in content, "old_save_ip not found"
content = content.replace(old_save_ip, new_save_ip, 1)

with open("companion/src/main.rs", "w") as f:
    f.write(content)
print("Updated companion/src/main.rs successfully!")
