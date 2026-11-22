//! Reads KORE JSON. A small JSON parser with an explicit stack that turns each object into a
//! pattern or a sort as soon as it closes, so deep terms need no recursion and no intermediate
//! JSON tree is kept.

use std::fmt;

use crate::syntax::*;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JsonError {
    /// Byte offset in the input.
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for JsonError {}

/// Objects and arrays that are not patterns or sorts may nest this deep. Only the wrapper of the
/// format and its fields are such objects, so this is plenty; it keeps their (recursive) drop
/// shallow.
const MAX_GENERIC_DEPTH: u32 = 64;

enum Node {
    Str(String),
    Num(String),
    Bool,
    Null,
    Arr { items: Vec<Node>, depth: u32 },
    Obj { fields: Vec<(String, Node)>, depth: u32 },
    Pattern(Pattern),
    Sort(Sort),
}

impl Node {
    fn generic_depth(&self) -> u32 {
        match self {
            Node::Arr { depth, .. } | Node::Obj { depth, .. } => *depth,
            _ => 0,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Node::Str(_) => "a string",
            Node::Num(_) => "a number",
            Node::Bool => "a boolean",
            Node::Null => "null",
            Node::Arr { .. } => "an array",
            Node::Obj { .. } => "an object",
            Node::Pattern(_) => "a pattern",
            Node::Sort(_) => "a sort",
        }
    }
}

enum Frame {
    Arr(Vec<Node>),
    Obj {
        fields: Vec<(String, Node)>,
        key: String,
        start: usize,
    },
}

struct Reader<'a> {
    src: &'a [u8],
    pos: usize,
}

type R<T> = Result<T, JsonError>;

impl<'a> Reader<'a> {
    fn error<T>(&self, offset: usize, message: impl Into<String>) -> R<T> {
        Err(JsonError {
            offset,
            message: message.into(),
        })
    }

    fn skip_ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.src.get(self.pos) {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.src.get(self.pos).copied()
    }

    fn expect(&mut self, b: u8) -> R<()> {
        if self.peek() == Some(b) {
            self.pos += 1;
            Ok(())
        } else {
            self.error(self.pos, format!("expected '{}'", b as char))
        }
    }

