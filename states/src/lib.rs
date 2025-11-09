pub mod basic_states;
pub mod ctx;
pub mod state;

macro_rules! StateIDs {
    ($name:ident, {$($id:ident),*$(,)*}) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(usize)]
        pub enum $name {
            $($id,)+
        }

        const LEN:usize = [$(StateID::$id,)+].len();
        const STATE_ID_ARRAY: [StateID; LEN] = [$(StateID::$id,)+];
        const STATE_ID_STRING: [&'static str; LEN] = [$(stringify!($id),)+];
        const STATE_ID_LENGTHS: [usize; LEN] = [$(stringify!($id).len(),)+];

        impl $name {
            #[inline]
            pub const fn id_len(&self) -> usize {
                STATE_ID_LENGTHS[*self as usize]
            }
            #[inline]
            pub const fn is_empty(&self) -> bool {
                false
            }
            #[inline]
            pub const fn get_all() -> &'static [StateID] {
                &STATE_ID_ARRAY
            }
            #[inline]
            pub const fn amount() -> usize {
                LEN
            }
        }

        impl Default for $name {
            #[inline]
            fn default() -> Self {
                STATE_ID_ARRAY[0]
            }
        }

        impl TryFrom<usize> for $name {
            type Error = usize;
            fn try_from(value: usize) -> Result<Self, Self::Error> {
                if value < LEN {
                    Ok(STATE_ID_ARRAY[value])
                } else {
                    Err(value)
                }
            }
        }

        impl<'a> TryFrom<&'a str> for $name {
            type Error = &'a str;
            fn try_from(value: &'a str) -> Result<Self, Self::Error> {
                match value {
                    $(
                        stringify!($id) => Ok(Self::$id),
                    )+
                    _ => Err(value)
                }
            }
        }

        impl Into<&'static str> for $name {
            fn into(self) -> &'static str {
                STATE_ID_STRING[self as usize]
            }
        }

        impl Into<usize> for $name {
            fn into(self) -> usize {
                self as usize
            }
        }

        impl ToString for $name {
            #[inline]
            fn to_string(&self) -> String {
                STATE_ID_STRING[*self as usize].to_string()
            }
        }
    };
}

StateIDs!(StateID, { None });
