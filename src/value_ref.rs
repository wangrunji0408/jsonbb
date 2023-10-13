use super::*;
pub use serde_json::Number;

/// A reference to a JSON value.
#[derive(Clone)]
pub enum ValueRef<'a> {
    /// Represents a JSON null value.
    Null,
    /// Represents a JSON boolean.
    Bool(bool),
    /// Represents a JSON number.
    Number(Number),
    /// Represents a JSON string.
    String(&'a str),
    /// Represents a JSON array.
    Array(ArrayRef<'a>),
    /// Represents a JSON object.
    Object(ObjectRef<'a>),
}

impl<'a> ValueRef<'a> {
    /// If the value is `null`, returns `()`. Returns `None` otherwise.
    pub fn as_null(self) -> Option<()> {
        match self {
            Self::Null => Some(()),
            _ => None,
        }
    }

    /// If the value is a boolean, returns the associated bool. Returns `None` otherwise.
    pub fn as_bool(self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(b),
            _ => None,
        }
    }

    /// If the value is an integer, returns the associated u64. Returns `None` otherwise.
    pub fn as_u64(self) -> Option<u64> {
        match self {
            Self::Number(n) => n.as_u64(),
            _ => None,
        }
    }

    /// If the value is an integer, returns the associated i64. Returns `None` otherwise.
    pub fn as_i64(self) -> Option<i64> {
        match self {
            Self::Number(n) => n.as_i64(),
            _ => None,
        }
    }

    /// If the value is a float, returns the associated f64. Returns `None` otherwise.
    pub fn as_f64(self) -> Option<f64> {
        match self {
            Self::Number(n) => n.as_f64(),
            _ => None,
        }
    }

    /// If the value is a string, returns the associated str. Returns `None` otherwise.
    pub fn as_str(self) -> Option<&'a str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// If the value is an array, returns the associated array. Returns `None` otherwise.
    pub fn as_array(self) -> Option<ArrayRef<'a>> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// If the value is an object, returns the associated map. Returns `None` otherwise.
    pub fn as_object(self) -> Option<ObjectRef<'a>> {
        match self {
            Self::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Creates owned `Value` from `ValueRef`.
    pub fn to_owned(self) -> Value {
        self.into()
    }

    pub(crate) fn from(buffer: &'a [u8], id: Id) -> Self {
        match id {
            Id::NULL => Self::Null,
            Id::TRUE => Self::Bool(true),
            Id::FALSE => Self::Bool(false),
            _ => {
                let mut buf = &buffer[id.0 as usize..];
                match buf.get_u8() {
                    TAG_U64 => Self::Number(buf.get_u64_le().into()),
                    TAG_I64 => Self::Number(buf.get_i64_le().into()),
                    TAG_F64 => {
                        Self::Number(Number::from_f64(buf.get_f64_le()).expect("infinite number"))
                    }
                    TAG_STRING => Self::String({
                        let len = buf.get_u32_le() as usize;
                        unsafe { std::str::from_utf8_unchecked(&buf[..len]) }
                    }),
                    TAG_ARRAY => Self::Array(ArrayRef {
                        buffer,
                        id,
                        len: buf.get_u32_le(),
                    }),
                    TAG_OBJECT => Self::Object(ObjectRef {
                        buffer,
                        id,
                        len: buf.get_u32_le(),
                    }),
                    t => panic!("invalid tag: {t}"),
                }
            }
        }
    }

    /// Returns the capacity to store this value, in bytes.
    pub(crate) fn capacity(&self) -> usize {
        match self {
            Self::Null => 0,
            Self::Bool(_) => 0,
            Self::Number(_) => 1 + 8,
            Self::String(s) => s.len() + 4 + 1,
            Self::Array(a) => a.buffer.len(),
            Self::Object(o) => o.buffer.len(),
        }
    }

    /// Index into a JSON array or object.
    /// A string index can be used to access a value in an object,
    /// and a usize index can be used to access an element of an array.
    pub fn get(&self, index: impl Index) -> Option<ValueRef<'a>> {
        index.index_into(self)
    }
}

impl fmt::Debug for ValueRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => f.write_str("null"),
            Self::Bool(b) => b.fmt(f),
            Self::Number(n) => n.fmt(f),
            Self::String(s) => s.fmt(f),
            Self::Array(a) => a.fmt(f),
            Self::Object(o) => o.fmt(f),
        }
    }
}

