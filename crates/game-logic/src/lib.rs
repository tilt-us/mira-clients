#![deny(unsafe_code)]
#![warn(missing_docs)]

//! Reusable gameplay systems that can run in both client and server apps.
//!
//! This crate stays free of rendering, UI, and client-only world setup so it can be
//! registered by the dedicated server without requiring Bevy render resources.

mod systems;
pub use systems::{
    MiraClientSystemsPlugin, MiraGameplaySystemsPlugin, MiraHudState, MiraSystemsPlugin,
};