    fn string(&mut self) -> R<String> {
        let start = self.pos;
        self.expect(b'"')?;
        let mut out = Vec::new();
        loop {
            // Copy the run of plain bytes up to the next quote or backslash.
            let run_start = self.pos;
            while let Some(&b) = self.src.get(self.pos) {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            out.extend_from_slice(&self.src[run_start..self.pos]);
            match self.src.get(self.pos) {
                None => return self.error(start, "unterminated string"),
                Some(b'"') => {
                    self.pos += 1;
                    break;
                }
                Some(b'\\') => {
                    let esc = self.src.get(self.pos + 1).copied();
                    self.pos += 2;
                    let c = match esc {
                        Some(b'"') => '"',
                        Some(b'\\') => '\\',
                        Some(b'/') => '/',
                        Some(b'b') => '\x08',
                        Some(b'f') => '\x0c',
                        Some(b'n') => '\n',
                        Some(b'r') => '\r',
                        Some(b't') => '\t',
                        Some(b'u') => self.unicode_escape()?,
                        _ => return self.error(self.pos - 2, "invalid escape sequence"),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
                Some(_) => return self.error(self.pos, "control character in string"),
            }
        }
        String::from_utf8(out).or_else(|_| self.error(start, "string is not valid UTF-8"))
    }

    fn hex4(&mut self) -> R<u32> {
        let digits = self.src.get(self.pos..self.pos + 4);
        let value = digits
            .and_then(|d| std::str::from_utf8(d).ok())
            .and_then(|d| u32::from_str_radix(d, 16).ok());
        match value {
            Some(v) => {
                self.pos += 4;
                Ok(v)
            }
            None => self.error(self.pos, "expected four hex digits"),
        }
    }

    /// After `\u`: one code unit, or a surrogate pair.
    fn unicode_escape(&mut self) -> R<char> {
        let start = self.pos - 2;
        let first = self.hex4()?;
        let code = if (0xd800..0xdc00).contains(&first) {
            if self.src.get(self.pos..self.pos + 2) != Some(b"\\u") {
                return self.error(start, "unpaired surrogate");
            }
            self.pos += 2;
            let second = self.hex4()?;
            if !(0xdc00..0xe000).contains(&second) {
                return self.error(start, "unpaired surrogate");
            }
            0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00)
        } else {
            first
        };
        match char::from_u32(code) {
            Some(c) => Ok(c),
            None => self.error(start, "unpaired surrogate"),
        }
    }

    fn scalar(&mut self) -> R<Node> {
        let start = self.pos;
        match self.peek() {
            Some(b'"') => Ok(Node::Str(self.string()?)),
            Some(b't') if self.src[self.pos..].starts_with(b"true") => {
                self.pos += 4;
                Ok(Node::Bool)
            }
            Some(b'f') if self.src[self.pos..].starts_with(b"false") => {
                self.pos += 5;
                Ok(Node::Bool)
            }
            Some(b'n') if self.src[self.pos..].starts_with(b"null") => {
                self.pos += 4;
                Ok(Node::Null)
            }
            Some(b'-' | b'0'..=b'9') => {
                while let Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') = self.src.get(self.pos) {
                    self.pos += 1;
                }
                let text = std::str::from_utf8(&self.src[start..self.pos]).expect("ASCII");
                Ok(Node::Num(text.to_owned()))
            }
            Some(_) => self.error(start, "expected a JSON value"),
            None => self.error(start, "unexpected end of input"),
        }
    }

    /// Reads one JSON value, converting KORE objects on the way.
    fn value(&mut self) -> R<Node> {
        let mut stack: Vec<Frame> = Vec::new();
        loop {
            // Start a value: open a container or read a scalar.
            let mut node = match self.peek() {
                Some(b'{') => {
                    let start = self.pos;
                    self.pos += 1;
                    if self.peek() == Some(b'}') {
                        self.pos += 1;
                        close_object(Vec::new(), start)?
                    } else {
                        let key = self.string()?;
                        self.expect(b':')?;
                        stack.push(Frame::Obj {
                            fields: Vec::new(),
                            key,
                            start,
                        });
                        continue;
                    }
                }
                Some(b'[') => {
                    self.pos += 1;
                    if self.peek() == Some(b']') {
                        self.pos += 1;
                        Node::Arr {
                            items: Vec::new(),
                            depth: 1,
                        }
                    } else {
                        stack.push(Frame::Arr(Vec::new()));
                        continue;
                    }
                }
                _ => self.scalar()?,
            };
            // Attach the finished value to its container; close containers that end here.
            loop {
                let Some(frame) = stack.last_mut() else {
                    return Ok(node);
                };
                match frame {
                    Frame::Arr(items) => {
                        items.push(node);
                        match self.peek() {
                            Some(b',') => {
                                self.pos += 1;
                                break;
                            }
                            Some(b']') => {
                                self.pos += 1;
                                let Some(Frame::Arr(items)) = stack.pop() else {
                                    unreachable!()
                                };
                                let depth = 1 + items.iter().map(Node::generic_depth).max().unwrap_or(0);
                                if depth > MAX_GENERIC_DEPTH {
                                    return self.error(self.pos, "arrays nested too deep");
                                }
                                node = Node::Arr { items, depth };
                            }
                            _ => return self.error(self.pos, "expected ',' or ']'"),
                        }
                    }
                    Frame::Obj { fields, key, .. } => {
                        fields.push((std::mem::take(key), node));
                        match self.peek() {
                            Some(b',') => {
                                self.pos += 1;
                                *key = self.string()?;
                                self.expect(b':')?;
                                break;
                            }
                            Some(b'}') => {
                                self.pos += 1;
                                let Some(Frame::Obj { fields, start, .. }) = stack.pop() else {
                                    unreachable!()
                                };
                                node = close_object(fields, start)?;
                            }
                            _ => return self.error(self.pos, "expected ',' or '}'"),
                        }
                    }
                }
            }
        }
    }
}

/// The fields of one object, taken by name.
struct Fields {
    fields: Vec<(String, Node)>,
    start: usize,
    tag: String,
}

impl Fields {
    fn err<T>(&self, message: impl Into<String>) -> R<T> {
        Err(JsonError {
            offset: self.start,
            message: format!("{}: {}", self.tag, message.into()),
        })
    }

