use std::{any::Any, collections::BTreeMap};

#[derive(Debug)]
pub enum BasicStates {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    F32(f32),
    F64(f64),
    Bool(bool),
    String(String),
    Array(Vec<BasicStates>),
    Object(BTreeMap<String, BasicStates>),
    Any(Box<dyn Any>),
}

impl TryInto<u8> for &BasicStates {
    type Error = &'static str;

    fn try_into(self) -> Result<u8, Self::Error> {
        if let &BasicStates::U8(value) = self {
            Ok(value)
        } else {
            Err("BasicSates is not a u8")
        }
    }
}

impl TryInto<u16> for &BasicStates {
    type Error = &'static str;

    fn try_into(self) -> Result<u16, Self::Error> {
        if let &BasicStates::U16(value) = self {
            Ok(value)
        } else {
            Err("BasicSates is not a u16")
        }
    }
}

impl TryInto<u32> for &BasicStates {
    type Error = &'static str;

    fn try_into(self) -> Result<u32, Self::Error> {
        if let &BasicStates::U32(value) = self {
            Ok(value)
        } else {
            Err("BasicSates is not a u32")
        }
    }
}

impl TryInto<u64> for &BasicStates {
    type Error = &'static str;

    fn try_into(self) -> Result<u64, Self::Error> {
        if let &BasicStates::U64(value) = self {
            Ok(value)
        } else {
            Err("BasicSates is not a u64")
        }
    }
}

impl TryInto<u128> for &BasicStates {
    type Error = &'static str;

    fn try_into(self) -> Result<u128, Self::Error> {
        if let &BasicStates::U128(value) = self {
            Ok(value)
        } else {
            Err("BasicSates is not a u128")
        }
    }
}

impl TryInto<i8> for &BasicStates {
    type Error = &'static str;

    fn try_into(self) -> Result<i8, Self::Error> {
        if let &BasicStates::I8(value) = self {
            Ok(value)
        } else {
            Err("BasicSates is not an i8")
        }
    }
}

impl TryInto<i16> for &BasicStates {
    type Error = &'static str;

    fn try_into(self) -> Result<i16, Self::Error> {
        if let &BasicStates::I16(value) = self {
            Ok(value)
        } else {
            Err("BasicSates is not an i16")
        }
    }
}

impl TryInto<i32> for &BasicStates {
    type Error = &'static str;

    fn try_into(self) -> Result<i32, Self::Error> {
        if let &BasicStates::I32(value) = self {
            Ok(value)
        } else {
            Err("BasicSates is not an i32")
        }
    }
}

impl TryInto<i64> for &BasicStates {
    type Error = &'static str;

    fn try_into(self) -> Result<i64, Self::Error> {
        if let &BasicStates::I64(value) = self {
            Ok(value)
        } else {
            Err("BasicSates is not an i64")
        }
    }
}

impl TryInto<i128> for &BasicStates {
    type Error = &'static str;

    fn try_into(self) -> Result<i128, Self::Error> {
        if let &BasicStates::I128(value) = self {
            Ok(value)
        } else {
            Err("BasicSates is not an i128")
        }
    }
}

impl TryInto<f32> for &BasicStates {
    type Error = &'static str;

    fn try_into(self) -> Result<f32, Self::Error> {
        if let &BasicStates::F32(value) = self {
            Ok(value)
        } else {
            Err("BasicSates is not an f32")
        }
    }
}

impl TryInto<f64> for &BasicStates {
    type Error = &'static str;

    fn try_into(self) -> Result<f64, Self::Error> {
        if let &BasicStates::F64(value) = self {
            Ok(value)
        } else {
            Err("BasicSates is not an f64")
        }
    }
}

impl TryInto<bool> for &BasicStates {
    type Error = &'static str;

    fn try_into(self) -> Result<bool, Self::Error> {
        if let &BasicStates::Bool(value) = self {
            Ok(value)
        } else {
            Err("BasicSates is not a bool")
        }
    }
}

impl TryInto<String> for BasicStates {
    type Error = &'static str;

    fn try_into(self) -> Result<String, Self::Error> {
        if let BasicStates::String(value) = self {
            Ok(value)
        } else {
            Err("BasicSates is not a String")
        }
    }
}

