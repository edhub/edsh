pub mod protocol {
    /// The ALPN (Application-Layer Protocol Negotiation) used for edsh connections.
    pub const EDSH_ALPN: &[u8] = b"edsh/1";
    pub const IS_ALPN: &[u8] = b"/iroh/ssh"; // 兼容 iroh-ssh 的服务协议
}
