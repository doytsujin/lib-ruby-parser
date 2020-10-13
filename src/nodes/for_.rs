use crate::source::Range;
use crate::Node;

#[derive(Debug, Clone, PartialEq)]
pub struct For {
    pub iterator: Box<Node>,
    pub iteratee: Box<Node>,
    pub body: Option<Box<Node>>,

    pub keyword_l: Range,
    pub in_l: Range,
    pub begin_l: Range,
    pub end_l: Range,
    pub expression_l: Range,
}