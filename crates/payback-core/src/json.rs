//! A small, strict JSON implementation: enough to talk to the UI without
//! pulling in serde. Objects keep insertion order.

use crate::error::{Error, Result};
use std::fmt::Write as _;

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

const MAX_DEPTH: usize = 128;

impl Json {
    pub fn parse(src: &str) -> Result<Json> {
        let mut p = Parser { s: src.as_bytes(), i: 0, depth: 0 };
        p.ws();
        let v = p.value()?;
        p.ws();
        if p.i != p.s.len() {
            return Err(p.err("unexpected trailing characters"));
        }
        Ok(v)
    }

    pub fn obj() -> Json {
        Json::Obj(Vec::new())
    }

    /// Builder-style insert for objects. Replaces an existing key.
    pub fn with(mut self, key: &str, value: impl ToJson) -> Json {
        self.set(key, value.to_json());
        self
    }

    pub fn set(&mut self, key: &str, value: Json) {
        if let Json::Obj(fields) = self {
            if let Some(slot) = fields.iter_mut().find(|(k, _)| k == key) {
                slot.1 = value;
            } else {
                fields.push((key.to_string(), value));
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Int(n) => Some(*n),
            Json::Float(f) if f.fract() == 0.0 && f.abs() < 9.007e15 => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Int(n) => Some(*n as f64),
            Json::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub fn stringify(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Int(n) => {
                let _ = write!(out, "{n}");
            }
            Json::Float(f) => {
                if f.is_finite() {
                    let _ = write!(out, "{f}");
                } else {
                    out.push_str("null");
                }
            }
            Json::Str(s) => write_str(s, out),
            Json::Arr(items) => {
                out.push('[');
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Json::Obj(fields) => {
                out.push('{');
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_str(k, out);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

fn write_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // U+2028/2029 are valid JSON but break some JS embedding contexts.
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn err(&self, msg: &str) -> Error {
        Error::Json(format!("Invalid JSON at byte {}: {msg}", self.i))
    }

    fn ws(&mut self) {
        while let Some(&c) = self.s.get(self.i) {
            if c == b' ' || c == b'\n' || c == b'\r' || c == b'\t' {
                self.i += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn expect_lit(&mut self, lit: &str, v: Json) -> Result<Json> {
        if self.s[self.i..].starts_with(lit.as_bytes()) {
            self.i += lit.len();
            Ok(v)
        } else {
            Err(self.err("unknown literal"))
        }
    }

    fn value(&mut self) -> Result<Json> {
        match self.peek() {
            None => Err(self.err("unexpected end of input")),
            Some(b'n') => self.expect_lit("null", Json::Null),
            Some(b't') => self.expect_lit("true", Json::Bool(true)),
            Some(b'f') => self.expect_lit("false", Json::Bool(false)),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b'[') => self.array(),
            Some(b'{') => self.object(),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            Some(_) => Err(self.err("unexpected character")),
        }
    }

    fn enter(&mut self) -> Result<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        Ok(())
    }

    fn array(&mut self) -> Result<Json> {
        self.enter()?;
        self.i += 1; // [
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            self.depth -= 1;
            return Ok(Json::Arr(items));
        }
        loop {
            self.ws();
            items.push(self.value()?);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(self.err("expected ',' or ']'")),
            }
        }
        self.depth -= 1;
        Ok(Json::Arr(items))
    }

    fn object(&mut self) -> Result<Json> {
        self.enter()?;
        self.i += 1; // {
        let mut fields: Vec<(String, Json)> = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            self.depth -= 1;
            return Ok(Json::Obj(fields));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("expected object key"));
            }
            let k = self.string()?;
            self.ws();
            if self.peek() != Some(b':') {
                return Err(self.err("expected ':'"));
            }
            self.i += 1;
            self.ws();
            let v = self.value()?;
            if let Some(slot) = fields.iter_mut().find(|(ek, _)| *ek == k) {
                slot.1 = v; // last one wins, like JSON.parse
            } else {
                fields.push((k, v));
            }
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
        self.depth -= 1;
        Ok(Json::Obj(fields))
    }

    fn hex4(&mut self) -> Result<u32> {
        let end = self.i + 4;
        if end > self.s.len() {
            return Err(self.err("truncated \\u escape"));
        }
        let txt = std::str::from_utf8(&self.s[self.i..end]).map_err(|_| self.err("bad \\u escape"))?;
        let v = u32::from_str_radix(txt, 16).map_err(|_| self.err("bad \\u escape"))?;
        self.i = end;
        Ok(v)
    }

    fn string(&mut self) -> Result<String> {
        self.i += 1; // opening quote
        let mut out = String::new();
        loop {
            let start = self.i;
            while let Some(&c) = self.s.get(self.i) {
                if c == b'"' || c == b'\\' || c < 0x20 {
                    break;
                }
                self.i += 1;
            }
            let chunk = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| self.err("invalid UTF-8"))?;
            out.push_str(chunk);
            match self.peek() {
                None => return Err(self.err("unterminated string")),
                Some(b'"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.i += 1;
                    let esc = self.peek().ok_or_else(|| self.err("unterminated escape"))?;
                    self.i += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xD800..0xDC00).contains(&hi) {
                                if self.s[self.i..].starts_with(b"\\u") {
                                    self.i += 2;
                                    let lo = self.hex4()?;
                                    if !(0xDC00..0xE000).contains(&lo) {
                                        return Err(self.err("invalid surrogate pair"));
                                    }
                                    0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                                } else {
                                    return Err(self.err("lone surrogate"));
                                }
                            } else if (0xDC00..0xE000).contains(&hi) {
                                return Err(self.err("lone surrogate"));
                            } else {
                                hi
                            };
                            out.push(char::from_u32(cp).ok_or_else(|| self.err("invalid code point"))?);
                        }
                        _ => return Err(self.err("invalid escape")),
                    }
                }
                Some(_) => return Err(self.err("control character in string")),
            }
        }
    }

    fn number(&mut self) -> Result<Json> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        let mut is_float = false;
        let digits = |p: &mut Parser| {
            let s = p.i;
            while matches!(p.peek(), Some(c) if c.is_ascii_digit()) {
                p.i += 1;
            }
            p.i - s
        };
        if self.peek() == Some(b'0') {
            self.i += 1;
            if matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                return Err(self.err("leading zeros are not allowed"));
            }
        } else if digits(self) == 0 {
            return Err(self.err("expected digit"));
        }
        if self.peek() == Some(b'.') {
            is_float = true;
            self.i += 1;
            if digits(self) == 0 {
                return Err(self.err("expected digit after '.'"));
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            is_float = true;
            self.i += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.i += 1;
            }
            if digits(self) == 0 {
                return Err(self.err("expected exponent digits"));
            }
        }
        let txt = std::str::from_utf8(&self.s[start..self.i]).unwrap();
        if !is_float {
            if let Ok(n) = txt.parse::<i64>() {
                return Ok(Json::Int(n));
            }
        }
        txt.parse::<f64>().map(Json::Float).map_err(|_| self.err("invalid number"))
    }
}

