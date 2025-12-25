//! User management module.
//!
//! This module provides user-related functionality including:
//! - OTP (One-Time Password) authentication setup and verification
//! - User creation API endpoints

pub mod otp;
pub mod routes;

pub use routes::{auth_routes, internal_routes};
