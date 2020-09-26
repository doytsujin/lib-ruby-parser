use crate::source::Range;
use crate::source::map::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Begin { statements: Vec<Node>, loc: CollectionMap },
    Int { value: String, loc: OperatorMap },
    Send { receiver: Option<Box<Node>>, operator: String, args: Vec<Node>, loc: SendMap },
    CSend { receiver: Option<Box<Node>>, operator: String, args: Vec<Node>, loc: SendMap },
    Nil { loc: Map },
    True { loc: Map },
    False { loc: Map },
    Self_ { loc: Map },
    __FILE__ { loc: Map },
    __LINE__ { loc: Map },
    __ENCODING__ { loc: Map },
    Preexe { body: Option<Box<Node>>, loc: KeywordMap },
    Lvar { name: String, loc: VariableMap },
    Rescue { body: Option<Box<Node>>, rescue_bodies: Vec<Node>, else_: Option<Box<Node>>, loc: ConditionMap },
    Ensure { body: Option<Box<Node>>, ensure: Box<Node>, loc: ConditionMap },
    KwBegin { statements: Vec<Node>, loc: CollectionMap },
    Args { args: Vec<Node>, loc: CollectionMap },
    Def { name: String, args: Option<Box<Node>>, body: Option<Box<Node>>, loc: MethodDefinitionMap },
    Arg { name: String, loc: VariableMap },
    Sym { name: String, loc: CollectionMap },
    Alias { to: Box<Node>, from: Box<Node>, loc: KeywordMap },
    Ivar { name: String, loc: VariableMap },
    Gvar { name: String, loc: VariableMap },
    Cvar { name: String, loc: VariableMap },
    BackRef { name: String, loc: VariableMap },
    NthRef { name: String, loc: VariableMap },
    Lvasgn { name: String, rhs: Box<Node>, loc: VariableMap },
    Cvasgn { name: String, rhs: Box<Node>, loc: VariableMap },
    Ivasgn { name: String, rhs: Box<Node>, loc: VariableMap },
    Gvasgn { name: String, rhs: Box<Node>, loc: VariableMap },
    Const { scope: Option<Box<Node>>, name: String, loc: ConstantMap },
    Casgn { name: String, rhs: Box<Node>, loc: VariableMap },
    IndexAsgn { receiver: Box<Node>, indexes: Vec<Node>, rhs: Box<Node>, loc: IndexMap },
    Undef { names: Vec<Node>, loc: KeywordMap },
}

impl Node {
    pub fn expression(&self) -> &Range {
        match self {
            Self::Begin { loc, .. } => &loc.expression,
            Self::Int { loc, .. } => &loc.expression,
            Self::Send { loc, .. } => &loc.expression,
            Self::CSend { loc, .. } => &loc.expression,
            Self::Nil { loc, .. } => &loc.expression,
            Self::True { loc, .. } => &loc.expression,
            Self::False { loc, .. } => &loc.expression,
            Self::Self_ { loc, .. } => &loc.expression,
            Self::__FILE__ { loc, .. } => &loc.expression,
            Self::__LINE__ { loc, .. } => &loc.expression,
            Self::__ENCODING__ { loc, .. } => &loc.expression,
            Self::Preexe { loc, .. } => &loc.expression,
            Self::Lvar { loc, .. } => &loc.expression,
            Self::Rescue { loc, .. } => &loc.expression,
            Self::Ensure { loc, .. } => &loc.expression,
            Self::KwBegin { loc, .. } => &loc.expression,
            Self::Args { loc, .. } => &loc.expression,
            Self::Def { loc, .. } => &loc.expression,
            Self::Arg { loc, .. } => &loc.expression,
            Self::Sym { loc, .. } => &loc.expression,
            Self::Alias { loc, .. } => &loc.expression,
            Self::Ivar { loc, .. } => &loc.expression,
            Self::Gvar { loc, .. } => &loc.expression,
            Self::Cvar { loc, .. } => &loc.expression,
            Self::BackRef { loc, .. } => &loc.expression,
            Self::NthRef { loc, .. } => &loc.expression,
            Self::Lvasgn { loc, .. } => &loc.expression,
            Self::Cvasgn { loc, .. } => &loc.expression,
            Self::Ivasgn { loc, .. } => &loc.expression,
            Self::Gvasgn { loc, .. } => &loc.expression,
            Self::Const { loc, .. } => &loc.expression,
            Self::Casgn { loc, .. } => &loc.expression,
            Self::IndexAsgn { loc, .. } => &loc.expression,
            Self::Undef { loc, .. } => &loc.expression,
        }
    }
}