// ---------------------------------------------------------------------------
// Conversions

pub trait ToJson {
    fn to_json(&self) -> Json;
}

impl ToJson for Json {
    fn to_json(&self) -> Json {
        self.clone()
    }
}
impl ToJson for bool {
    fn to_json(&self) -> Json {
        Json::Bool(*self)
    }
}
macro_rules! int_to_json {
    ($($t:ty),*) => { $(impl ToJson for $t { fn to_json(&self) -> Json { Json::Int(*self as i64) } })* };
}
int_to_json!(i64, i32, u32, u16, u8, usize);
impl ToJson for f64 {
    fn to_json(&self) -> Json {
        Json::Float(*self)
    }
}
impl ToJson for String {
    fn to_json(&self) -> Json {
        Json::Str(self.clone())
    }
}
impl ToJson for str {
    fn to_json(&self) -> Json {
        Json::Str(self.to_string())
    }
}
impl<T: ToJson> ToJson for Option<T> {
    fn to_json(&self) -> Json {
        match self {
            Some(v) => v.to_json(),
            None => Json::Null,
        }
    }
}
impl<T: ToJson> ToJson for Vec<T> {
    fn to_json(&self) -> Json {
        Json::Arr(self.iter().map(|v| v.to_json()).collect())
    }
}
impl<T: ToJson> ToJson for [T] {
    fn to_json(&self) -> Json {
        Json::Arr(self.iter().map(|v| v.to_json()).collect())
    }
}
impl<T: ToJson, const N: usize> ToJson for [T; N] {
    fn to_json(&self) -> Json {
        Json::Arr(self.iter().map(|v| v.to_json()).collect())
    }
}
impl<T: ToJson + ?Sized> ToJson for &T {
    fn to_json(&self) -> Json {
        (**self).to_json()
    }
}

