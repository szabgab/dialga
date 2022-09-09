use ahash::AHashMap;
use serde::de::{self, Error, IntoDeserializer, Unexpected};
use smol_str::SmolStr;

use crate::{literal::KdlAnnotatedLiteralDeser, DeError, KdlAnnotatedLiteral, KdlLiteral, KdlNode};

#[derive(Debug, Clone)]
pub(crate) struct KdlNodeDeser<'de> {
    name: &'de str,
    arguments: &'de [KdlAnnotatedLiteral],
    properties: &'de AHashMap<SmolStr, KdlAnnotatedLiteral>,
    children: Option<&'de [KdlNode]>,
}

impl<'de> KdlNodeDeser<'de> {
    pub(crate) fn new(wrapped: &'de KdlNode) -> Self {
        Self {
            name: wrapped.name.as_str(),
            arguments: &wrapped.arguments,
            properties: &wrapped.properties,
            children: wrapped.children.as_ref().map(|kids| kids.as_slice()),
        }
    }
}

macro_rules! type_error {
    (@ $ty:ident) => {
        paste::paste! {
            fn [< deserialize_ $ty >]<V>(self, visitor: V) -> Result<V::Value, Self::Error>
            where
                V: de::Visitor<'de> {
                Err(DeError::invalid_type(Unexpected::Other("a node"), &visitor))
            }
        }
    };
    ( $($ty:ident)* ) => {
        $(
            type_error!(@ $ty);
        )*
    };
}

impl<'de> de::Deserializer<'de> for KdlNodeDeser<'de> {
    type Error = DeError;

    type_error! {
        u8 u16 u32 u64 i8 i16 i32 i64 char bool f32 f64
        str string bytes byte_buf option identifier
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let kids_all_dashes = if let Some(kids) = self.children {
            kids.iter().all(|kid| kid.name == "-")
        } else {
            false
        };

        match (
            !self.arguments.is_empty(),
            !self.properties.is_empty() || self.children.is_some(),
        ) {
            (false, false) => visitor.visit_unit(),
            (true, true) => Err(DeError::invalid_type(
                Unexpected::Other(
                    "node with arguments, properties/children, or neither (and not both)",
                ),
                &visitor,
            )),
            (true, false) => visitor.visit_seq(SeqArgsDeser(self.arguments)),
            _ if kids_all_dashes => visitor.visit_seq(SeqDashChildrenDeser(self.children.unwrap())),
            (false, true) => self.deserialize_map(visitor),
        }
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if !self.arguments.is_empty() {
            return Err(DeError::invalid_type(
                Unexpected::Other("node with no arguments"),
                &visitor,
            ));
        }

        let mut properties: Vec<_> = self
            .properties
            .iter()
            .map(|(key, val)| (key.as_str(), val))
            .collect();
        properties.reverse();
        visitor.visit_map(MapDeser {
            properties,
            children: self.children,
            value: MapDeserVal::None,
        })
    }
    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if !self.properties.is_empty() || (self.arguments.is_empty() && self.children.is_none()) {
            return Err(DeError::invalid_type(
                Unexpected::Other(
                    "node invalid as sequence (needs either only args, or children all named `-`)",
                ),
                &visitor,
            ));
        }

        if let Some(kids) = self.children {
            let kids_all_dashes = kids.iter().all(|kid| kid.name == "-");
            if !kids_all_dashes {
                return Err(DeError::invalid_type(Unexpected::Other("node invalid as sequence (needs either only args, or children all named `-`)"), &visitor));
            }
            visitor.visit_seq(SeqDashChildrenDeser(kids))
        } else {
            visitor.visit_seq(SeqArgsDeser(self.arguments))
        }
    }
    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }
    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if self.arguments.is_empty() && self.properties.is_empty() && self.children.is_none() {
            visitor.visit_unit()
        } else {
            Err(DeError::invalid_type(
                Unexpected::Other("node with arguments or properties or children"),
                &visitor,
            ))
        }
    }
    fn deserialize_unit_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }
    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        Err(DeError::invalid_type(Unexpected::Other("a node"), &visitor))
    }
}

struct MapDeser<'de> {
    /// These are in *backwards* order so it's cheap to pop the back one off
    properties: Vec<(&'de str, &'de KdlAnnotatedLiteral)>,
    children: Option<&'de [KdlNode]>,

    value: MapDeserVal<'de>,
}

enum MapDeserVal<'de> {
    None,
    Property(&'de KdlAnnotatedLiteral),
    Child(&'de KdlNode),
}

impl<'de> de::MapAccess<'de> for MapDeser<'de> {
    type Error = DeError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        if !matches!(self.value, MapDeserVal::None) {
            return Err(DeError::custom("map visitor requested two keys in a row"));
        }

        // more like *pop*erties amirite
        if let Some((key, val)) = self.properties.pop() {
            self.value = MapDeserVal::Property(val);
            seed.deserialize(key.into_deserializer()).map(Some)
        } else if let Some([kid, tail @ ..]) = self.children {
            // lispily pop the front
            self.children = Some(tail);
            self.value = MapDeserVal::Child(kid);
            seed.deserialize(kid.name.as_str().into_deserializer())
                .map(Some)
        } else {
            Ok(None)
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        match std::mem::replace(&mut self.value, MapDeserVal::None) {
            MapDeserVal::None => Err(DeError::custom(
                "map visitor requested a value without a key",
            )),
            MapDeserVal::Property(prop) => seed.deserialize(KdlAnnotatedLiteralDeser::new(prop)),
            MapDeserVal::Child(kid) => seed.deserialize(KdlNodeDeser::new(kid)),
        }
    }
}

/// Sequence deserializer for a struct with only arguments
struct SeqArgsDeser<'de>(&'de [KdlAnnotatedLiteral]);

impl<'de> de::SeqAccess<'de> for SeqArgsDeser<'de> {
    type Error = DeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        if let [head, tail @ ..] = self.0 {
            self.0 = tail;
            seed.deserialize(KdlAnnotatedLiteralDeser::new(head))
                .map(Some)
        } else {
            Ok(None)
        }
    }
}

/// Sequence deserializer for a struct with only children and all of the nodes are named `-`
struct SeqDashChildrenDeser<'de>(&'de [KdlNode]);

impl<'de> de::SeqAccess<'de> for SeqDashChildrenDeser<'de> {
    type Error = DeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        if let [head, tail @ ..] = self.0 {
            self.0 = tail;
            seed.deserialize(KdlNodeDeser::new(head)).map(Some)
        } else {
            Ok(None)
        }
    }
}
