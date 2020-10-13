use crate::source::Range;
use crate::Node;

#[derive(Debug, Clone, PartialEq)]
pub struct CSend {
    pub receiver: Box<Node>,
    pub method_name: String,
    pub args: Vec<Node>,

    pub dot_l: Range,
    pub selector_l: Range,
    pub expression_l: Range,
}