impl TryInto<Vec<BasicStates>> for BasicStates {
    type Error = &'static str;

    fn try_into(self) -> Result<Vec<BasicStates>, Self::Error> {
        if let BasicStates::Array(value) = self {
            Ok(value)
        } else {
            Err("BasicSates is not an Array")
        }
    }
}

impl TryInto<BTreeMap<String, BasicStates>> for BasicStates {
    type Error = &'static str;

    fn try_into(self) -> Result<BTreeMap<String, BasicStates>, Self::Error> {
        if let BasicStates::Object(value) = self {
            Ok(value)
        } else {
            Err("BasicSates is not an Object")
        }
    }
}

impl Clone for BasicStates {
    fn clone(&self) -> Self {
        match self {
            BasicStates::U8(value) => BasicStates::U8(*value),
            BasicStates::U16(value) => BasicStates::U16(*value),
            BasicStates::U32(value) => BasicStates::U32(*value),
            BasicStates::U64(value) => BasicStates::U64(*value),
            BasicStates::U128(value) => BasicStates::U128(*value),
            BasicStates::I8(value) => BasicStates::I8(*value),
            BasicStates::I16(value) => BasicStates::I16(*value),
            BasicStates::I32(value) => BasicStates::I32(*value),
            BasicStates::I64(value) => BasicStates::I64(*value),
            BasicStates::I128(value) => BasicStates::I128(*value),
            BasicStates::F32(value) => BasicStates::F32(*value),
            BasicStates::F64(value) => BasicStates::F64(*value),
            BasicStates::Bool(value) => BasicStates::Bool(*value),
            BasicStates::String(value) => BasicStates::String(value.clone()),
            BasicStates::Array(value) => BasicStates::Array(value.clone()),
            BasicStates::Object(value) => BasicStates::Object(value.clone()),
            BasicStates::Any(_) => panic!("Cannot clone BasicSates::Any variant"),
        }
    }
}

impl AsRef<u8> for BasicStates {
    fn as_ref(&self) -> &u8 {
        match self {
            BasicStates::U8(value) => value,
            _ => panic!("BasicStates is not a U8 variant"),
        }
    }
}

impl AsRef<u16> for BasicStates {
    fn as_ref(&self) -> &u16 {
        match self {
            BasicStates::U16(value) => value,
            _ => panic!("BasicStates is not a U16 variant"),
        }
    }
}

impl AsRef<u32> for BasicStates {
    fn as_ref(&self) -> &u32 {
        match self {
            BasicStates::U32(value) => value,
            _ => panic!("BasicStates is not a U32 variant"),
        }
    }
}

impl AsRef<u64> for BasicStates {
    fn as_ref(&self) -> &u64 {
        match self {
            BasicStates::U64(value) => value,
            _ => panic!("BasicStates is not a U64 variant"),
        }
    }
}

impl AsRef<u128> for BasicStates {
    fn as_ref(&self) -> &u128 {
        match self {
            BasicStates::U128(value) => value,
            _ => panic!("BasicStates is not a U128 variant"),
        }
    }
}

impl AsRef<i8> for BasicStates {
    fn as_ref(&self) -> &i8 {
        match self {
            BasicStates::I8(value) => value,
            _ => panic!("BasicStates is not an I8 variant"),
        }
    }
}

impl AsRef<i16> for BasicStates {
    fn as_ref(&self) -> &i16 {
        match self {
            BasicStates::I16(value) => value,
            _ => panic!("BasicStates is not an I16 variant"),
        }
    }
}

impl AsRef<i32> for BasicStates {
    fn as_ref(&self) -> &i32 {
        match self {
            BasicStates::I32(value) => value,
            _ => panic!("BasicStates is not an I32 variant"),
        }
    }
}

impl AsRef<i64> for BasicStates {
    fn as_ref(&self) -> &i64 {
        match self {
            BasicStates::I64(value) => value,
            _ => panic!("BasicStates is not an I64 variant"),
        }
    }
}

