/*!

The "parse-tree" is what is produced by the parser. We use it do
some pre-expansion and so forth before creating the proper AST.

Here is an example file to give you the idea:

```
grammar Type<'input, T> {

  // External token type; "xxx" is assumed to map
  // to a variant name, but we can do some substitutions
  // for you. Input will be an `Iterator<Item=T>`.
  //
  // Eventually this should become optional, but not
  // for the first version because I am lazy.
  token parser::Token<T> where {
    "(" => LParen;
    ")" => RParen;
  };

  // Declare an "aliasing" nonterminal.
  Expr = Alt;

  // ...which can optionally map.
  Expr = Alt => code;

  // Declare a "match" nonterminal.
  Expr: Type = {
    "class" "Id" "{" Foo+ Foo* => {
        // action code
    }
    "foo" "bar" if $X ~ "[COMMA]" => {
    }
  };

  // Macro nonterminals. Macro arguments may be either any
  // symbol expressions and may be used in types, definitions,
  // or guard expressions.

  // Example 1: comma-separated list with optional trailing comma.
  Comma<E>: Vec<E> = {
      ~v:(~E ",")* ~e:E? => {
          let mut v = v;
          if let Some(e) = e { v.push(e); }
          v
      };
  };

  // Example 2: conditional patterns
  Expr<M>: Expr = {
      ~Expr "(" ~Comma<Expr> ")" => Expr::CallExpr(~~);

      ID if M !~ "NO_ID" => {
      };
  };
}
```

*/

use intern::InternedString;
use std::fmt::{Display, Formatter, Error};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grammar {
    pub type_name: TypeRef,
    pub items: Vec<GrammarItem>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Span(pub usize, pub usize);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrammarItem {
    TokenType(TokenTypeData),
    Nonterminal(NonterminalData),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenTypeData {
    pub type_name: TypeRef,
    pub conversions: Vec<(InternedString, InternedString)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeRef {
    // (T1, T2)
    Tuple(Vec<TypeRef>),

    // Foo<'a, 'b, T1, T2>, Foo::Bar, etc
    Nominal {
        path: Vec<InternedString>,
        types: Vec<TypeRef>
    },

    // 'x ==> only should appear within nominal types, but what do we care
    Lifetime(InternedString),

    // Foo or Bar ==> treated specially since macros may care
    Id(InternedString),

    // <N> ==> type of a nonterminal, emitted by macro expansion
    OfSymbol(Symbol),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NonterminalData {
    pub name: InternedString,
    pub args: Vec<InternedString>, // macro arguments
    pub type_decl: Option<TypeRef>,
    pub alternatives: Vec<Alternative>
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alternative {
    pub span: Span,

    pub expr: ExprSymbol,

    // if C, only legal in macros
    pub condition: Option<Condition>,

    // => { code }
    pub action: Option<Action>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    // code provided by the user
    User(String),

    // an index into a side-list of action fns, which is setup to take
    // all of the values in this alternative as arguments, dropping
    // the ones it doesn't care about.
    Fn(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Condition {
    pub span: Span,
    pub lhs: InternedString, // X
    pub rhs: InternedString, // "Foo"
    pub op: ConditionOp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConditionOp {
    // X == "Foo", equality
    Equals,

    // X != "Foo", inequality
    NotEquals,

    // X ~~ "Foo", regexp match
    Match,

    // X !~ "Foo", regexp non-match
    NotMatch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Symbol {
    // (X Y)
    Expr(ExprSymbol),

    // "foo"
    Terminal(InternedString),

    // foo
    Nonterminal(InternedString),

    // foo<..>
    Macro(MacroSymbol),

    // X+, X?, X*
    Repeat(Box<RepeatSymbol>),

    // ~X
    Choose(Box<Symbol>),

    // ~x:X
    Name(InternedString, Box<Symbol>),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RepeatOp {
    Star, Plus, Question
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepeatSymbol {
    pub op: RepeatOp,
    pub symbol: Symbol
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExprSymbol {
    pub span: Span,
    pub symbols: Vec<Symbol>
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacroSymbol {
    pub name: InternedString,
    pub args: Vec<Symbol>,
    pub span: Span,
}

impl GrammarItem {
    pub fn is_macro_def(&self) -> bool {
        match *self {
            GrammarItem::Nonterminal(ref d) => d.is_macro_def(),
            _ => false,
        }
    }
}

impl NonterminalData {
    pub fn is_macro_def(&self) -> bool {
        !self.args.is_empty()
    }
}

impl Symbol {
    pub fn canonical_form(&self) -> String {
        format!("{}", self)
    }
}

impl Display for Symbol {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            Symbol::Expr(ref expr) =>
                write!(fmt, "{}", expr),
            Symbol::Terminal(ref s) =>
                write!(fmt, "\"{}\"", s.to_string()),
            Symbol::Nonterminal(ref s) =>
                write!(fmt, "{}", s),
            Symbol::Macro(ref m) =>
                write!(fmt, "{}", m),
            Symbol::Repeat(ref r) =>
                write!(fmt, "{}", r),
            Symbol::Choose(ref s) =>
                write!(fmt, "~{}", s),
            Symbol::Name(n, ref s) =>
                write!(fmt, "~{}:{}", n, s),
        }
    }
}

impl Display for RepeatSymbol {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "{}{}", self.symbol, self.op)
    }
}

impl Display for RepeatOp {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            RepeatOp::Plus => write!(fmt, "+"),
            RepeatOp::Star => write!(fmt, "*"),
            RepeatOp::Question => write!(fmt, "?"),
        }
    }
}

impl Display for ExprSymbol {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "({})", Sep(" ", &self.symbols))
    }
}

impl ExprSymbol {
    pub fn canonical_form(&self) -> String {
        format!("{}", self)
    }
}

impl MacroSymbol {
    pub fn canonical_form(&self) -> String {
        format!("{}", self)
    }
}

impl Display for MacroSymbol {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "{}<{}>", self.name, Sep(", ", &self.args))
    }
}

struct Sep<S>(&'static str, S);

impl<'a,S:Display> Display for Sep<&'a Vec<S>> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        let &Sep(sep, vec) = self;
        let mut elems = vec.iter();
        if let Some(elem) = elems.next() {
            write!(fmt, "{}", elem);
            while let Some(elem) = elems.next() {
                write!(fmt, "{}{}", sep, elem);
            }
        }
        Ok(())
    }
}

impl Display for TypeRef {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            TypeRef::Tuple(ref types) =>
                write!(fmt, "({})", Sep(", ", types)),
            TypeRef::Nominal { ref path, ref types } if types.len() == 0 =>
                write!(fmt, "{}", Sep("::", path)),
            TypeRef::Nominal { ref path, ref types } =>
                write!(fmt, "{}<{}>", Sep("::", path), Sep(", ", types)),
            TypeRef::Lifetime(ref s) =>
                write!(fmt, "{}", s),
            TypeRef::Id(ref s) =>
                write!(fmt, "{}", s),
            TypeRef::OfSymbol(ref s) =>
                write!(fmt, "`{}`", s),
        }
    }
}