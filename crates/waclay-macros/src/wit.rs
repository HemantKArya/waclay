//! A small, dependency-free WIT parser covering the subset waclay 2.0
//! binds: packages, interfaces, worlds, records, variants, enums, flags,
//! resources, type aliases, `use` and functions.
//!
//! Deliberately minimal: clear errors over exhaustive spec coverage.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Ident(String),
    Punct(char),
    Eof,
}

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
}

impl Lexer {
    pub fn new(src: &str) -> Self {
        Lexer {
            chars: src.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                Some('/') if self.chars.get(self.pos + 1) == Some(&'/') => {
                    while let Some(c) = self.bump() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
                Some('/') if self.chars.get(self.pos + 1) == Some(&'*') => {
                    self.bump();
                    self.bump();
                    let mut depth = 1;
                    while depth > 0 {
                        match self.bump() {
                            Some('*') if self.peek() == Some('/') => {
                                self.bump();
                                depth -= 1;
                            }
                            Some('/') if self.peek() == Some('*') => {
                                self.bump();
                                depth += 1;
                            }
                            Some(_) => {}
                            None => break,
                        }
                    }
                }
                _ => break,
            }
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_trivia();
        match self.peek() {
            None => Token::Eof,
            Some(c) if c.is_alphabetic() || c == '_' || c == '%' => {
                let mut s = String::new();
                if c == '%' {
                    self.bump();
                }
                while let Some(c) = self.peek() {
                    if c.is_alphanumeric() || c == '_' || c == '-' {
                        s.push(c);
                        self.bump();
                    } else {
                        break;
                    }
                }
                Token::Ident(s)
            }
            Some(c) if "(){}[]<>,;:=.@/+-".contains(c) => {
                // `->` arrow
                if c == '-' && self.chars.get(self.pos + 1) == Some(&'>') {
                    self.bump();
                    self.bump();
                    Token::Punct('→')
                } else {
                    self.bump();
                    Token::Punct(c)
                }
            }
            Some(_) => {
                self.bump();
                Token::Punct('?')
            }
        }
    }

    /// Tokenize everything.
    pub fn tokenize(mut self) -> Vec<Token> {
        let mut out = Vec::new();
        loop {
            let t = self.next_token();
            let done = t == Token::Eof;
            out.push(t);
            if done {
                break;
            }
        }
        out
    }
}

/// A WIT type expression.
#[derive(Debug, Clone, PartialEq)]
pub enum WitType {
    Bool,
    S8,
    U8,
    S16,
    U16,
    S32,
    U32,
    S64,
    U64,
    F32,
    F64,
    Char,
    String,
    Named(String),
    List(Box<WitType>),
    Option(Box<WitType>),
    /// `stream<T>` — the runtime value is an opaque waitable handle
    /// (`waclay::Val::Stream`); the payload type is checked by the host.
    Stream(Option<Box<WitType>>),
    /// `future<T>` — see [`WitType::Stream`].
    Future(Option<Box<WitType>>),
    Result(Box<Option<WitType>>, Box<Option<WitType>>),
    Tuple(Vec<WitType>),
    Own(String),
    Borrow(String),
}

/// A named type declaration.
#[derive(Debug, Clone)]
pub enum TypeDecl {
    Record {
        name: String,
        fields: Vec<(String, WitType)>,
    },
    Variant {
        name: String,
        cases: Vec<(String, Option<WitType>)>,
    },
    Enum {
        name: String,
        cases: Vec<String>,
    },
    Flags {
        name: String,
        cases: Vec<String>,
    },
    Resource {
        name: String,
    },
    Alias {
        name: String,
        ty: WitType,
    },
}

/// A function declaration.
#[derive(Debug, Clone)]
pub struct FuncDecl {
    pub name: String,
    pub params: Vec<(String, WitType)>,
    pub result: Option<WitType>,
}

/// Declarations inside an interface or world.
#[derive(Debug, Clone)]
pub enum Item {
    Type(TypeDecl),
    Func(FuncDecl),
    /// `use other.{A, B};`
    Use {
        #[allow(dead_code)] // consumed by cross-interface resolution later
        from: String,
        #[allow(dead_code)]
        names: Vec<String>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Interface {
    pub name: String,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub enum WorldItem {
    /// `import iface-name;` / `import ns:pkg/iface@v;`
    ImportInterface(String),
    /// `import name: func(...) -> ...;`
    ImportFunc(#[allow(dead_code)] FuncDecl),
    /// `export iface-name;`
    ExportInterface(String),
    /// `export name: func(...) -> ...;`
    ExportFunc(FuncDecl),
    Type(TypeDecl),
    Use {
        #[allow(dead_code)] // consumed by cross-interface resolution later
        from: String,
        #[allow(dead_code)]
        names: Vec<String>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct World {
    pub name: String,
    pub items: Vec<WorldItem>,
}

#[derive(Debug, Clone, Default)]
pub struct Package {
    pub name: Option<String>,
    pub interfaces: Vec<Interface>,
    pub worlds: Vec<World>,
}

#[derive(Debug)]
pub struct ParseError {
    pub msg: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WIT parse error: {}", self.msg)
    }
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn bump(&mut self) -> Token {
        let t = self.peek().clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.bump() {
            Token::Ident(s) => Ok(s),
            t => Err(ParseError {
                msg: format!("expected identifier, found {t:?}"),
            }),
        }
    }

    fn expect_punct(&mut self, p: char) -> Result<(), ParseError> {
        match self.bump() {
            Token::Punct(c) if c == p => Ok(()),
            t => Err(ParseError {
                msg: format!("expected `{p}`, found {t:?}"),
            }),
        }
    }

    fn eat_punct(&mut self, p: char) -> bool {
        if self.peek() == &Token::Punct(p) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn eat_keyword(&mut self, kw: &str) -> bool {
        if self.peek() == &Token::Ident(kw.to_string()) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn parse_type(&mut self) -> Result<WitType, ParseError> {
        match self.bump() {
            Token::Ident(id) => match id.as_str() {
                "bool" => Ok(WitType::Bool),
                "s8" => Ok(WitType::S8),
                "u8" => Ok(WitType::U8),
                "s16" => Ok(WitType::S16),
                "u16" => Ok(WitType::U16),
                "s32" => Ok(WitType::S32),
                "u32" => Ok(WitType::U32),
                "s64" => Ok(WitType::S64),
                "u64" => Ok(WitType::U64),
                "f32" => Ok(WitType::F32),
                "f64" => Ok(WitType::F64),
                "char" => Ok(WitType::Char),
                "string" => Ok(WitType::String),
                "list" => {
                    self.expect_punct('<')?;
                    let inner = self.parse_type()?;
                    self.expect_punct('>')?;
                    Ok(WitType::List(Box::new(inner)))
                }
                "option" => {
                    self.expect_punct('<')?;
                    let inner = self.parse_type()?;
                    self.expect_punct('>')?;
                    Ok(WitType::Option(Box::new(inner)))
                }
                "stream" => {
                    // stream / stream<T>
                    if self.eat_punct('<') {
                        let inner = self.parse_type()?;
                        self.expect_punct('>')?;
                        Ok(WitType::Stream(Some(Box::new(inner))))
                    } else {
                        Ok(WitType::Stream(None))
                    }
                }
                "future" => {
                    if self.eat_punct('<') {
                        let inner = self.parse_type()?;
                        self.expect_punct('>')?;
                        Ok(WitType::Future(Some(Box::new(inner))))
                    } else {
                        Ok(WitType::Future(None))
                    }
                }
                "result" => {
                    // result / result<ok> / result<_, err> / result<ok, err>
                    if self.eat_punct('<') {
                        let first = if self.eat_punct('_') {
                            None
                        } else {
                            Some(self.parse_type()?)
                        };
                        let second = if self.eat_punct(',') {
                            Some(self.parse_type()?)
                        } else {
                            None
                        };
                        self.expect_punct('>')?;
                        Ok(WitType::Result(Box::new(first), Box::new(second)))
                    } else {
                        Ok(WitType::Result(Box::new(None), Box::new(None)))
                    }
                }
                "tuple" => {
                    self.expect_punct('<')?;
                    let mut items = Vec::new();
                    loop {
                        items.push(self.parse_type()?);
                        if !self.eat_punct(',') {
                            break;
                        }
                    }
                    self.expect_punct('>')?;
                    Ok(WitType::Tuple(items))
                }
                "own" => {
                    self.expect_punct('<')?;
                    let name = self.expect_ident()?;
                    self.expect_punct('>')?;
                    Ok(WitType::Own(name))
                }
                "borrow" => {
                    self.expect_punct('<')?;
                    let name = self.expect_ident()?;
                    self.expect_punct('>')?;
                    Ok(WitType::Borrow(name))
                }
                other => Ok(WitType::Named(other.to_string())),
            },
            t => Err(ParseError {
                msg: format!("expected type, found {t:?}"),
            }),
        }
    }

    fn parse_func(&mut self, name: String) -> Result<FuncDecl, ParseError> {
        // we are after `name:`
        self.eat_keyword("static");
        self.expect_keyword("func")?;
        self.expect_punct('(')?;
        let mut params = Vec::new();
        loop {
            if self.eat_punct(')') {
                break;
            }
            let pname = self.expect_ident()?;
            self.expect_punct(':')?;
            let ty = self.parse_type()?;
            params.push((pname, ty));
            if !self.eat_punct(',') {
                self.expect_punct(')')?;
                break;
            }
        }
        let result = if self.eat_punct('→') {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect_punct(';')?;
        Ok(FuncDecl {
            name,
            params,
            result,
        })
    }

    fn expect_keyword(&mut self, kw: &str) -> Result<(), ParseError> {
        if self.eat_keyword(kw) {
            Ok(())
        } else {
            Err(ParseError {
                msg: format!("expected `{kw}`"),
            })
        }
    }

    fn parse_type_decl(&mut self, kind: String) -> Result<TypeDecl, ParseError> {
        // called after the caller consumed the kind keyword (`record`,
        // `variant`, `enum`, `flags`, `resource`); reads the type name.
        let name = self.expect_ident()?;
        match kind.as_str() {
            "record" => {
                self.expect_punct('{')?;
                let mut fields = Vec::new();
                loop {
                    if self.eat_punct('}') {
                        break;
                    }
                    let fname = self.expect_ident()?;
                    self.expect_punct(':')?;
                    let ty = self.parse_type()?;
                    fields.push((fname, ty));
                    if !self.eat_punct(',') {
                        self.expect_punct('}')?;
                        break;
                    }
                }
                Ok(TypeDecl::Record { name, fields })
            }
            "variant" => {
                self.expect_punct('{')?;
                let mut cases = Vec::new();
                loop {
                    if self.eat_punct('}') {
                        break;
                    }
                    let cname = self.expect_ident()?;
                    let cty = if self.eat_punct('(') {
                        let t = self.parse_type()?;
                        self.expect_punct(')')?;
                        Some(t)
                    } else {
                        None
                    };
                    cases.push((cname, cty));
                    if !self.eat_punct(',') {
                        self.expect_punct('}')?;
                        break;
                    }
                }
                Ok(TypeDecl::Variant { name, cases })
            }
            "enum" => {
                self.expect_punct('{')?;
                let mut cases = Vec::new();
                loop {
                    if self.eat_punct('}') {
                        break;
                    }
                    cases.push(self.expect_ident()?);
                    if !self.eat_punct(',') {
                        self.expect_punct('}')?;
                        break;
                    }
                }
                Ok(TypeDecl::Enum { name, cases })
            }
            "flags" => {
                self.expect_punct('{')?;
                let mut cases = Vec::new();
                loop {
                    if self.eat_punct('}') {
                        break;
                    }
                    cases.push(self.expect_ident()?);
                    if !self.eat_punct(',') {
                        self.expect_punct('}')?;
                        break;
                    }
                }
                Ok(TypeDecl::Flags { name, cases })
            }
            "resource" => {
                // resource X { ... } — methods are parsed as funcs and
                // filtered by name shape elsewhere; skip the body
                if self.eat_punct('{') {
                    let mut depth = 1;
                    while depth > 0 {
                        match self.bump() {
                            Token::Punct('{') => depth += 1,
                            Token::Punct('}') => depth -= 1,
                            Token::Eof => break,
                            _ => {}
                        }
                    }
                } else {
                    self.expect_punct(';')?;
                }
                Ok(TypeDecl::Resource { name })
            }
            _ => {
                // `type X = <kind already consumed as ident>`? no: kind was
                // the alias target if the syntax was `type X = T;`
                Err(ParseError {
                    msg: format!("unsupported type declaration `{kind}`"),
                })
            }
        }
    }

    fn parse_alias(&mut self, name: String) -> Result<TypeDecl, ParseError> {
        self.expect_punct('=')?;
        let ty = self.parse_type()?;
        self.expect_punct(';')?;
        Ok(TypeDecl::Alias { name, ty })
    }

    fn parse_interface_items(&mut self, items: &mut Vec<Item>) -> Result<(), ParseError> {
        loop {
            match self.peek().clone() {
                Token::Punct('}') => {
                    self.bump();
                    break;
                }
                Token::Eof => break,
                Token::Ident(word) => match word.as_str() {
                    "record" | "variant" | "enum" | "flags" | "resource" => {
                        let kind = word.clone();
                        self.bump();
                        items.push(Item::Type(self.parse_type_decl(kind)?));
                    }
                    "type" => {
                        self.bump();
                        let name = self.expect_ident()?;
                        items.push(Item::Type(self.parse_alias(name)?));
                    }
                    "use" => {
                        self.bump();
                        let mut from = self.expect_ident()?;
                        // possibly `ns:pkg/iface`
                        if self.eat_punct(':') {
                            from.push(':');
                            from.push_str(&self.expect_ident()?);
                            if self.eat_punct('/') {
                                from.push('/');
                                from.push_str(&self.expect_ident()?);
                            }
                        }
                        self.expect_punct('.')?;
                        self.expect_punct('{')?;
                        let mut names = Vec::new();
                        loop {
                            if self.eat_punct('}') {
                                break;
                            }
                            names.push(self.expect_ident()?);
                            if !self.eat_punct(',') {
                                self.expect_punct('}')?;
                                break;
                            }
                        }
                        self.expect_punct(';')?;
                        items.push(Item::Use { from, names });
                    }
                    _ => {
                        // `name: func(...) -> ...;`
                        let name = self.expect_ident()?;
                        self.expect_punct(':')?;
                        let f = self.parse_func(name)?;
                        items.push(Item::Func(f));
                    }
                },
                other => {
                    return Err(ParseError {
                        msg: format!("unexpected token in interface: {other:?}"),
                    })
                }
            }
        }
        Ok(())
    }

    fn parse_world_items(&mut self, items: &mut Vec<WorldItem>) -> Result<(), ParseError> {
        loop {
            match self.peek().clone() {
                Token::Punct('}') => {
                    self.bump();
                    break;
                }
                Token::Eof => break,
                Token::Ident(word) => match word.as_str() {
                    "import" | "export" => {
                        let is_import = word == "import";
                        self.bump();
                        let next = self.peek().clone();
                        match next {
                            Token::Ident(name) if name.contains(':') => {
                                // interface reference: ns:pkg/iface@ver
                                self.bump();
                                let mut full = name;
                                if self.eat_punct('/') {
                                    full.push('/');
                                    full.push_str(&self.expect_ident()?);
                                }
                                if self.eat_punct('@') {
                                    full.push('@');
                                    full.push_str(&self.expect_ident()?);
                                }
                                self.expect_punct(';')?;
                                if is_import {
                                    items.push(WorldItem::ImportInterface(full));
                                } else {
                                    items.push(WorldItem::ExportInterface(full));
                                }
                            }
                            Token::Ident(name) => {
                                // either `include`d interface by local name,
                                // or a root func: `name: func(...)`
                                self.bump();
                                if self.peek() == &Token::Punct(';') {
                                    self.bump();
                                    if is_import {
                                        items.push(WorldItem::ImportInterface(name));
                                    } else {
                                        items.push(WorldItem::ExportInterface(name));
                                    }
                                } else {
                                    self.expect_punct(':')?;
                                    let f = self.parse_func(name)?;
                                    if is_import {
                                        items.push(WorldItem::ImportFunc(f));
                                    } else {
                                        items.push(WorldItem::ExportFunc(f));
                                    }
                                }
                            }
                            other => {
                                return Err(ParseError {
                                    msg: format!("bad world item: {other:?}"),
                                })
                            }
                        }
                    }
                    "record" | "variant" | "enum" | "flags" | "resource" => {
                        let kind = word.clone();
                        self.bump();
                        items.push(WorldItem::Type(self.parse_type_decl(kind)?));
                    }
                    "type" => {
                        self.bump();
                        let name = self.expect_ident()?;
                        items.push(WorldItem::Type(self.parse_alias(name)?));
                    }
                    "use" => {
                        self.bump();
                        let from = self.expect_ident()?;
                        self.expect_punct('.')?;
                        self.expect_punct('{')?;
                        let mut names = Vec::new();
                        loop {
                            if self.eat_punct('}') {
                                break;
                            }
                            names.push(self.expect_ident()?);
                            if !self.eat_punct(',') {
                                self.expect_punct('}')?;
                                break;
                            }
                        }
                        self.expect_punct(';')?;
                        items.push(WorldItem::Use { from, names });
                    }
                    other => {
                        return Err(ParseError {
                            msg: format!("unexpected token in world: {other:?}"),
                        })
                    }
                },
                other => {
                    return Err(ParseError {
                        msg: format!("unexpected token in world: {other:?}"),
                    })
                }
            }
        }
        Ok(())
    }
}

/// Parse a full WIT source file into a [`Package`].
pub fn parse_wit(src: &str) -> Result<Package, ParseError> {
    let tokens = Lexer::new(src).tokenize();
    let mut p = Parser::new(tokens);
    let mut pkg = Package::default();

    loop {
        match p.peek().clone() {
            Token::Eof => break,
            Token::Ident(w) if w == "package" => {
                p.bump();
                let mut name = p.expect_ident()?;
                if p.eat_punct(':') {
                    name.push(':');
                    name.push_str(&p.expect_ident()?);
                    if p.eat_punct('/') {
                        name.push('/');
                        name.push_str(&p.expect_ident()?);
                    }
                }
                if p.eat_punct('@') {
                    name.push('@');
                    name.push_str(&p.expect_ident()?);
                }
                p.expect_punct(';')?;
                pkg.name = Some(name);
            }
            Token::Ident(w) if w == "interface" => {
                p.bump();
                let name = p.expect_ident()?;
                p.expect_punct('{')?;
                let mut items = Vec::new();
                p.parse_interface_items(&mut items)?;
                pkg.interfaces.push(Interface { name, items });
            }
            Token::Ident(w) if w == "world" => {
                p.bump();
                let name = p.expect_ident()?;
                p.expect_punct('{')?;
                let mut items = Vec::new();
                p.parse_world_items(&mut items)?;
                pkg.worlds.push(World { name, items });
            }
            Token::Ident(w) if w == "include" => {
                // `include other-world;` — ignore for binding purposes
                p.bump();
                while !matches!(p.bump(), Token::Punct(';') | Token::Eof) {}
            }
            other => {
                return Err(ParseError {
                    msg: format!("unexpected top-level token: {other:?}"),
                })
            }
        }
    }
    Ok(pkg)
}