impl AsRef<i128> for BasicStates {
    fn as_ref(&self) -> &i128 {
        match self {
            BasicStates::I128(value) => value,
            _ => panic!("BasicStates is not an I128 variant"),
        }
    }
}

impl AsRef<f32> for BasicStates {
    fn as_ref(&self) -> &f32 {
        match self {
            BasicStates::F32(value) => value,
            _ => panic!("BasicStates is not an F32 variant"),
        }
    }
}

impl AsRef<f64> for BasicStates {
    fn as_ref(&self) -> &f64 {
        match self {
            BasicStates::F64(value) => value,
            _ => panic!("BasicStates is not an F64 variant"),
        }
    }
}

impl AsRef<bool> for BasicStates {
    fn as_ref(&self) -> &bool {
        match self {
            BasicStates::Bool(value) => value,
            _ => panic!("BasicStates is not a Bool variant"),
        }
    }
}

impl AsRef<String> for BasicStates {
    fn as_ref(&self) -> &String {
        match self {
            BasicStates::String(value) => value,
            _ => panic!("BasicStates is not a String variant"),
        }
    }
}

impl AsRef<str> for BasicStates {
    fn as_ref(&self) -> &str {
        match self {
            BasicStates::String(value) => value.as_str(),
            _ => panic!("BasicStates is not a String variant"),
        }
    }
}

impl AsRef<Vec<BasicStates>> for BasicStates {
    fn as_ref(&self) -> &Vec<BasicStates> {
        match self {
            BasicStates::Array(value) => value,
            _ => panic!("BasicStates is not an Array variant"),
        }
    }
}

impl AsRef<[BasicStates]> for BasicStates {
    fn as_ref(&self) -> &[BasicStates] {
        match self {
            BasicStates::Array(value) => value.as_slice(),
            _ => panic!("BasicStates is not an Array variant"),
        }
    }
}

impl AsRef<BTreeMap<String, BasicStates>> for BasicStates {
    fn as_ref(&self) -> &BTreeMap<String, BasicStates> {
        match self {
            BasicStates::Object(value) => value,
            _ => panic!("BasicStates is not an Object variant"),
        }
    }
}

impl AsMut<u8> for BasicStates {
    fn as_mut(&mut self) -> &mut u8 {
        match self {
            BasicStates::U8(value) => value,
            _ => panic!("BasicStates is not a U8 variant"),
        }
    }
}

impl AsMut<u16> for BasicStates {
    fn as_mut(&mut self) -> &mut u16 {
        match self {
            BasicStates::U16(value) => value,
            _ => panic!("BasicStates is not a U16 variant"),
        }
    }
}

impl AsMut<u32> for BasicStates {
    fn as_mut(&mut self) -> &mut u32 {
        match self {
            BasicStates::U32(value) => value,
            _ => panic!("BasicStates is not a U32 variant"),
        }
    }
}

impl AsMut<u64> for BasicStates {
    fn as_mut(&mut self) -> &mut u64 {
        match self {
            BasicStates::U64(value) => value,
            _ => panic!("BasicStates is not a U64 variant"),
        }
    }
}

impl AsMut<u128> for BasicStates {
    fn as_mut(&mut self) -> &mut u128 {
        match self {
            BasicStates::U128(value) => value,
            _ => panic!("BasicStates is not a U128 variant"),
        }
    }
}

impl AsMut<i8> for BasicStates {
    fn as_mut(&mut self) -> &mut i8 {
        match self {
            BasicStates::I8(value) => value,
            _ => panic!("BasicStates is not an I8 variant"),
        }
    }
}

impl AsMut<i16> for BasicStates {
    fn as_mut(&mut self) -> &mut i16 {
        match self {
            BasicStates::I16(value) => value,
            _ => panic!("BasicStates is not an I16 variant"),
        }
    }
}

impl AsMut<i32> for BasicStates {
    fn as_mut(&mut self) -> &mut i32 {
        match self {
            BasicStates::I32(value) => value,
            _ => panic!("BasicStates is not an I32 variant"),
        }
    }
}

impl AsMut<i64> for BasicStates {
    fn as_mut(&mut self) -> &mut i64 {
        match self {
            BasicStates::I64(value) => value,
            _ => panic!("BasicStates is not an I64 variant"),
        }
    }
}

