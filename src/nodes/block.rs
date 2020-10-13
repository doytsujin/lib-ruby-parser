use crate::source::Range;
use crate::Node;

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub call: Box<Node>,
    pub args: Option<Box<Node>>,
    pub body: Option<Box<Node>>,

    pub begin_l: Range,
    pub end_l: Range,
    pub expression_l: Range,
}