//! How two machines speak to each other, and how each knows who the other is.
//!
//! [`tls`] answers *who* — a self-signed certificate a machine keeps, and the fingerprint the
//! other end pins at pairing. [`wire`] answers *how* — a door that speaks TLS and the small,
//! deliberately incomplete HTTP the two programs actually exchange.
//!
//! Both halves are here rather than in either program, because a handshake implemented twice is
//! two handshakes.

pub mod tls;
pub mod wire;
