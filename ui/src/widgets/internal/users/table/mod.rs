//! Table components for internal users management.
//!
//! This module contains the table rendering logic split into smaller,
//! focused components:
//! - `columns`: Column definitions and widths
//! - `header`: Table header rendering
//! - `row`: Individual row rendering with cells
//! - `cells`: Cell rendering functions for each column type

mod cells;
pub mod columns;
pub mod header;
pub mod row;
