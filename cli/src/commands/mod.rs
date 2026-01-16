//! Command implementations for the Collects CLI.
//!
//! Each subcommand is implemented in its own module for better organization.

pub mod add;
pub mod completions;
pub mod list;
pub mod login;
pub mod new;
pub mod schema;
pub mod view;

pub use add::run_add;
pub use completions::generate_completions;
pub use list::run_list;
pub use login::run_login;
pub use new::run_new;
pub use schema::print_schema;
pub use view::run_view;
