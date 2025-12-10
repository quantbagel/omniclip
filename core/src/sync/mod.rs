//! TCP-based peer synchronization

pub mod connection;
pub mod framing;
pub mod server;

pub use connection::PeerConnection;
pub use framing::{read_framed_message, write_framed_message};
pub use server::{PairedDevice, SyncEvent, SyncServer, SyncServerHandle};