/// Display a JSON value as a string.
impl fmt::Display for ValueRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serialize_in_json(self, f)
    }
}

/// A reference to a JSON array.
#[derive(Clone, Copy)]
pub struct ArrayRef<'a> {
    pub(crate) buffer: &'a [u8],
    // assume tag == TAG_ARRAY
    pub(crate) id: Id,
    pub(crate) len: u32,
}

impl<'a> ArrayRef<'a> {
    /// Returns the element at the given index, or `None` if the index is out of bounds.
    pub fn get(&self, index: usize) -> Option<ValueRef<'a>> {
        if index >= self.len() {
            return None;
        }
        let ptr = self.id.0 as usize + 1 + Id::SIZE * (index + 1);
        Some(ValueRef::from(
            self.buffer,
            Id((&self.buffer[ptr..]).get_u32_le()),
        ))
    }

    /// Returns the number of elements in the array.
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns `true` if the array contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns an iterator over the array's elements.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = ValueRef<'a>> + 'a {
        let buffer = self.buffer;
        let mut buf = &self.buffer[self.id.0 as usize + 1 + Id::SIZE..];
        (0..self.len()).map(move |_| ValueRef::from(buffer, Id(buf.get_u32_le())))
    }
}

impl fmt::Debug for ArrayRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// Display a JSON array as a string.
impl fmt::Display for ArrayRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serialize_in_json(self, f)
    }
}

/// A reference to a JSON object.
#[derive(Clone, Copy)]
pub struct ObjectRef<'a> {
    buffer: &'a [u8],
    // assume tag == TAG_OBJECT
    id: Id,
    len: u32,
}

impl<'a> ObjectRef<'a> {
    /// Returns the value associated with the given key, or `None` if the key is not present.
    pub fn get(&self, key: &str) -> Option<ValueRef<'a>> {
        // TODO: binary search
        // linear search
        self.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    /// Returns the number of elements in the object.
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns `true` if the object contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns an iterator over the object's key-value pairs.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&'a str, ValueRef<'a>)> + 'a {
        let buffer = self.buffer;
        let mut buf = &self.buffer[self.id.0 as usize + 1 + Id::SIZE..];
        (0..self.len()).map(move |_| {
            let kid = Id(buf.get_u32_le());
            let vid = Id(buf.get_u32_le());
            let k = ValueRef::from(buffer, kid).as_str().unwrap();
            let v = ValueRef::from(buffer, vid);
            (k, v)
        })
    }

    /// Returns an iterator over the object's keys.
    pub fn keys(&self) -> impl ExactSizeIterator<Item = &'a str> + 'a {
        self.iter().map(|(k, _)| k)
    }

    /// Returns an iterator over the object's values.
    pub fn values(&self) -> impl ExactSizeIterator<Item = ValueRef<'a>> + 'a {
        self.iter().map(|(_, v)| v)
    }
}

impl fmt::Debug for ObjectRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

/// Display a JSON object as a string.
impl fmt::Display for ObjectRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serialize_in_json(self, f)
    }
}

