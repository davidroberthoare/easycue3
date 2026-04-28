//! DMX output and universe management
//!
//! This module handles DMX512 universe abstraction and multiple backend
//! implementations (Virtual, USB, Art-Net).

pub mod universe;
pub mod backends;

pub use universe::Universe;
