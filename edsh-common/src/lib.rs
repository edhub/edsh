pub mod protocol {
    use serde::{Deserialize, Serialize};

    /// The ALPN (Application-Layer Protocol Negotiation) used for edsh connections.
    pub const EDSH_ALPN: &[u8] = b"edsh/1";

    #[derive(Debug, Serialize, Deserialize)]
    pub enum Message {
        // Define protocol messages here if needed for handshaking
        Connect,
    }
}
