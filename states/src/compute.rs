use std::any::Any;

use crate::Reg;

pub trait Compute: Any {
    const TYPE: &'static str = "compute";
    const ID: Reg;
    const DEPS: &'static [Reg];
}