    fn take(&mut self, name: &str) -> R<Node> {
        match self.fields.iter().position(|(k, _)| k == name) {
            Some(i) => Ok(self.fields.swap_remove(i).1),
            None => self.err(format!("missing field '{name}'")),
        }
    }

    fn string(&mut self, name: &str) -> R<String> {
        match self.take(name)? {
            Node::Str(s) => Ok(s),
            other => self.err(format!("'{name}' must be a string, not {}", other.kind())),
        }
    }

    fn id(&mut self, name: &str) -> R<Id> {
        self.string(name).map(Id::from)
    }

    fn sort(&mut self, name: &str) -> R<Sort> {
        match self.take(name)? {
            Node::Sort(s) => Ok(s),
            other => self.err(format!("'{name}' must be a sort, not {}", other.kind())),
        }
    }

    fn pattern(&mut self, name: &str) -> R<Pattern> {
        match self.take(name)? {
            Node::Pattern(p) => Ok(p),
            other => self.err(format!("'{name}' must be a pattern, not {}", other.kind())),
        }
    }

    fn array(&mut self, name: &str) -> R<Vec<Node>> {
        match self.take(name)? {
            Node::Arr { items, .. } => Ok(items),
            other => self.err(format!("'{name}' must be an array, not {}", other.kind())),
        }
    }

    fn sorts(&mut self, name: &str) -> R<Vec<Sort>> {
        let items = self.array(name)?;
        items
            .into_iter()
            .map(|n| match n {
                Node::Sort(s) => Ok(s),
                other => self.err(format!("'{name}' must hold sorts, not {}", other.kind())),
            })
            .collect()
    }

    fn patterns(&mut self, name: &str) -> R<Vec<Pattern>> {
        let items = self.array(name)?;
        items
            .into_iter()
            .map(|n| match n {
                Node::Pattern(p) => Ok(p),
                other => self.err(format!("'{name}' must hold patterns, not {}", other.kind())),
            })
            .collect()
    }

