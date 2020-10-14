use crate::nodes::InnerNode;
use crate::source::Range;

#[derive(Debug, Clone, PartialEq)]
pub struct ZSuper {
    pub expression_l: Range,
}

impl InnerNode for ZSuper {
    fn expression(&self) -> &Range {
        &self.expression_l
    }

    fn inspected_children(&self, _indent: usize) -> Vec<String> {
        vec![]
    }

    fn str_type(&self) -> &'static str {
        "zsuper"
    }
}
