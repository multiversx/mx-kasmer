//! Parser for KORE text, accepting what pyk's parser accepts (`pyk.kore.parser`).
//!
//! Patterns are parsed with an explicit stack, so the nesting depth is limited by memory only.

use super::ParseError;
use super::lexer::{Keyword, Lexer, Tok};
use crate::quote::dequote;
use crate::syntax::*;

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    la: Tok<'a>,
    la_pos: usize,
}

/// A compound pattern whose arguments are being parsed.
enum Frame {
    App {
        symbol: Id,
        sorts: Vec<Sort>,
        args: Vec<Pattern>,
    },
    Ml {
        conn: Conn,
        sorts: Vec<Sort>,
        var: Option<Var>,
        args: Vec<Pattern>,
        pos: usize,
    },
    /// `\left-assoc{}(` or `\right-assoc{}(`: one application follows, then `)`.
    Assoc { dir: AssocDir },
}

impl<'a> Parser<'a> {
    pub fn new(src: &'a str) -> Result<Parser<'a>, ParseError> {
        let mut lexer = Lexer::new(src);
        let (la, la_pos) = lexer.next_token()?;
        Ok(Parser { lexer, la, la_pos })
    }

    fn error(&self, pos: usize, message: impl Into<String>) -> ParseError {
        ParseError::at(self.lexer.src(), pos, message)
    }

    fn unexpected(&self, expected: &str) -> ParseError {
        self.error(
            self.la_pos,
            format!("expected {expected}, found {}", self.la.describe()),
        )
    }

    fn advance(&mut self) -> Result<Tok<'a>, ParseError> {
        let (next, pos) = self.lexer.next_token()?;
        self.la_pos = pos;
        Ok(std::mem::replace(&mut self.la, next))
    }

    fn expect(&mut self, tok: Tok<'static>) -> Result<(), ParseError> {
        if self.la != tok {
            return Err(self.unexpected(&tok.describe()));
        }
        self.advance()?;
        Ok(())
    }

    pub fn at_eof(&self) -> bool {
        self.la == Tok::Eof
    }

    /// Fails unless all input has been consumed.
    pub fn expect_eof(&self) -> Result<(), ParseError> {
        if self.at_eof() {
            Ok(())
        } else {
            Err(self.unexpected("end of input"))
        }
    }

