#[macro_export]
macro_rules! backcompat_repr_u8_enum {
    (
        $(#[$attrs:meta])*
        pub enum $name:ident {
            $($variant:ident),* $(,)?
        }
    ) => {
        $(#[$attrs])*
        #[repr(u8)]
        pub enum $name {
            $( $variant ),*
        }

        impl serde::Serialize for $name
        where
            Self: Copy {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer
            {
                serializer.serialize_u8(*self as u8)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name
        where
            Self: Copy {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct V;

                impl<'de> serde::de::Visitor<'de> for V {
                    type Value = $name;

                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        write!(f, "string or u8 enum variant")
                    }

                    fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        match v {
                            $( x if x == $name::$variant as u8 => Ok($name::$variant), )*
                            _ => Err(E::custom(format!("invalid {} value {}", stringify!($name), v))),
                        }
                    }

                    // JSON numbers → u64
                    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
                    where
                        E: ::serde::de::Error,
                    {
                        match v {
                            $( x if x == $name::$variant as u64 => Ok($name::$variant), )*
                            _ => Err(E::custom(format!("invalid {} value {}", stringify!($name), v))),
                        }
                    }

                    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        match s {
                            $( stringify!($variant) => Ok($name::$variant), )*
                            _ => Err(E::custom(format!("invalid {} variant {}", stringify!($name), s))),
                        }
                    }
                }

                deserializer.deserialize_any(V)
            }
        }
    };
}