impl AsMut<i128> for BasicStates {
    fn as_mut(&mut self) -> &mut i128 {
        match self {
            BasicStates::I128(value) => value,
            _ => panic!("BasicStates is not an I128 variant"),
        }
    }
}

impl AsMut<f32> for BasicStates {
    fn as_mut(&mut self) -> &mut f32 {
        match self {
            BasicStates::F32(value) => value,
            _ => panic!("BasicStates is not an F32 variant"),
        }
    }
}

impl AsMut<f64> for BasicStates {
    fn as_mut(&mut self) -> &mut f64 {
        match self {
            BasicStates::F64(value) => value,
            _ => panic!("BasicStates is not an F64 variant"),
        }
    }
}

impl AsMut<bool> for BasicStates {
    fn as_mut(&mut self) -> &mut bool {
        match self {
            BasicStates::Bool(value) => value,
            _ => panic!("BasicStates is not a Bool variant"),
        }
    }
}

impl AsMut<String> for BasicStates {
    fn as_mut(&mut self) -> &mut String {
        match self {
            BasicStates::String(value) => value,
            _ => panic!("BasicStates is not a String variant"),
        }
    }
}

impl AsMut<Vec<BasicStates>> for BasicStates {
    fn as_mut(&mut self) -> &mut Vec<BasicStates> {
        match self {
            BasicStates::Array(value) => value,
            _ => panic!("BasicStates is not an Array variant"),
        }
    }
}

impl AsMut<[BasicStates]> for BasicStates {
    fn as_mut(&mut self) -> &mut [BasicStates] {
        match self {
            BasicStates::Array(value) => value.as_mut_slice(),
            _ => panic!("BasicStates is not an Array variant"),
        }
    }
}

impl AsMut<BTreeMap<String, BasicStates>> for BasicStates {
    fn as_mut(&mut self) -> &mut BTreeMap<String, BasicStates> {
        match self {
            BasicStates::Object(value) => value,
            _ => panic!("BasicStates is not an Object variant"),
        }
    }
}

impl AsRef<dyn Any> for BasicStates {
    fn as_ref(&self) -> &dyn Any {
        match self {
            BasicStates::Any(value) => value.as_ref(),
            _ => panic!("BasicStates is not of Any variant"),
        }
    }
}

impl AsMut<dyn Any> for BasicStates {
    fn as_mut(&mut self) -> &mut dyn Any {
        match self {
            BasicStates::Any(value) => value.as_mut(),
            _ => panic!("BasicStates is not of Any variant"),
        }
    }
}

impl BasicStates {
    pub fn is_u8(&self) -> bool {
        matches!(self, BasicStates::U8(_))
    }

    pub fn is_u16(&self) -> bool {
        matches!(self, BasicStates::U16(_))
    }

    pub fn is_u32(&self) -> bool {
        matches!(self, BasicStates::U32(_))
    }

    pub fn is_u64(&self) -> bool {
        matches!(self, BasicStates::U64(_))
    }

    pub fn is_u128(&self) -> bool {
        matches!(self, BasicStates::U128(_))
    }

    pub fn is_i8(&self) -> bool {
        matches!(self, BasicStates::I8(_))
    }

    pub fn is_i16(&self) -> bool {
        matches!(self, BasicStates::I16(_))
    }

    pub fn is_i32(&self) -> bool {
        matches!(self, BasicStates::I32(_))
    }

    pub fn is_i64(&self) -> bool {
        matches!(self, BasicStates::I64(_))
    }

    pub fn is_i128(&self) -> bool {
        matches!(self, BasicStates::I128(_))
    }

    pub fn is_f32(&self) -> bool {
        matches!(self, BasicStates::F32(_))
    }

    pub fn is_f64(&self) -> bool {
        matches!(self, BasicStates::F64(_))
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, BasicStates::Bool(_))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, BasicStates::String(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, BasicStates::Array(_))
    }

    pub fn is_object(&self) -> bool {
        matches!(self, BasicStates::Object(_))
    }

    pub fn is_any(&self) -> bool {
        matches!(self, BasicStates::Any(_))
    }
}