    fn id(&mut self) -> Result<&'a str, ParseError> {
        match self.la {
            Tok::Id(s) => {
                self.advance()?;
                Ok(s)
            }
            _ => Err(self.unexpected("an identifier")),
        }
    }

    /// A symbol name: an identifier, or `\` followed by one.
    fn symbol_id(&mut self) -> Result<&'a str, ParseError> {
        match self.la {
            Tok::Id(s) | Tok::SymbolId(s) => {
                self.advance()?;
                Ok(s)
            }
            _ => Err(self.unexpected("a symbol")),
        }
    }

    /// `X` or `X{S1, ..., Sn}`. Sorts are shallow; this recurses.
    pub fn sort(&mut self) -> Result<Sort, ParseError> {
        let name = self.id()?;
        if self.la == Tok::LBrace {
            let args = self.list(Tok::LBrace, Tok::RBrace, Self::sort)?;
            Ok(Sort::App(name.into(), args))
        } else {
            Ok(Sort::Var(name.into()))
        }
    }

    /// `open item, item, ... close`, possibly empty.
    fn list<T>(
        &mut self,
        open: Tok<'static>,
        close: Tok<'static>,
        mut item: impl FnMut(&mut Self) -> Result<T, ParseError>,
    ) -> Result<Vec<T>, ParseError> {
        self.expect(open)?;
        let mut items = Vec::new();
        if self.la == close {
            self.advance()?;
            return Ok(items);
        }
        loop {
            items.push(item(self)?);
            match self.la {
                Tok::Comma => {
                    self.advance()?;
                }
                t if t == close => {
                    self.advance()?;
                    return Ok(items);
                }
                _ => return Err(self.unexpected(&format!("',' or {}", close.describe()))),
            }
        }
    }

    fn sort_vars(&mut self) -> Result<Vec<Id>, ParseError> {
        self.list(Tok::LBrace, Tok::RBrace, |p| p.id().map(Id::from))
    }

    fn string(&mut self) -> Result<String, ParseError> {
        match self.la {
            Tok::Str(raw) => {
                let pos = self.la_pos;
                self.advance()?;
                dequote(raw).map_err(|e| self.error(pos, e))
            }
            _ => Err(self.unexpected("a string")),
        }
    }

    /// `name : sort`, element or set variable.
    fn var(&mut self) -> Result<Var, ParseError> {
        let name = match self.la {
            Tok::Id(s) | Tok::SetVarId(s) => s,
            _ => return Err(self.unexpected("a variable")),
        };
        self.advance()?;
        self.expect(Tok::Colon)?;
        Ok(Var::new(name, self.sort()?))
    }

    /// Parses the start of a pattern. Returns the pattern if it has no subpatterns; otherwise
    /// pushes a frame, having consumed the opening parenthesis (and the bound variable and its
    /// comma, for binders).
    fn head(&mut self, stack: &mut Vec<Frame>) -> Result<Option<Pattern>, ParseError> {
        let pos = self.la_pos;
        match self.la {
            Tok::Str(_) => Ok(Some(Pattern::String(self.string()?))),
            Tok::SetVarId(name) => {
                self.advance()?;
                self.expect(Tok::Colon)?;
                Ok(Some(Pattern::SVar(Var::new(name, self.sort()?))))
            }
            Tok::Id(name) => {
                self.advance()?;
                if self.la == Tok::Colon {
                    self.advance()?;
                    return Ok(Some(Pattern::EVar(Var::new(name, self.sort()?))));
                }
                self.app_head(name, stack)?;
                Ok(None)
            }
            Tok::SymbolId(name) => {
                self.advance()?;
                self.app_head(name, stack)?;
                Ok(None)
            }
            Tok::Dv => {
                self.advance()?;
                self.expect(Tok::LBrace)?;
                let sort = self.sort()?;
                self.expect(Tok::RBrace)?;
                self.expect(Tok::LParen)?;
                let value = self.string()?;
                self.expect(Tok::RParen)?;
                Ok(Some(Pattern::Dv { sort, value }))
            }
            Tok::LeftAssoc | Tok::RightAssoc => {
                let dir = if self.la == Tok::LeftAssoc {
                    AssocDir::Left
                } else {
                    AssocDir::Right
                };
                self.advance()?;
                self.expect(Tok::LBrace)?;
                self.expect(Tok::RBrace)?;
                self.expect(Tok::LParen)?;
                stack.push(Frame::Assoc { dir });
                // The application inside.
                let name = self.symbol_id()?;
                self.app_head(name, stack)?;
                Ok(None)
            }
            Tok::Ml(conn) => {
                self.advance()?;
                let sorts = self.list(Tok::LBrace, Tok::RBrace, Self::sort)?;
                self.expect(Tok::LParen)?;
                let var = if conn.shape().binder != Binder::None {
                    let v = self.var()?;
                    self.expect(Tok::Comma)?;
                    Some(v)
                } else {
                    None
                };
                stack.push(Frame::Ml {
                    conn,
                    sorts,
                    var,
                    args: Vec::new(),
                    pos,
                });
                Ok(None)
            }
            _ => Err(self.unexpected("a pattern")),
        }
    }

    /// `{sorts}(` after a symbol name.
    fn app_head(&mut self, name: &str, stack: &mut Vec<Frame>) -> Result<(), ParseError> {
        let sorts = self.list(Tok::LBrace, Tok::RBrace, Self::sort)?;
        self.expect(Tok::LParen)?;
        stack.push(Frame::App {
            symbol: name.into(),
            sorts,
            args: Vec::new(),
        });
        Ok(())
    }

    fn close(&self, frame: Frame) -> Result<Pattern, ParseError> {
        match frame {
            Frame::App {
                symbol,
                sorts,
                args,
            } => Ok(Pattern::App(App {
                symbol,
                sorts,
                args: args.into(),
            })),
            Frame::Ml {
                conn,
                sorts,
                var,
                args,
                pos,
            } => Ml::new(conn, sorts, var, args)
                .map(Pattern::Ml)
                .map_err(|e| self.error(pos, e)),
            Frame::Assoc { .. } => unreachable!("closed in `pattern`"),
        }
    }

    pub fn pattern(&mut self) -> Result<Pattern, ParseError> {
        let mut stack: Vec<Frame> = Vec::new();
        let mut done = self.head(&mut stack)?;
        loop {
            // `done`: a complete pattern, to be added to the frame on top of the stack.
            // Otherwise: a frame was just opened, its argument list may be empty.
            let Some(p) = done.take() else {
                let empty = matches!(stack.last(), Some(Frame::App { .. } | Frame::Ml { .. }))
                    && self.la == Tok::RParen;
                if empty {
                    self.advance()?;
                    let frame = stack.pop().expect("frame just pushed");
                    done = Some(self.close(frame)?);
                } else {
                    done = self.head(&mut stack)?;
                }
                continue;
            };
            let Some(top) = stack.last_mut() else {
                return Ok(p);
            };
            match top {
                Frame::Assoc { dir } => {
                    let dir = *dir;
                    stack.pop();
                    let Pattern::App(app) = p else {
                        unreachable!("assoc frame holds an application")
                    };
                    self.expect(Tok::RParen)?;
                    done = Some(Pattern::Assoc(Assoc { dir, app }));
                }
                Frame::App { args, .. } | Frame::Ml { args, .. } => {
                    args.push(p);
                    match self.la {
                        Tok::Comma => {
                            self.advance()?;
                            done = self.head(&mut stack)?;
                        }
                        Tok::RParen => {
                            self.advance()?;
                            let frame = stack.pop().expect("frame on stack");
                            done = Some(self.close(frame)?);
                        }
                        _ => return Err(self.unexpected("',' or ')'")),
                    }
                }
            }
        }
    }

    /// An application, as in attributes and alias left-hand sides.
    fn app(&mut self) -> Result<App, ParseError> {
        let pos = self.la_pos;
        match self.la {
            Tok::Id(_) | Tok::SymbolId(_) => {}
            _ => return Err(self.unexpected("a symbol application")),
        }
        match self.pattern()? {
            Pattern::App(app) => Ok(app),
            _ => Err(self.error(pos, "expected a symbol application")),
        }
    }

    fn attrs(&mut self) -> Result<Vec<Attr>, ParseError> {
        self.list(Tok::LBrack, Tok::RBrack, Self::app)
    }

    /// `name{vars}(param sorts) : sort`, shared by symbol and alias declarations.
    fn symbol_signature(&mut self) -> Result<(Id, Vec<Id>, Vec<Sort>, Sort), ParseError> {
        let name = self.symbol_id()?.into();
        let vars = self.sort_vars()?;
        let params = self.list(Tok::LParen, Tok::RParen, Self::sort)?;
        self.expect(Tok::Colon)?;
        let sort = self.sort()?;
        Ok((name, vars, params, sort))
    }

    pub fn sentence(&mut self) -> Result<Sentence, ParseError> {
        let Tok::Kw(kw) = self.la else {
            return Err(self.unexpected("a sentence"));
        };
        self.advance()?;
        Ok(match kw {
            Keyword::Import => {
                let module = self.id()?.into();
                Sentence::Import {
                    module,
                    attrs: self.attrs()?,
                }
            }
            Keyword::Sort | Keyword::HookedSort => {
                let name = self.id()?.into();
                let vars = self.sort_vars()?;
                let attrs = self.attrs()?;
                Sentence::Sort(SortDecl {
                    name,
                    vars,
                    hooked: kw == Keyword::HookedSort,
                    attrs,
                })
            }
            Keyword::Symbol | Keyword::HookedSymbol => {
                let (name, vars, params, sort) = self.symbol_signature()?;
                let attrs = self.attrs()?;
                Sentence::Symbol(SymbolDecl {
                    name,
                    vars,
                    params,
                    sort,
                    hooked: kw == Keyword::HookedSymbol,
                    attrs,
                })
            }
            Keyword::Alias => {
                let (name, vars, params, sort) = self.symbol_signature()?;
                self.expect(Tok::Kw(Keyword::Where))?;
                let left = self.app()?;
                self.expect(Tok::Walrus)?;
                let right = self.pattern()?;
                let attrs = self.attrs()?;
                Sentence::Alias(AliasDecl {
                    name,
                    vars,
                    params,
                    sort,
                    left,
                    right,
                    attrs,
                })
            }
            Keyword::Axiom | Keyword::Claim => {
                let vars = self.sort_vars()?;
                let pattern = self.pattern()?;
                let attrs = self.attrs()?;
                let axiom = Axiom {
                    vars,
                    pattern,
                    attrs,
                };
                if kw == Keyword::Axiom {
                    Sentence::Axiom(axiom)
                } else {
                    Sentence::Claim(axiom)
                }
            }
            _ => return Err(self.error(self.la_pos, format!("unexpected keyword {kw:?}"))),
        })
    }

    pub fn module(&mut self) -> Result<Module, ParseError> {
        self.expect(Tok::Kw(Keyword::Module))?;
        let name = self.id()?.into();
        let mut sentences = Vec::new();
        while self.la != Tok::Kw(Keyword::EndModule) {
            sentences.push(self.sentence()?);
        }
        self.advance()?;
        let attrs = self.attrs()?;
        Ok(Module {
            name,
            sentences,
            attrs,
        })
    }

    pub fn definition(&mut self) -> Result<Definition, ParseError> {
        let attrs = self.attrs()?;
        let mut modules = Vec::new();
        while !self.at_eof() {
            modules.push(self.module()?);
        }
        Ok(Definition { attrs, modules })
    }
}
