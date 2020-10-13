use crate::source::Range;
use crate::Node;

#[derive(Debug, Clone, PartialEq)]
pub struct And {
    pub lhs: Box<Node>,
    pub rhs: Box<Node>,

    pub operator_l: Range,
    pub expression_l: Range,
}