use std::marker::PhantomData;

use super::*;
pub use serde_json::Number;

/// A reference to a JSON value.
#[derive(Clone, PartialEq, Eq)]
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
    /// Creates a `ValueRef` from a byte slice.
    ///
    /// # Safety
    ///
    /// The bytes must be a valid JSON value created by `Builder`.
    pub unsafe fn from_bytes(bytes: &[u8]) -> ValueRef<'_> {
        let base = bytes.as_ptr().add(bytes.len() - 4);
        let entry = (base as *const Entry).read();
        ValueRef::from_raw(base, entry)
    }

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

    pub(crate) unsafe fn from_raw(base: *const u8, entry: Entry) -> Self {
        if entry.is_null() {
            Self::Null
        } else if entry.is_false() {
            Self::Bool(false)
        } else if entry.is_true() {
            Self::Bool(true)
        } else if entry.is_number() {
            let ptr = entry.apply_offset(base);
            let kind = ptr.read();
            let payload = ptr.add(1);
            match kind {
                NUMBER_U64 => Self::Number(Number::from((payload as *const u64).read())),
                NUMBER_I64 => Self::Number(Number::from((payload as *const i64).read())),
                NUMBER_F64 => {
                    Self::Number(Number::from_f64((payload as *const f64).read()).unwrap())
                }
                _ => panic!("invalid number kind"),
            }
        } else if entry.is_string() {
            let ptr = entry.apply_offset(base);
            let len = (ptr as *const u32).read() as usize;
            let payload = unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr.add(4), len))
            };
            Self::String(payload)
        } else if entry.is_array() {
            let ptr = entry.apply_offset(base);
            Self::Array(ArrayRef::from_raw(ptr))
        } else if entry.is_object() {
            let ptr = entry.apply_offset(base);
            Self::Object(ObjectRef::from_raw(ptr))
        } else {
            panic!("invalid entry");
        }
    }

    /// Returns the capacity to store this value, in bytes.
    pub(crate) fn capacity(&self) -> usize {
        match self {
            Self::Null => 0,
            Self::Bool(_) => 0,
            Self::Number(_) => 1 + 8,
            Self::String(s) => s.len() + 4,
            Self::Array(a) => a.as_slice().len(),
            Self::Object(o) => o.as_slice().len(),
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
    // # layout
    //      v----------------------\
    // | elements | len | size | [eptr] x len |
    // |   size   |  4  |  4   |   4 x len    |
    // |<------------ as_slice -------------->|
    //            ^ptr
    ptr: *const u8,
    _mark: PhantomData<&'a u8>,
}

impl<'a> ArrayRef<'a> {
    /// Returns the element at the given index, or `None` if the index is out of bounds.
    pub fn get(&self, index: usize) -> Option<ValueRef<'a>> {
        if index >= self.len() {
            return None;
        }
        let entry = unsafe { ((self.ptr as *const u32).add(2 + index) as *const Entry).read() };
        Some(unsafe { ValueRef::from_raw(self.ptr, entry) })
    }

    /// Returns the number of elements in the array.
    pub fn len(&self) -> usize {
        unsafe { (self.ptr as *const u32).read() as usize }
    }

    /// Returns `true` if the array contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns an iterator over the array's elements.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = ValueRef<'a>> {
        let base = self.ptr;
        unsafe {
            let entries = std::slice::from_raw_parts(
                (self.ptr as *const u32).add(2) as *const Entry,
                self.len(),
            );
            entries.iter().map(move |e| ValueRef::from_raw(base, *e))
        }
    }

    /// Returns the entire array as a slice.
    pub(crate) fn as_slice(&self) -> &[u8] {
        let len = self.len();
        let elem_len = self.elements_len();
        unsafe { std::slice::from_raw_parts(self.ptr.sub(elem_len), elem_len + 4 + 4 + 4 * len) }
    }

    /// Returns the length of the array's elements, in bytes.
    pub(crate) fn elements_len(&self) -> usize {
        unsafe { (self.ptr as *const u32).add(1).read() as usize }
    }

    /// Creates an `ArrayRef` from a raw pointer.
    unsafe fn from_raw(ptr: *const u8) -> Self {
        Self {
            ptr,
            _mark: PhantomData,
        }
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

impl PartialEq for ArrayRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.iter().eq(other.iter())
    }
}

impl Eq for ArrayRef<'_> {}

/// A reference to a JSON object.
#[derive(Clone, Copy)]
pub struct ObjectRef<'a> {
    // # layout
    //      v-v--------------------\-----\
    // | elements | len | size | [kptr, vptr] x len |
    // |   size   |  4  |  4   |     4 x 2 x len    |
    // |<--------------- as_slice ----------------->|
    //            ^ptr
    ptr: *const u8,
    _mark: PhantomData<&'a u8>,
}

impl<'a> ObjectRef<'a> {
    /// Returns the value associated with the given key, or `None` if the key is not present.
    pub fn get(&self, key: &str) -> Option<ValueRef<'a>> {
        // do binary search since entries are ordered by key
        let idx = self
            .entries()
            .binary_search_by_key(&key, |&(kentry, _)| unsafe {
                ValueRef::from_raw(self.ptr, kentry)
                    .as_str()
                    .expect("key must be string")
            })
            .ok()?;
        let (_, ventry) = self.entries()[idx];
        Some(unsafe { ValueRef::from_raw(self.ptr, ventry) })
    }

    /// Returns the number of elements in the object.
    pub fn len(&self) -> usize {
        unsafe { (self.ptr as *const u32).read() as usize }
    }

    /// Returns `true` if the object contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns an iterator over the object's key-value pairs.
    pub fn iter(&'a self) -> impl ExactSizeIterator<Item = (&'a str, ValueRef<'a>)> {
        let base = self.ptr;
        unsafe {
            self.entries().iter().map(move |&(kentry, ventry)| {
                let k = ValueRef::from_raw(base, kentry);
                let v = ValueRef::from_raw(base, ventry);
                (k.as_str().expect("key must be string"), v)
            })
        }
    }

    /// Returns an iterator over the object's keys.
    pub fn keys(&'a self) -> impl ExactSizeIterator<Item = &'a str> {
        self.iter().map(|(k, _)| k)
    }

    /// Returns an iterator over the object's values.
    pub fn values(&'a self) -> impl ExactSizeIterator<Item = ValueRef<'a>> {
        self.iter().map(|(_, v)| v)
    }

    /// Returns the entire object as a slice.
    pub(crate) fn as_slice(&self) -> &[u8] {
        let len = self.len();
        let elem_len = self.elements_len();
        unsafe { std::slice::from_raw_parts(self.ptr.sub(elem_len), elem_len + 4 + 4 + 8 * len) }
    }

    /// Returns the length of the object's elements, in bytes.
    pub(crate) fn elements_len(&self) -> usize {
        unsafe { (self.ptr as *const u32).add(1).read() as usize }
    }

    /// Creates an `ArrayRef` from a raw pointer.
    unsafe fn from_raw(ptr: *const u8) -> Self {
        Self {
            ptr,
            _mark: PhantomData,
        }
    }

    /// Returns the key-value entries.
    fn entries(&self) -> &[(Entry, Entry)] {
        unsafe {
            std::slice::from_raw_parts(
                (self.ptr as *const u32).add(2) as *const (Entry, Entry),
                self.len(),
            )
        }
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

impl PartialEq for ObjectRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.iter().eq(other.iter())
    }
}

impl Eq for ObjectRef<'_> {}

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
