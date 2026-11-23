//! Tokens of KORE text, as in pyk's lexer (`pyk.kore.lexer`).

use super::ParseError;
use crate::syntax::Conn;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tok<'a> {
    Eof,
    Comma,
    Colon,
    /// `:=`, in alias declarations
    Walrus,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBrack,
    RBrack,
    /// A string literal, still quoted, without the double quotes.
    Str(&'a str),
    /// Identifier: letters, digits, `'` and `-`, starting with a letter.
    Id(&'a str),
    /// `\` followed by an identifier that is not a matching-logic symbol.
    SymbolId(&'a str),
    /// `@` followed by an identifier; the text includes the `@`.
    SetVarId(&'a str),
    Ml(Conn),
    Dv,
    LeftAssoc,
    RightAssoc,
    Kw(Keyword),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Keyword {
    Module,
    EndModule,
    Import,
    Sort,
    HookedSort,
    Symbol,
    HookedSymbol,
    Axiom,
    Claim,
    Alias,
    Where,
}

impl Keyword {
    fn from_id(id: &str) -> Option<Keyword> {
        Some(match id {
            "module" => Keyword::Module,
            "endmodule" => Keyword::EndModule,
            "import" => Keyword::Import,
            "sort" => Keyword::Sort,
            "hooked-sort" => Keyword::HookedSort,
            "symbol" => Keyword::Symbol,
            "hooked-symbol" => Keyword::HookedSymbol,
            "axiom" => Keyword::Axiom,
            "claim" => Keyword::Claim,
            "alias" => Keyword::Alias,
            "where" => Keyword::Where,
            _ => return None,
        })
    }
}

impl Tok<'_> {
    /// For error messages.
    pub fn describe(&self) -> String {
        match self {
            Tok::Eof => "end of input".into(),
            Tok::Comma => "','".into(),
            Tok::Colon => "':'".into(),
            Tok::Walrus => "':='".into(),
            Tok::LParen => "'('".into(),
            Tok::RParen => "')'".into(),
            Tok::LBrace => "'{'".into(),
            Tok::RBrace => "'}'".into(),
            Tok::LBrack => "'['".into(),
            Tok::RBrack => "']'".into(),
            Tok::Str(_) => "a string".into(),
            Tok::Id(s) | Tok::SymbolId(s) | Tok::SetVarId(s) => format!("'{s}'"),
            Tok::Ml(c) => format!("'{}'", c.symbol()),
            Tok::Dv => r"'\dv'".into(),
            Tok::LeftAssoc => r"'\left-assoc'".into(),
            Tok::RightAssoc => r"'\right-assoc'".into(),
            Tok::Kw(k) => format!("keyword {k:?}"),
        }
    }
}

pub struct Lexer<'a> {
    src: &'a str,
    pos: usize,
}

fn is_id_first(b: u8) -> bool {
    b.is_ascii_alphabetic()
}

fn is_id_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'\'' || b == b'-'
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Lexer<'a> {
        Lexer { src, pos: 0 }
    }

    pub fn src(&self) -> &'a str {
        self.src
    }

    fn peek(&self) -> Option<u8> {
        self.src.as_bytes().get(self.pos).copied()
    }

    fn error(&self, pos: usize, message: impl Into<String>) -> ParseError {
        ParseError::at(self.src, pos, message)
    }

    /// Skips whitespace and comments.
    fn skip_trivia(&mut self) -> Result<(), ParseError> {
        let bytes = self.src.as_bytes();
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\n' | b'\r') => self.pos += 1,
                Some(b'/') => {
                    let start = self.pos;
                    match bytes.get(self.pos + 1) {
                        Some(b'/') => {
                            while let Some(b) = self.peek() {
                                self.pos += 1;
                                if b == b'\n' {
                                    break;
                                }
                            }
                        }
                        Some(b'*') => {
                            self.pos += 2;
                            loop {
                                match self.peek() {
                                    None => return Err(self.error(start, "unterminated comment")),
                                    Some(b'*') if bytes.get(self.pos + 1) == Some(&b'/') => {
                                        self.pos += 2;
                                        break;
                                    }
                                    Some(_) => self.pos += 1,
                                }
                            }
                        }
                        _ => return Err(self.error(start, "expected '/' or '*' after '/'")),
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    /// The identifier starting at `self.pos`, which must start with a letter.
    fn ident(&mut self, start: usize) -> Result<&'a str, ParseError> {
        match self.peek() {
            Some(b) if is_id_first(b) => {}
            _ => return Err(self.error(self.pos, "expected a letter")),
        }
        while self.peek().is_some_and(is_id_char) {
            self.pos += 1;
        }
        Ok(&self.src[start..self.pos])
    }

    /// The next token and its start position.
    pub fn next_token(&mut self) -> Result<(Tok<'a>, usize), ParseError> {
        self.skip_trivia()?;
        let start = self.pos;
        let Some(b) = self.peek() else {
            return Ok((Tok::Eof, start));
        };
        let simple = |t| Some(t);
        let tok = match b {
            b',' => simple(Tok::Comma),
            b'(' => simple(Tok::LParen),
            b')' => simple(Tok::RParen),
            b'{' => simple(Tok::LBrace),
            b'}' => simple(Tok::RBrace),
            b'[' => simple(Tok::LBrack),
            b']' => simple(Tok::RBrack),
            _ => None,
        };
        if let Some(t) = tok {
            self.pos += 1;
            return Ok((t, start));
        }
        let tok = match b {
            b':' => {
                self.pos += 1;
                if self.peek() == Some(b'=') {
                    self.pos += 1;
                    Tok::Walrus
                } else {
                    Tok::Colon
                }
            }
            b'"' => {
                self.pos += 1;
                let content_start = self.pos;
                loop {
                    match self.peek() {
                        None => return Err(self.error(start, "unterminated string")),
                        Some(b'"') => break,
                        // An escape: skip the next byte, whatever it is.
                        Some(b'\\') => self.pos += 2,
                        Some(_) => self.pos += 1,
                    }
                }
                if self.pos > self.src.len() {
                    return Err(self.error(start, "unterminated string"));
                }
                let content = &self.src[content_start..self.pos];
                self.pos += 1;
                Tok::Str(content)
            }
            b'\\' => {
                self.pos += 1;
                let id = self.ident(start)?;
                match id {
                    r"\dv" => Tok::Dv,
                    r"\left-assoc" => Tok::LeftAssoc,
                    r"\right-assoc" => Tok::RightAssoc,
                    _ => match Conn::from_symbol(id) {
                        Some(c) => Tok::Ml(c),
                        None => Tok::SymbolId(id),
                    },
                }
            }
            b'@' => {
                self.pos += 1;
                Tok::SetVarId(self.ident(start)?)
            }
            b if is_id_first(b) => {
                let id = self.ident(start)?;
                match Keyword::from_id(id) {
                    Some(k) => Tok::Kw(k),
                    None => Tok::Id(id),
                }
            }
            _ => {
                let c = self.src[start..].chars().next().unwrap_or('?');
                return Err(self.error(start, format!("unexpected character: {c:?}")));
            }
        };
        Ok((tok, start))
    }
}
