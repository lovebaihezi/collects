mod is_api_health;

pub use is_api_health::APIStatusSignal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Effects {
    APIStatus,
}
