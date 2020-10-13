use crate::source::Range;
use crate::Node;

#[derive(Debug, Clone, PartialEq)]
pub struct If {
    pub cond: Box<Node>,
    pub if_true: Option<Box<Node>>,
    pub if_false: Option<Box<Node>>,

    pub if_l: Range,
    pub else_l: Option<Range>,
    pub end_l: Range,
    pub expression_l: Range,
}