/// Dump the internal buffer structure.
/// This is useful for debugging.
pub(crate) fn dump(mut buf: &[u8]) -> String {
    use std::fmt::Write;
    let mut string = String::new();
    let s = &mut string;

    let start_ptr = buf.as_ptr() as usize;
    while !buf.is_empty() {
        let id = Id((buf.as_ptr() as usize - start_ptr) as u32);
        match buf.get_u8() {
            TAG_U64 => writeln!(s, "{id:?}: {}", buf.get_u64_le()).unwrap(),
            TAG_I64 => writeln!(s, "{id:?}: {}", buf.get_i64_le()).unwrap(),
            TAG_F64 => writeln!(s, "{id:?}: {}", buf.get_f64_le()).unwrap(),
            TAG_STRING => {
                let len = buf.get_u32_le() as usize;
                let str = unsafe { std::str::from_utf8_unchecked(&buf[..len]) };
                buf = &buf[len..];
                writeln!(s, "{id:?}: {str:?}").unwrap();
            }
            TAG_ARRAY => {
                let len = buf.get_u32_le() as usize;
                write!(s, "{id:?}: [").unwrap();
                for i in 0..len {
                    if i != 0 {
                        write!(s, ", ").unwrap();
                    }
                    write!(s, "{:?}", Id(buf.get_u32_le())).unwrap();
                }
                writeln!(s, "]").unwrap();
            }
            TAG_OBJECT => {
                let len = buf.get_u32_le() as usize;
                write!(s, "{id:?}: {{").unwrap();
                for i in 0..len {
                    if i != 0 {
                        write!(s, ", ").unwrap();
                    }
                    let kid = Id(buf.get_u32_le());
                    let vid = Id(buf.get_u32_le());
                    write!(s, "{kid:?}:{vid:?}").unwrap();
                }
                writeln!(s, "}}").unwrap();
            }
            t => panic!("invalid tag: {t}"),
        }
    }
    string
}

/// Serialize a value in JSON format.
fn serialize_in_json(value: &impl ::serde::Serialize, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    use std::io;

    struct WriterFormatter<'a, 'b: 'a> {
        inner: &'a mut fmt::Formatter<'b>,
    }

    impl<'a, 'b> io::Write for WriterFormatter<'a, 'b> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            // Safety: the serializer below only emits valid utf8 when using
            // the default formatter.
            let s = unsafe { std::str::from_utf8_unchecked(buf) };
            self.inner.write_str(s).map_err(io_error)?;
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn io_error(_: fmt::Error) -> io::Error {
        // Error value does not matter because Display impl just maps it
        // back to fmt::Error.
        io::Error::new(io::ErrorKind::Other, "fmt error")
    }

    let alternate = f.alternate();
    let mut wr = WriterFormatter { inner: f };
    if alternate {
        // {:#}
        value
            .serialize(&mut serde_json::Serializer::pretty(&mut wr))
            .map_err(|_| fmt::Error)
    } else {
        // {}
        value
            .serialize(&mut serde_json::Serializer::new(&mut wr))
            .map_err(|_| fmt::Error)
    }
}

/// A type that can be used to index into a `ValueRef`.
pub trait Index: private::Sealed {
    /// Return None if the key is not already in the array or object.
    #[doc(hidden)]
    fn index_into<'v>(&self, v: &ValueRef<'v>) -> Option<ValueRef<'v>>;
}

impl Index for usize {
    fn index_into<'v>(&self, v: &ValueRef<'v>) -> Option<ValueRef<'v>> {
        match v {
            ValueRef::Array(a) => a.get(*self),
            _ => None,
        }
    }
}

impl Index for str {
    fn index_into<'v>(&self, v: &ValueRef<'v>) -> Option<ValueRef<'v>> {
        match v {
            ValueRef::Object(o) => o.get(self),
            _ => None,
        }
    }
}

impl Index for String {
    fn index_into<'v>(&self, v: &ValueRef<'v>) -> Option<ValueRef<'v>> {
        match v {
            ValueRef::Object(o) => o.get(self),
            _ => None,
        }
    }
}

impl<'a, T> Index for &'a T
where
    T: ?Sized + Index,
{
    fn index_into<'v>(&self, v: &ValueRef<'v>) -> Option<ValueRef<'v>> {
        (**self).index_into(v)
    }
}

// Prevent users from implementing the Index trait.
mod private {
    pub trait Sealed {}
    impl Sealed for usize {}
    impl Sealed for str {}
    impl Sealed for String {}
    impl<'a, T> Sealed for &'a T where T: ?Sized + Sealed {}
}