/// Implement `ToJson` for a struct by listing `field => "jsonKey"` pairs.
#[macro_export]
macro_rules! impl_to_json {
    ($t:ty { $($field:ident => $key:literal),* $(,)? }) => {
        impl $crate::json::ToJson for $t {
            fn to_json(&self) -> $crate::json::Json {
                $crate::json::Json::Obj(vec![
                    $(($key.to_string(), $crate::json::ToJson::to_json(&self.$field))),*
                ])
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Typed access to request arguments with readable error messages.

#[derive(Clone, Copy)]
pub struct Args<'a> {
    v: &'a Json,
}

static NULL: Json = Json::Null;

impl<'a> Args<'a> {
    pub fn new(v: &'a Json) -> Self {
        Args { v }
    }

    pub fn raw(&self) -> &'a Json {
        self.v
    }

    fn field(&self, key: &str) -> Option<&'a Json> {
        self.v.get(key)
    }

    /// True when the key is present, even if its value is null.
    pub fn has(&self, key: &str) -> bool {
        self.field(key).is_some()
    }

    pub fn obj(&self, key: &str) -> Option<Args<'a>> {
        match self.field(key) {
            Some(v @ Json::Obj(_)) => Some(Args { v }),
            _ => None,
        }
    }

    pub fn obj_or_empty(&self, key: &str) -> Args<'a> {
        self.obj(key).unwrap_or(Args { v: &NULL })
    }

    pub fn req_i64(&self, key: &str) -> Result<i64> {
        self.opt_i64(key)?.ok_or_else(|| Error::Json(format!("Missing number '{key}'")))
    }

    pub fn opt_i64(&self, key: &str) -> Result<Option<i64>> {
        match self.field(key) {
            None | Some(Json::Null) => Ok(None),
            Some(v) => v.as_i64().map(Some).ok_or_else(|| Error::Json(format!("'{key}' must be a whole number"))),
        }
    }

    /// For patches: `None` = leave unchanged, `Some(None)` = clear, `Some(Some(n))` = set.
    pub fn patch_i64(&self, key: &str) -> Result<Option<Option<i64>>> {
        match self.field(key) {
            None => Ok(None),
            Some(Json::Null) => Ok(Some(None)),
            Some(v) => v
                .as_i64()
                .map(|n| Some(Some(n)))
                .ok_or_else(|| Error::Json(format!("'{key}' must be a whole number or null"))),
        }
    }

    pub fn req_str(&self, key: &str) -> Result<&'a str> {
        self.opt_str(key)?.ok_or_else(|| Error::Json(format!("Missing text '{key}'")))
    }

    pub fn opt_str(&self, key: &str) -> Result<Option<&'a str>> {
        match self.field(key) {
            None | Some(Json::Null) => Ok(None),
            Some(Json::Str(s)) => Ok(Some(s)),
            Some(_) => Err(Error::Json(format!("'{key}' must be text"))),
        }
    }

    pub fn opt_bool(&self, key: &str) -> Result<Option<bool>> {
        match self.field(key) {
            None | Some(Json::Null) => Ok(None),
            Some(Json::Bool(b)) => Ok(Some(*b)),
            Some(_) => Err(Error::Json(format!("'{key}' must be true or false"))),
        }
    }

    pub fn bool_or(&self, key: &str, default: bool) -> Result<bool> {
        Ok(self.opt_bool(key)?.unwrap_or(default))
    }

    pub fn i64_list(&self, key: &str) -> Result<Vec<i64>> {
        match self.field(key) {
            None | Some(Json::Null) => Ok(Vec::new()),
            Some(Json::Arr(items)) => items
                .iter()
                .map(|v| v.as_i64().ok_or_else(|| Error::Json(format!("'{key}' must be a list of numbers"))))
                .collect(),
            Some(_) => Err(Error::Json(format!("'{key}' must be a list"))),
        }
    }

    pub fn opt_i64_list(&self, key: &str) -> Result<Option<Vec<i64>>> {
        if self.has(key) && !matches!(self.field(key), Some(Json::Null)) {
            Ok(Some(self.i64_list(key)?))
        } else {
            Ok(None)
        }
    }

    pub fn array(&self, key: &str) -> Result<&'a [Json]> {
        match self.field(key) {
            Some(Json::Arr(items)) => Ok(items),
            None | Some(Json::Null) => Ok(&[]),
            Some(_) => Err(Error::Json(format!("'{key}' must be a list"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let src = r#"{"a":1,"b":[true,false,null],"c":"hé\"llo\n","d":-2.5,"e":{"x":"😀"}}"#;
        let v = Json::parse(src).unwrap();
        assert_eq!(v.get("a").unwrap().as_i64(), Some(1));
        assert_eq!(v.get("c").unwrap().as_str(), Some("hé\"llo\n"));
        assert_eq!(v.get("d").unwrap().as_f64(), Some(-2.5));
        let again = Json::parse(&v.stringify()).unwrap();
        assert_eq!(v, again);
    }

    #[test]
    fn surrogates_and_errors() {
        assert_eq!(Json::parse(r#""😀""#).unwrap().as_str(), Some("😀"));
        assert!(Json::parse(r#""\ud83d""#).is_err());
        assert!(Json::parse("[1,]").is_err());
        assert!(Json::parse("{\"a\" 1}").is_err());
        assert!(Json::parse("01").is_err());
        assert!(Json::parse("1 2").is_err());
        assert!(Json::parse(&"[".repeat(500)).is_err());
        assert_eq!(Json::parse(" 12345678901 ").unwrap(), Json::Int(12345678901));
        assert_eq!(Json::parse("1e3").unwrap().as_i64(), Some(1000));
    }

    #[test]
    fn args_access() {
        let v = Json::parse(r#"{"id":3,"name":"x","rate":null,"tags":[1,2],"b":true}"#).unwrap();
        let a = Args::new(&v);
        assert_eq!(a.req_i64("id").unwrap(), 3);
        assert_eq!(a.patch_i64("rate").unwrap(), Some(None));
        assert_eq!(a.patch_i64("missing").unwrap(), None);
        assert_eq!(a.i64_list("tags").unwrap(), vec![1, 2]);
        assert!(a.req_i64("name").is_err());
        assert!(a.req_str("nope").is_err());
        assert_eq!(a.opt_bool("b").unwrap(), Some(true));
    }

    #[test]
    fn escapes_control_chars() {
        let s = Json::Str("a\u{1}b\u{2028}".into()).stringify();
        assert_eq!(s, "\"a\\u0001b\\u2028\"");
    }
}