    fn var(&mut self, name: &str, sort: &str) -> R<Var> {
        Ok(Var {
            name: self.id(name)?,
            sort: self.sort(sort)?,
        })
    }
}

fn close_object(fields: Vec<(String, Node)>, start: usize) -> R<Node> {
    let tag = match fields.iter().find(|(k, _)| k == "tag") {
        Some((_, Node::Str(t))) => t.clone(),
        _ => {
            let depth = 1 + fields.iter().map(|(_, n)| n.generic_depth()).max().unwrap_or(0);
            if depth > MAX_GENERIC_DEPTH {
                return Err(JsonError {
                    offset: start,
                    message: "objects nested too deep".into(),
                });
            }
            return Ok(Node::Obj { fields, depth });
        }
    };
    let mut f = Fields { fields, start, tag };
    let tag = f.tag.as_str();
    let node = match tag {
        "SortVar" => Node::Sort(Sort::Var(f.id("name")?)),
        "SortApp" => Node::Sort(Sort::App(f.id("name")?, f.sorts("args")?)),
        "EVar" => Node::Pattern(Pattern::EVar(f.var("name", "sort")?)),
        "SVar" => {
            let v = f.var("name", "sort")?;
            if !v.is_set_var() {
                return f.err("set variable names start with '@'");
            }
            Node::Pattern(Pattern::SVar(v))
        }
        "String" => Node::Pattern(Pattern::String(f.string("value")?)),
        "DV" => Node::Pattern(Pattern::Dv {
            sort: f.sort("sort")?,
            value: f.string("value")?,
        }),
        "App" => Node::Pattern(Pattern::App(App {
            symbol: f.id("name")?,
            sorts: f.sorts("sorts")?,
            args: f.patterns("args")?.into(),
        })),
        "LeftAssoc" | "RightAssoc" => {
            let dir = if tag == "LeftAssoc" {
                AssocDir::Left
            } else {
                AssocDir::Right
            };
            let app = App {
                symbol: f.id("symbol")?,
                sorts: f.sorts("sorts")?,
                args: f.patterns("argss")?.into(),
            };
            Node::Pattern(Pattern::Assoc(Assoc { dir, app }))
        }
        _ => match Conn::from_json_tag(tag) {
            Some(conn) => Node::Pattern(Pattern::Ml(ml(conn, &mut f)?)),
            None => return f.err("unknown tag"),
        },
    };
    Ok(node)
}

fn ml(conn: Conn, f: &mut Fields) -> R<Ml> {
    let (sorts, var, args) = match conn {
        Conn::Top | Conn::Bottom => (vec![f.sort("sort")?], None, vec![]),
        Conn::Not => (vec![f.sort("sort")?], None, vec![f.pattern("arg")?]),
        Conn::Implies | Conn::Iff => (
            vec![f.sort("sort")?],
            None,
            vec![f.pattern("first")?, f.pattern("second")?],
        ),
        Conn::And | Conn::Or => (vec![f.sort("sort")?], None, f.patterns("patterns")?),
        Conn::Exists | Conn::Forall => (
            vec![f.sort("sort")?],
            Some(f.var("var", "varSort")?),
            vec![f.pattern("arg")?],
        ),
        Conn::Mu | Conn::Nu => (vec![], Some(f.var("var", "varSort")?), vec![f.pattern("arg")?]),
        Conn::Ceil | Conn::Floor => (vec![f.sort("argSort")?, f.sort("sort")?], None, vec![f.pattern("arg")?]),
        Conn::Equals | Conn::In => (
            vec![f.sort("argSort")?, f.sort("sort")?],
            None,
            vec![f.pattern("first")?, f.pattern("second")?],
        ),
        Conn::Next => (vec![f.sort("sort")?], None, vec![f.pattern("dest")?]),
        Conn::Rewrites => (
            vec![f.sort("sort")?],
            None,
            vec![f.pattern("source")?, f.pattern("dest")?],
        ),
    };
    Ml::new(conn, sorts, var, args).or_else(|e| f.err(e))
}

fn read_all(src: &str) -> R<Node> {
    let mut reader = Reader {
        src: src.as_bytes(),
        pos: 0,
    };
    let node = reader.value()?;
    reader.skip_ws();
    if reader.pos != reader.src.len() {
        return reader.error(reader.pos, "trailing characters");
    }
    Ok(node)
}

/// Reads a pattern in KORE JSON, without the wrapper.
pub fn parse_pattern(src: &str) -> R<Pattern> {
    match read_all(src)? {
        Node::Pattern(p) => Ok(p),
        other => Err(JsonError {
            offset: 0,
            message: format!("expected a pattern, found {}", other.kind()),
        }),
    }
}

/// Reads a KORE JSON term: `{"format": "KORE", "version": 1, "term": pattern}`.
pub fn parse_term(src: &str) -> R<Pattern> {
    let fields = match read_all(src)? {
        Node::Obj { fields, .. } => fields,
        other => {
            return Err(JsonError {
                offset: 0,
                message: format!("expected an object, found {}", other.kind()),
            });
        }
    };
    let mut f = Fields {
        fields,
        start: 0,
        tag: "KORE term".into(),
    };
    match f.take("format")? {
        Node::Str(s) if s == "KORE" => {}
        _ => return f.err("'format' must be \"KORE\""),
    }
    match f.take("version")? {
        Node::Num(n) if n == "1" => {}
        _ => return f.err("'version' must be 1"),
    }
    f.pattern("term")
}
