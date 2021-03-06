#[cfg(feature = "onig")]
use onig::{Regex, RegexOptions};
use std::collections::HashMap;
use std::convert::TryInto;

use crate::error::Diagnostics;
use crate::nodes::*;
use crate::source::Range;
use crate::StringValue;
use crate::{
    Context, CurrentArgStack, Lexer, Loc, MaxNumparamStack, Node, StaticEnvironment, Token,
    VariablesStack,
};
use crate::{Diagnostic, DiagnosticMessage, ErrorLevel};

#[derive(Debug, PartialEq)]
pub(crate) enum LoopType {
    While,
    Until,
}

#[derive(Debug, PartialEq)]
pub(crate) enum KeywordCmd {
    Break,
    Defined,
    Next,
    Redo,
    Retry,
    Return,
    Super,
    Yield,
    Zsuper,
}

enum MethodCallType {
    Send,
    CSend,
}

#[derive(Debug, PartialEq)]
pub(crate) enum LogicalOp {
    And,
    Or,
}

#[derive(Debug, Clone)]
pub(crate) enum PKwLabel {
    PlainLabel(Token),
    QuotedLabel((Token, Vec<Node>, Token)),
}

#[derive(Debug, Clone)]
pub(crate) enum ArgsType {
    Args(Option<Node>),
    Numargs(u8),
}

#[derive(Debug)]
pub(crate) struct Builder {
    static_env: StaticEnvironment,
    context: Context,
    current_arg_stack: CurrentArgStack,
    max_numparam_stack: MaxNumparamStack,
    pattern_variables: VariablesStack,
    pattern_hash_keys: VariablesStack,
    diagnostics: Diagnostics,
}

impl Builder {
    pub(crate) fn new(
        static_env: StaticEnvironment,
        context: Context,
        current_arg_stack: CurrentArgStack,
        max_numparam_stack: MaxNumparamStack,
        pattern_variables: VariablesStack,
        pattern_hash_keys: VariablesStack,
        diagnostics: Diagnostics,
    ) -> Self {
        Self {
            static_env,
            context,
            current_arg_stack,
            max_numparam_stack,
            pattern_variables,
            pattern_hash_keys,
            diagnostics,
        }
    }

    //
    // Literals
    //

    // Singletons

    pub(crate) fn nil(&self, nil_t: Token) -> Node {
        Node::Nil(Box::new(Nil {
            expression_l: self.loc(&nil_t),
        }))
    }

    pub(crate) fn true_(&self, true_t: Token) -> Node {
        Node::True(Box::new(True {
            expression_l: self.loc(&true_t),
        }))
    }

    pub(crate) fn false_(&self, false_t: Token) -> Node {
        Node::False(Box::new(False {
            expression_l: self.loc(&false_t),
        }))
    }

    // Numerics

    pub(crate) fn integer(&self, integer_t: Token) -> Node {
        let expression_l = self.loc(&integer_t);
        Node::Int(Box::new(Int {
            value: value(integer_t),
            expression_l,
            operator_l: None,
        }))
    }

    pub(crate) fn float(&self, float_t: Token) -> Node {
        let expression_l = self.loc(&float_t);
        Node::Float(Box::new(Float {
            value: value(float_t),
            expression_l,
            operator_l: None,
        }))
    }

    pub(crate) fn rational(&self, rational_t: Token) -> Node {
        let expression_l = self.loc(&rational_t);
        Node::Rational(Box::new(Rational {
            value: value(rational_t),
            expression_l,
            operator_l: None,
        }))
    }

    pub(crate) fn complex(&self, complex_t: Token) -> Node {
        let expression_l = self.loc(&complex_t);
        Node::Complex(Box::new(Complex {
            value: value(complex_t),
            expression_l,
            operator_l: None,
        }))
    }

    pub(crate) fn unary_num(&self, unary_t: Token, mut numeric: Node) -> Node {
        let new_operator_l = self.loc(&unary_t);
        let sign = value(unary_t);

        match &mut numeric {
            Node::Int(inner) => {
                inner.value = sign + &inner.value;
                inner.expression_l = new_operator_l.join(&inner.expression_l);
                inner.operator_l = Some(new_operator_l);
            }
            Node::Float(inner) => {
                inner.value = sign + &inner.value;
                inner.expression_l = new_operator_l.join(&inner.expression_l);
                inner.operator_l = Some(new_operator_l);
            }
            Node::Rational(inner) => {
                inner.value = sign + &inner.value;
                inner.expression_l = new_operator_l.join(&inner.expression_l);
                inner.operator_l = Some(new_operator_l);
            }
            Node::Complex(inner) => {
                inner.value = sign + &inner.value;
                inner.expression_l = new_operator_l.join(&inner.expression_l);
                inner.operator_l = Some(new_operator_l);
            }
            _ => unreachable!(),
        }

        numeric
    }

    pub(crate) fn __line__(&self, line_t: Token) -> Node {
        Node::Line(Box::new(Line {
            expression_l: self.loc(&line_t),
        }))
    }

    // Strings

    pub(crate) fn str_node(
        &self,
        begin_t: Option<Token>,
        value: StringValue,
        parts: Vec<Node>,
        end_t: Option<Token>,
    ) -> Node {
        match self.string_map(&begin_t, &parts, &end_t) {
            StringMap::CollectionMap((begin_l, end_l, expression_l)) => Node::Str(Box::new(Str {
                value,
                begin_l,
                end_l,
                expression_l,
            })),
            StringMap::HeredocMap((heredoc_body_l, heredoc_end_l, expression_l)) => {
                Node::Heredoc(Box::new(Heredoc {
                    parts,
                    heredoc_body_l,
                    heredoc_end_l,
                    expression_l,
                }))
            }
        }
    }

    pub(crate) fn string_internal(&self, string_t: Token) -> Node {
        let expression_l = self.loc(&string_t);
        let value = StringValue::new(string_t);
        Node::Str(Box::new(Str {
            value,
            begin_l: None,
            end_l: None,
            expression_l,
        }))
    }

    pub(crate) fn string_compose(
        &self,
        begin_t: Option<Token>,
        parts: Vec<Node>,
        end_t: Option<Token>,
    ) -> Node {
        match &parts[..] {
            [] => return self.str_node(begin_t, StringValue::empty(), parts, end_t),
            [Node::Str(_)] | [Node::Dstr(_)] | [Node::Heredoc(_)]
                if begin_t.is_none() && end_t.is_none() =>
            {
                return first(parts);
            }
            [Node::Str(inner)] => {
                let value = inner.value.clone();
                return self.str_node(begin_t, value, parts, end_t);
            }
            [Node::Dstr(_)] | [Node::Heredoc(_)] => unreachable!(),
            _ => {}
        };

        match self.string_map(&begin_t, &parts, &end_t) {
            StringMap::CollectionMap((begin_l, end_l, expression_l)) => {
                Node::Dstr(Box::new(Dstr {
                    parts,
                    begin_l,
                    end_l,
                    expression_l,
                }))
            }
            StringMap::HeredocMap((heredoc_body_l, heredoc_end_l, expression_l)) => {
                Node::Heredoc(Box::new(Heredoc {
                    parts,
                    heredoc_body_l,
                    heredoc_end_l,
                    expression_l,
                }))
            }
        }
    }

    pub(crate) fn character(&self, char_t: Token) -> Node {
        let str_range = self.loc(&char_t);

        let begin_l = Some(str_range.with_end(str_range.begin_pos + 1));
        let end_l = None;
        let expression_l = str_range;

        let value = StringValue::new(char_t);
        Node::Str(Box::new(Str {
            value,
            begin_l,
            end_l,
            expression_l,
        }))
    }

    pub(crate) fn __file__(&self, file_t: Token) -> Node {
        Node::File(Box::new(File {
            expression_l: self.loc(&file_t),
        }))
    }

    // Symbols

    fn validate_sym_value(&self, value: &StringValue, loc: &Range) {
        if !value.valid {
            self.error(
                DiagnosticMessage::InvalidSymbol("UTF-8".to_owned()),
                loc.clone(),
            )
        }
    }

    pub(crate) fn symbol(&self, start_t: Token, value_t: Token) -> Node {
        let expression_l = self.loc(&start_t).join(&self.loc(&value_t));
        let begin_l = Some(self.loc(&start_t));
        let value = StringValue::new(value_t);
        self.validate_sym_value(&value, &expression_l);
        Node::Sym(Box::new(Sym {
            name: value,
            begin_l,
            end_l: None,
            expression_l,
        }))
    }

    pub(crate) fn symbol_internal(&self, symbol_t: Token) -> Node {
        let expression_l = self.loc(&symbol_t);
        let value = StringValue::new(symbol_t);
        self.validate_sym_value(&value, &expression_l);
        Node::Sym(Box::new(Sym {
            name: value,
            begin_l: None,
            end_l: None,
            expression_l,
        }))
    }

    pub(crate) fn symbol_compose(&self, begin_t: Token, parts: Vec<Node>, end_t: Token) -> Node {
        if let [Node::Str(inner)] = &parts[..] {
            let value = &inner.value;
            let (begin_l, end_l, expression_l) =
                self.collection_map(&Some(begin_t), &[], &Some(end_t));

            self.validate_sym_value(value, &expression_l);

            return Node::Sym(Box::new(Sym {
                name: value.clone(),
                begin_l,
                end_l,
                expression_l,
            }));
        }

        let (begin_l, end_l, expression_l) =
            self.collection_map(&Some(begin_t), &parts, &Some(end_t));
        Node::Dsym(Box::new(Dsym {
            parts,
            begin_l,
            end_l,
            expression_l,
        }))
    }

    // Executable strings

    pub(crate) fn xstring_compose(&self, begin_t: Token, parts: Vec<Node>, end_t: Token) -> Node {
        let begin_l = self.loc(&begin_t);
        if value(begin_t).starts_with("<<") {
            let heredoc_body_l = collection_expr(&parts).unwrap_or_else(|| self.loc(&end_t));
            let heredoc_end_l = self.loc(&end_t);
            let expression_l = begin_l;

            Node::XHeredoc(Box::new(XHeredoc {
                parts,
                heredoc_body_l,
                heredoc_end_l,
                expression_l,
            }))
        } else {
            let end_l = self.loc(&end_t);
            let expression_l = begin_l.join(&end_l);

            Node::Xstr(Box::new(Xstr {
                parts,
                begin_l,
                end_l,
                expression_l,
            }))
        }
    }

    // Indented (interpolated, noninterpolated, executable) strings

    pub(crate) fn heredoc_dedent(&self, node: &mut Node, dedent_level: i32) {
        if dedent_level == 0 {
            return;
        }

        let dedent_level: usize = dedent_level
            .try_into()
            .expect("dedent_level must be positive");

        let dedent_heredoc_parts = |parts: &mut Vec<Node>| {
            for part in parts.iter_mut() {
                match part {
                    Node::Str(inner) => {
                        Self::dedent_string(&mut inner.as_mut().value, dedent_level)
                    }
                    Node::Begin(_) => {}
                    _ => unreachable!("unsupported heredoc child {}", part.str_type()),
                }
            }
        };

        match node {
            Node::Heredoc(heredoc) => {
                dedent_heredoc_parts(&mut heredoc.parts);
            }
            Node::XHeredoc(heredoc) => {
                dedent_heredoc_parts(&mut heredoc.parts);
            }
            other => unreachable!("unsupported heredoc_dedent argument {}", other.str_type()),
        }
    }

    const TAB_WIDTH: usize = 8;

    pub(crate) fn dedent_string(s: &mut StringValue, width: usize) {
        let mut col: usize = 0;
        let mut i: usize = 0;

        loop {
            if !(i < s.bytes.len() && col < width) {
                break;
            }

            if s.bytes[i] == b' ' {
                col += 1;
            } else if s.bytes[i] == b'\t' {
                let n = Self::TAB_WIDTH * (col / Self::TAB_WIDTH + 1);
                if n > Self::TAB_WIDTH {
                    break;
                }
                col = n;
            } else {
                break;
            }

            i += 1;
        }

        s.bytes = s.bytes[i..].to_owned()
    }

    // Regular expressions

    pub(crate) fn regexp_options(&self, regexp_end_t: Token) -> Option<Node> {
        if regexp_end_t.loc.end - regexp_end_t.loc.begin == 1 {
            // no regexp options, only trailing "/"
            return None;
        }
        let expression_l = self.loc(&regexp_end_t).adjust_begin(1);
        let mut options = value(regexp_end_t)[1..].chars().collect::<Vec<_>>();
        options.sort_unstable();
        options.dedup();

        Some(Node::RegOpt(Box::new(RegOpt {
            options,
            expression_l,
        })))
    }

    pub(crate) fn regexp_compose(
        &self,
        begin_t: Token,
        parts: Vec<Node>,
        end_t: Token,
        options: Option<Node>,
    ) -> Node {
        let begin_l = self.loc(&begin_t);
        let end_l = self.loc(&end_t).resize(1);
        let expression_l =
            begin_l.join(&maybe_node_expr(&options.as_ref()).unwrap_or_else(|| self.loc(&end_t)));
        match &options {
            Some(Node::RegOpt(inner)) => {
                self.validate_static_regexp(&parts, &inner.options, &expression_l)
            }
            None => self.validate_static_regexp(&parts, &[], &expression_l),
            _ => unreachable!("must be Option<RegOpt>"),
        };
        Node::Regexp(Box::new(Regexp {
            parts,
            options,
            begin_l,
            end_l,
            expression_l,
        }))
    }

    // Arrays

    pub(crate) fn array(
        &self,
        begin_t: Option<Token>,
        elements: Vec<Node>,
        end_t: Option<Token>,
    ) -> Node {
        let (begin_l, end_l, expression_l) = self.collection_map(&begin_t, &elements, &end_t);
        Node::Array(Box::new(Array {
            elements,
            begin_l,
            end_l,
            expression_l,
        }))
    }

    pub(crate) fn splat(&self, star_t: Token, value: Option<Node>) -> Node {
        let operator_l = self.loc(&star_t);
        let expression_l = operator_l
            .clone()
            .maybe_join(&maybe_node_expr(&value.as_ref()));

        Node::Splat(Box::new(Splat {
            operator_l,
            expression_l,
            value,
        }))
    }

    pub(crate) fn word(&self, parts: Vec<Node>) -> Node {
        match &parts[..] {
            [Node::Str(_)] | [Node::Dstr(_)] => {
                // collapse_string_parts? == true
                return parts
                    .into_iter()
                    .next()
                    .expect("parts is supposed to have exactly 1 element");
            }
            _ => {}
        }

        let (begin_l, end_l, expression_l) = self.collection_map(&None, &parts, &None);
        Node::Dstr(Box::new(Dstr {
            parts,
            begin_l,
            end_l,
            expression_l,
        }))
    }

    pub(crate) fn words_compose(&self, begin_t: Token, elements: Vec<Node>, end_t: Token) -> Node {
        let begin_l = self.loc(&begin_t);
        let end_l = self.loc(&end_t);
        let expression_l = begin_l.join(&end_l);
        Node::Array(Box::new(Array {
            elements,
            begin_l: Some(begin_l),
            end_l: Some(end_l),
            expression_l,
        }))
    }

    pub(crate) fn symbols_compose(&self, begin_t: Token, parts: Vec<Node>, end_t: Token) -> Node {
        let parts = parts
            .into_iter()
            .map(|part| match part {
                Node::Str(inner) => {
                    let Str {
                        value,
                        begin_l,
                        end_l,
                        expression_l,
                    } = *inner;
                    self.validate_sym_value(&value, &expression_l);
                    Node::Sym(Box::new(Sym {
                        name: value,
                        begin_l,
                        end_l,
                        expression_l,
                    }))
                }
                Node::Dstr(inner) => {
                    let Dstr {
                        parts,
                        begin_l,
                        end_l,
                        expression_l,
                    } = *inner;
                    Node::Dsym(Box::new(Dsym {
                        parts,
                        begin_l,
                        end_l,
                        expression_l,
                    }))
                }
                _ => part,
            })
            .collect::<Vec<_>>();

        let begin_l = self.loc(&begin_t);
        let end_l = self.loc(&end_t);
        let expression_l = begin_l.join(&end_l);
        Node::Array(Box::new(Array {
            elements: parts,
            begin_l: Some(begin_l),
            end_l: Some(end_l),
            expression_l,
        }))
    }

    // Hashes

    pub(crate) fn pair(&self, key: Node, assoc_t: Token, value: Node) -> Node {
        let operator_l = self.loc(&assoc_t);
        let expression_l = join_exprs(&key, &value);

        Node::Pair(Box::new(Pair {
            key,
            value,
            operator_l,
            expression_l,
        }))
    }

    pub(crate) fn pair_keyword(&self, key_t: Token, value: Node) -> Node {
        let key_range = self.loc(&key_t);
        let key_l = key_range.adjust_end(-1);
        let colon_l = key_range.with_begin(key_range.end_pos - 1);
        let expression_l = key_range.join(&value.expression());

        let key = StringValue::new(key_t);
        self.validate_sym_value(&key, &key_l);

        let key = Node::Sym(Box::new(Sym {
            name: key,
            begin_l: None,
            end_l: None,
            expression_l: key_l,
        }));
        Node::Pair(Box::new(Pair {
            key,
            value,
            operator_l: colon_l,
            expression_l,
        }))
    }

    pub(crate) fn pair_quoted(
        &self,
        begin_t: Token,
        parts: Vec<Node>,
        end_t: Token,
        value: Node,
    ) -> Node {
        let end_l = self.loc(&end_t);

        let quote_loc = Loc {
            begin: end_l.end_pos - 2,
            end: end_l.end_pos - 1,
        };

        let colon_l = end_l.with_begin(end_l.end_pos - 1);

        let end_t: Token = Token {
            token_type: end_t.token_type,
            token_value: end_t.token_value,
            loc: quote_loc,
        };
        let expression_l = self.loc(&begin_t).join(&value.expression());

        let key = self.symbol_compose(begin_t, parts, end_t);

        Node::Pair(Box::new(Pair {
            key,
            value,
            operator_l: colon_l,
            expression_l,
        }))
    }

    pub(crate) fn kwsplat(&self, dstar_t: Token, arg: Node) -> Node {
        let operator_l = self.loc(&dstar_t);
        let expression_l = arg.expression().join(&operator_l);

        Node::Kwsplat(Box::new(Kwsplat {
            value: arg,
            operator_l,
            expression_l,
        }))
    }

    pub(crate) fn associate(
        &self,
        begin_t: Option<Token>,
        pairs: Vec<Node>,
        end_t: Option<Token>,
    ) -> Node {
        let (begin_l, end_l, expression_l) = self.collection_map(&begin_t, &pairs, &end_t);
        Node::Hash(Box::new(Hash {
            pairs,
            begin_l,
            end_l,
            expression_l,
        }))
    }

    // Ranges

    pub(crate) fn range_inclusive(
        &self,
        left: Option<Node>,
        dot2_t: Token,
        right: Option<Node>,
    ) -> Node {
        let operator_l = self.loc(&dot2_t);
        let expression_l = operator_l
            .clone()
            .maybe_join(&maybe_node_expr(&left.as_ref()))
            .maybe_join(&maybe_node_expr(&right.as_ref()));

        Node::Irange(Box::new(Irange {
            left,
            right,
            operator_l,
            expression_l,
        }))
    }

    pub(crate) fn range_exclusive(
        &self,
        left: Option<Node>,
        dot3_t: Token,
        right: Option<Node>,
    ) -> Node {
        let operator_l = self.loc(&dot3_t);
        let expression_l = operator_l
            .clone()
            .maybe_join(&maybe_node_expr(&left.as_ref()))
            .maybe_join(&maybe_node_expr(&right.as_ref()));

        Node::Erange(Box::new(Erange {
            left,
            right,
            operator_l,
            expression_l,
        }))
    }

    //
    // Access
    //

    pub(crate) fn self_(&self, token: Token) -> Node {
        Node::Self_(Box::new(Self_ {
            expression_l: self.loc(&token),
        }))
    }

    pub(crate) fn lvar(&self, token: Token) -> Node {
        let expression_l = self.loc(&token);
        Node::Lvar(Box::new(Lvar {
            name: value(token),
            expression_l,
        }))
    }

    pub(crate) fn ivar(&self, token: Token) -> Node {
        let expression_l = self.loc(&token);
        Node::Ivar(Box::new(Ivar {
            name: value(token),
            expression_l,
        }))
    }

    pub(crate) fn gvar(&self, token: Token) -> Node {
        let expression_l = self.loc(&token);
        Node::Gvar(Box::new(Gvar {
            name: value(token),
            expression_l,
        }))
    }

    pub(crate) fn cvar(&self, token: Token) -> Node {
        let expression_l = self.loc(&token);
        Node::Cvar(Box::new(Cvar {
            name: value(token),
            expression_l,
        }))
    }

    pub(crate) fn back_ref(&self, token: Token) -> Node {
        let expression_l = self.loc(&token);
        Node::BackRef(Box::new(BackRef {
            name: value(token),
            expression_l,
        }))
    }

    const MAX_NTH_REF: usize = 0b111111111111111111111111111111;

    pub(crate) fn nth_ref(&self, token: Token) -> Node {
        let expression_l = self.loc(&token);
        let name = value(token)[1..].to_owned();
        let parsed = name.parse::<usize>();

        if parsed.is_err() || parsed.map(|n| n > Self::MAX_NTH_REF) == Ok(true) {
            self.warn(
                DiagnosticMessage::NthRefIsTooBig(name.clone()),
                expression_l.clone(),
            )
        }

        Node::NthRef(Box::new(NthRef { name, expression_l }))
    }
    pub(crate) fn accessible(&self, node: Node) -> Node {
        match node {
            Node::Lvar(inner) => {
                let Lvar { name, expression_l } = *inner;
                if self.static_env.is_declared(&name) {
                    if let Some(current_arg) = self.current_arg_stack.top() {
                        if current_arg == name {
                            self.error(
                                DiagnosticMessage::CircularArgumentReference(name.clone()),
                                expression_l.clone(),
                            );
                        }
                    }

                    Node::Lvar(Box::new(Lvar { name, expression_l }))
                } else {
                    Node::Send(Box::new(Send {
                        recv: None,
                        method_name: name,
                        args: vec![],
                        dot_l: None,
                        selector_l: Some(expression_l.clone()),
                        begin_l: None,
                        end_l: None,
                        operator_l: None,
                        expression_l,
                    }))
                }
            }
            _ => node,
        }
    }

    pub(crate) fn const_(&self, name_t: Token) -> Node {
        let name_l = self.loc(&name_t);
        let expression_l = name_l.clone();

        Node::Const(Box::new(Const {
            scope: None,
            name: value(name_t),
            double_colon_l: None,
            name_l,
            expression_l,
        }))
    }

    pub(crate) fn const_global(&self, t_colon3: Token, name_t: Token) -> Node {
        let scope = Node::Cbase(Box::new(Cbase {
            expression_l: self.loc(&t_colon3),
        }));

        let name_l = self.loc(&name_t);
        let expression_l = scope.expression().join(&name_l);
        let double_colon_l = self.loc(&t_colon3);

        Node::Const(Box::new(Const {
            scope: Some(scope),
            name: value(name_t),
            double_colon_l: Some(double_colon_l),
            name_l,
            expression_l,
        }))
    }

    pub(crate) fn const_fetch(&self, scope: Node, t_colon2: Token, name_t: Token) -> Node {
        let name_l = self.loc(&name_t);
        let expression_l = scope.expression().join(&name_l);
        let double_colon_l = self.loc(&t_colon2);

        Node::Const(Box::new(Const {
            scope: Some(scope),
            name: value(name_t),
            double_colon_l: Some(double_colon_l),
            name_l,
            expression_l,
        }))
    }

    pub(crate) fn __encoding__(&self, encoding_t: Token) -> Node {
        Node::Encoding(Box::new(Encoding {
            expression_l: self.loc(&encoding_t),
        }))
    }

    //
    // Assignments
    //

    pub(crate) fn assignable(&self, node: Node) -> Result<Node, ()> {
        let node = match node {
            Node::Cvar(inner) => {
                let Cvar { name, expression_l } = *inner;
                Node::Cvasgn(Box::new(Cvasgn {
                    name,
                    value: None,
                    name_l: expression_l.clone(),
                    expression_l,
                    operator_l: None,
                }))
            }
            Node::Ivar(inner) => {
                let Ivar { name, expression_l } = *inner;
                Node::Ivasgn(Box::new(Ivasgn {
                    name,
                    value: None,
                    name_l: expression_l.clone(),
                    expression_l,
                    operator_l: None,
                }))
            }
            Node::Gvar(inner) => {
                let Gvar { name, expression_l } = *inner;
                Node::Gvasgn(Box::new(Gvasgn {
                    name,
                    value: None,
                    name_l: expression_l.clone(),
                    expression_l,
                    operator_l: None,
                }))
            }
            Node::Const(inner) => {
                let Const {
                    name,
                    scope,
                    expression_l,
                    double_colon_l,
                    name_l,
                } = *inner;
                if !self.context.is_dynamic_const_definition_allowed() {
                    self.error(DiagnosticMessage::DynamicConstantAssignment, expression_l);
                    return Err(());
                }
                Node::Casgn(Box::new(Casgn {
                    name,
                    scope,
                    value: None,
                    name_l,
                    double_colon_l,
                    expression_l,
                    operator_l: None,
                }))
            }
            Node::Lvar(inner) => {
                let Lvar { name, expression_l } = *inner;
                self.check_assignment_to_numparam(&name, &expression_l)?;
                self.check_reserved_for_numparam(&name, &expression_l)?;

                self.static_env.declare(&name);

                Node::Lvasgn(Box::new(Lvasgn {
                    name,
                    value: None,
                    name_l: expression_l.clone(),
                    expression_l,
                    operator_l: None,
                }))
            }

            Node::Self_(inner) => {
                let Self_ { expression_l } = *inner;
                self.error(DiagnosticMessage::CantAssignToSelf, expression_l);
                return Err(());
            }
            Node::Nil(inner) => {
                let Nil { expression_l } = *inner;
                self.error(DiagnosticMessage::CantAssignToNil, expression_l);
                return Err(());
            }
            Node::True(inner) => {
                let True { expression_l } = *inner;
                self.error(DiagnosticMessage::CantAssignToTrue, expression_l);
                return Err(());
            }
            Node::False(inner) => {
                let False { expression_l } = *inner;
                self.error(DiagnosticMessage::CantAssignToFalse, expression_l);
                return Err(());
            }
            Node::File(inner) => {
                let File { expression_l } = *inner;
                self.error(DiagnosticMessage::CantAssignToFile, expression_l);
                return Err(());
            }
            Node::Line(inner) => {
                let Line { expression_l } = *inner;
                self.error(DiagnosticMessage::CantAssignToLine, expression_l);
                return Err(());
            }
            Node::Encoding(inner) => {
                let Encoding { expression_l } = *inner;
                self.error(DiagnosticMessage::CantAssignToEncoding, expression_l);
                return Err(());
            }
            Node::BackRef(inner) => {
                let BackRef { expression_l, name } = *inner;
                self.error(DiagnosticMessage::CantSetVariable(name), expression_l);
                return Err(());
            }
            Node::NthRef(inner) => {
                let NthRef { expression_l, name } = *inner;
                self.error(
                    DiagnosticMessage::CantSetVariable(format!("${}", name)),
                    expression_l,
                );
                return Err(());
            }
            _ => unreachable!("{:?} can't be used in assignment", node),
        };

        Ok(node)
    }

    pub(crate) fn const_op_assignable(&self, node: Node) -> Node {
        match node {
            Node::Const(inner) => {
                let Const {
                    scope,
                    name,
                    name_l,
                    double_colon_l,
                    expression_l,
                } = *inner;
                Node::Casgn(Box::new(Casgn {
                    scope,
                    name,
                    name_l,
                    double_colon_l,
                    expression_l,
                    value: None,
                    operator_l: None,
                }))
            }
            _ => unreachable!("unsupported const_op_assignable arument: {:?}", node),
        }
    }

    pub(crate) fn assign(&self, mut lhs: Node, eql_t: Token, new_rhs: Node) -> Node {
        let op_l = Some(self.loc(&eql_t));
        let expr_l = join_exprs(&lhs, &new_rhs);

        match &mut lhs {
            Node::Cvasgn(inner) => {
                inner.expression_l = expr_l;
                inner.operator_l = op_l;
                inner.value = Some(new_rhs);
            }
            Node::Ivasgn(inner) => {
                inner.expression_l = expr_l;
                inner.operator_l = op_l;
                inner.value = Some(new_rhs);
            }
            Node::Gvasgn(inner) => {
                inner.expression_l = expr_l;
                inner.operator_l = op_l;
                inner.value = Some(new_rhs);
            }
            Node::Lvasgn(inner) => {
                inner.expression_l = expr_l;
                inner.operator_l = op_l;
                inner.value = Some(new_rhs);
            }
            Node::Casgn(inner) => {
                inner.expression_l = expr_l;
                inner.operator_l = op_l;
                inner.value = Some(new_rhs);
            }
            Node::IndexAsgn(inner) => {
                inner.expression_l = expr_l;
                inner.operator_l = op_l;
                inner.value = Some(new_rhs);
            }
            Node::Send(inner) => {
                inner.expression_l = expr_l;
                inner.operator_l = op_l;
                if inner.args.is_empty() {
                    inner.args = vec![new_rhs];
                } else {
                    unreachable!("can't assign to method call with args")
                }
            }
            Node::CSend(inner) => {
                inner.expression_l = expr_l;
                inner.operator_l = op_l;
                if inner.args.is_empty() {
                    inner.args = vec![new_rhs];
                } else {
                    unreachable!("can't assign to method call with args")
                }
            }
            _ => unreachable!("{:?} can't be used in assignment", lhs),
        }

        lhs
    }

    pub(crate) fn op_assign(&self, mut lhs: Node, op_t: Token, rhs: Node) -> Result<Node, ()> {
        let operator_l = self.loc(&op_t);
        let mut operator = value(op_t);
        operator.pop();
        let expression_l = join_exprs(&lhs, &rhs);

        match lhs {
            Node::Gvasgn { .. }
            | Node::Ivasgn { .. }
            | Node::Lvasgn { .. }
            | Node::Cvasgn { .. }
            | Node::Casgn { .. }
            | Node::Send { .. }
            | Node::CSend { .. } => {}
            Node::Index(inner) => {
                let Index {
                    recv,
                    indexes,
                    begin_l,
                    end_l,
                    expression_l,
                } = *inner;
                lhs = Node::IndexAsgn(Box::new(IndexAsgn {
                    recv,
                    indexes,
                    value: None,
                    begin_l,
                    end_l,
                    expression_l,
                    operator_l: None,
                }));
            }
            Node::BackRef(inner) => {
                let BackRef { expression_l, name } = *inner;
                self.error(DiagnosticMessage::CantSetVariable(name), expression_l);
                return Err(());
            }
            Node::NthRef(inner) => {
                let NthRef { expression_l, name } = *inner;
                self.error(
                    DiagnosticMessage::CantSetVariable(format!("${}", name)),
                    expression_l,
                );
                return Err(());
            }
            _ => unreachable!("unsupported op_assign lhs {:?}", lhs),
        };

        let recv = lhs;
        let value = rhs;

        let result = match &operator[..] {
            "&&" => Node::AndAsgn(Box::new(AndAsgn {
                recv,
                value,
                operator_l,
                expression_l,
            })),
            "||" => Node::OrAsgn(Box::new(OrAsgn {
                recv,
                value,
                operator_l,
                expression_l,
            })),
            _ => Node::OpAsgn(Box::new(OpAsgn {
                recv,
                value,
                operator,
                operator_l,
                expression_l,
            })),
        };

        Ok(result)
    }

    pub(crate) fn multi_lhs(
        &self,
        begin_t: Option<Token>,
        items: Vec<Node>,
        end_t: Option<Token>,
    ) -> Node {
        let (begin_l, end_l, expression_l) = self.collection_map(&begin_t, &items, &end_t);
        Node::Mlhs(Box::new(Mlhs {
            items,
            begin_l,
            end_l,
            expression_l,
        }))
    }

    pub(crate) fn multi_assign(&self, lhs: Node, eql_t: Token, rhs: Node) -> Node {
        let operator_l = self.loc(&eql_t);
        let expression_l = join_exprs(&lhs, &rhs);

        Node::Masgn(Box::new(Masgn {
            lhs,
            rhs,
            operator_l,
            expression_l,
        }))
    }

    pub(crate) fn rassign(&self, lhs: Node, eql_t: Token, rhs: Node) -> Node {
        self.assign(rhs, eql_t, lhs)
    }

    pub(crate) fn multi_rassign(&self, lhs: Node, eql_t: Token, rhs: Node) -> Node {
        self.multi_assign(rhs, eql_t, lhs)
    }

    //
    // Class and module definition
    //

    pub(crate) fn def_class(
        &self,
        class_t: Token,
        name: Node,
        lt_t: Option<Token>,
        superclass: Option<Node>,
        body: Option<Node>,
        end_t: Token,
    ) -> Node {
        let keyword_l = self.loc(&class_t);
        let end_l = self.loc(&end_t);
        let operator_l = self.maybe_loc(&lt_t);
        let expression_l = keyword_l.join(&end_l);

        Node::Class(Box::new(Class {
            name,
            superclass,
            body,
            keyword_l,
            operator_l,
            end_l,
            expression_l,
        }))
    }

    pub(crate) fn def_sclass(
        &self,
        class_t: Token,
        lshift_t: Token,
        expr: Node,
        body: Option<Node>,
        end_t: Token,
    ) -> Node {
        let keyword_l = self.loc(&class_t);
        let end_l = self.loc(&end_t);
        let operator_l = self.loc(&lshift_t);
        let expression_l = keyword_l.join(&end_l);

        Node::SClass(Box::new(SClass {
            expr,
            body,
            keyword_l,
            operator_l,
            end_l,
            expression_l,
        }))
    }

    pub(crate) fn def_module(
        &self,
        module_t: Token,
        name: Node,
        body: Option<Node>,
        end_t: Token,
    ) -> Node {
        let keyword_l = self.loc(&module_t);
        let end_l = self.loc(&end_t);
        let expression_l = keyword_l.join(&end_l);

        Node::Module(Box::new(Module {
            name,
            body,
            keyword_l,
            end_l,
            expression_l,
        }))
    }

    //
    // Method (un)definition
    //

    pub(crate) fn def_method(
        &self,
        def_t: Token,
        name_t: Token,
        args: Option<Node>,
        body: Option<Node>,
        end_t: Token,
    ) -> Result<Node, ()> {
        let name_l = self.loc(&name_t);
        let keyword_l = self.loc(&def_t);
        let end_l = self.loc(&end_t);
        let expression_l = keyword_l.join(&end_l);

        let name = value(name_t);
        self.check_reserved_for_numparam(&name, &name_l)?;

        Ok(Node::Def(Box::new(Def {
            name,
            args,
            body,
            keyword_l,
            name_l,
            assignment_l: None,
            end_l: Some(end_l),
            expression_l,
        })))
    }

    pub(crate) fn def_endless_method(
        &self,
        def_t: Token,
        name_t: Token,
        args: Option<Node>,
        assignment_t: Token,
        body: Option<Node>,
    ) -> Result<Node, ()> {
        let body_l = maybe_node_expr(&body.as_ref())
            .unwrap_or_else(|| unreachable!("endless method always has a body"));

        let keyword_l = self.loc(&def_t);
        let expression_l = keyword_l.join(&body_l);
        let name_l = self.loc(&name_t);
        let assignment_l = self.loc(&assignment_t);

        let name = value(name_t);
        self.check_reserved_for_numparam(&name, &name_l)?;

        Ok(Node::Def(Box::new(Def {
            name,
            args,
            body,
            keyword_l,
            name_l,
            assignment_l: Some(assignment_l),
            end_l: None,
            expression_l,
        })))
    }

    pub(crate) fn def_singleton(
        &self,
        def_t: Token,
        definee: Node,
        dot_t: Token,
        name_t: Token,
        args: Option<Node>,
        body: Option<Node>,
        end_t: Token,
    ) -> Result<Node, ()> {
        let keyword_l = self.loc(&def_t);
        let operator_l = self.loc(&dot_t);
        let name_l = self.loc(&name_t);
        let end_l = self.loc(&end_t);
        let expression_l = keyword_l.join(&end_l);

        let name = value(name_t);
        self.check_reserved_for_numparam(&name, &name_l)?;

        Ok(Node::Defs(Box::new(Defs {
            definee,
            name,
            args,
            body,
            keyword_l,
            operator_l,
            name_l,
            assignment_l: None,
            end_l: Some(end_l),
            expression_l,
        })))
    }

    pub(crate) fn def_endless_singleton(
        &self,
        def_t: Token,
        definee: Node,
        dot_t: Token,
        name_t: Token,
        args: Option<Node>,
        assignment_t: Token,
        body: Option<Node>,
    ) -> Result<Node, ()> {
        let body_l = maybe_node_expr(&body.as_ref())
            .unwrap_or_else(|| unreachable!("endless method always has body"));

        let keyword_l = self.loc(&def_t);
        let operator_l = self.loc(&dot_t);
        let name_l = self.loc(&name_t);
        let assignment_l = self.loc(&assignment_t);
        let expression_l = keyword_l.join(&body_l);

        let name = value(name_t);
        self.check_reserved_for_numparam(&name, &name_l)?;

        Ok(Node::Defs(Box::new(Defs {
            definee,
            name,
            args,
            body,
            keyword_l,
            operator_l,
            name_l,
            assignment_l: Some(assignment_l),
            end_l: None,
            expression_l,
        })))
    }

    pub(crate) fn undef_method(&self, undef_t: Token, names: Vec<Node>) -> Node {
        let keyword_l = self.loc(&undef_t);
        let expression_l = keyword_l.clone().maybe_join(&collection_expr(&names));
        Node::Undef(Box::new(Undef {
            names,
            keyword_l,
            expression_l,
        }))
    }

    pub(crate) fn alias(&self, alias_t: Token, to: Node, from: Node) -> Node {
        let keyword_l = self.loc(&alias_t);
        let expression_l = keyword_l.join(from.expression());
        Node::Alias(Box::new(Alias {
            to,
            from,
            keyword_l,
            expression_l,
        }))
    }

    //
    // Formal arguments
    //

    pub(crate) fn args(
        &self,
        begin_t: Option<Token>,
        args: Vec<Node>,
        end_t: Option<Token>,
    ) -> Option<Node> {
        self.check_duplicate_args(&args, &mut HashMap::new());

        if begin_t.is_none() && args.is_empty() && end_t.is_none() {
            return None;
        }

        let (begin_l, end_l, expression_l) = self.collection_map(&begin_t, &args, &end_t);
        Some(Node::Args(Box::new(Args {
            args,
            begin_l,
            end_l,
            expression_l,
        })))
    }

    pub(crate) fn forward_only_args(&self, begin_t: Token, dots_t: Token, end_t: Token) -> Node {
        let args = vec![self.forward_arg(dots_t)];
        let begin_l = self.loc(&begin_t);
        let end_l = self.loc(&end_t);
        let expression_l = begin_l.join(&end_l);
        Node::Args(Box::new(Args {
            args,
            begin_l: Some(begin_l),
            end_l: Some(end_l),
            expression_l,
        }))
    }

    pub(crate) fn forward_arg(&self, dots_t: Token) -> Node {
        Node::ForwardArg(Box::new(ForwardArg {
            expression_l: self.loc(&dots_t),
        }))
    }

    pub(crate) fn arg(&self, name_t: Token) -> Result<Node, ()> {
        let name_l = self.loc(&name_t);
        let name = value(name_t);

        self.check_reserved_for_numparam(&name, &name_l)?;

        Ok(Node::Arg(Box::new(Arg {
            name,
            expression_l: name_l,
        })))
    }

    pub(crate) fn optarg(&self, name_t: Token, eql_t: Token, default: Node) -> Result<Node, ()> {
        let operator_l = self.loc(&eql_t);
        let name_l = self.loc(&name_t);
        let expression_l = self.loc(&name_t).join(default.expression());

        let name = value(name_t);
        self.check_reserved_for_numparam(&name, &name_l)?;

        Ok(Node::Optarg(Box::new(Optarg {
            name,
            default,
            name_l,
            operator_l,
            expression_l,
        })))
    }

    pub(crate) fn restarg(&self, star_t: Token, name_t: Option<Token>) -> Result<Node, ()> {
        let (name, name_l) = match name_t {
            Some(name_t) => {
                let name_l = self.loc(&name_t);
                let name = value(name_t);
                self.check_reserved_for_numparam(&name, &name_l)?;
                (Some(name), Some(name_l))
            }
            _ => (None, None),
        };

        let operator_l = self.loc(&star_t);
        let expression_l = operator_l.clone().maybe_join(&name_l);

        Ok(Node::Restarg(Box::new(Restarg {
            name,
            operator_l,
            name_l,
            expression_l,
        })))
    }

    pub(crate) fn kwarg(&self, name_t: Token) -> Result<Node, ()> {
        let name_l = self.loc(&name_t);
        let name = value(name_t);
        self.check_reserved_for_numparam(&name, &name_l)?;

        let expression_l = name_l;
        let name_l = expression_l.adjust_end(-1);

        Ok(Node::Kwarg(Box::new(Kwarg {
            name,
            name_l,
            expression_l,
        })))
    }

    pub(crate) fn kwoptarg(&self, name_t: Token, default: Node) -> Result<Node, ()> {
        let name_l = self.loc(&name_t);
        let name = value(name_t);
        self.check_reserved_for_numparam(&name, &name_l)?;

        let label_l = name_l;
        let name_l = label_l.adjust_end(-1);
        let expression_l = default.expression().join(&label_l);

        Ok(Node::Kwoptarg(Box::new(Kwoptarg {
            name,
            default,
            name_l,
            expression_l,
        })))
    }

    pub(crate) fn kwrestarg(&self, dstar_t: Token, name_t: Option<Token>) -> Result<Node, ()> {
        let (name, name_l) = match name_t {
            Some(name_t) => {
                let name_l = self.loc(&name_t);
                let name = value(name_t);
                self.check_reserved_for_numparam(&name, &name_l)?;
                (Some(name), Some(name_l))
            }
            _ => (None, None),
        };

        let operator_l = self.loc(&dstar_t);
        let expression_l = operator_l.clone().maybe_join(&name_l);

        Ok(Node::Kwrestarg(Box::new(Kwrestarg {
            name,
            operator_l,
            name_l,
            expression_l,
        })))
    }

    pub(crate) fn kwnilarg(&self, dstar_t: Token, nil_t: Token) -> Node {
        let dstar_l = self.loc(&dstar_t);
        let nil_l = self.loc(&nil_t);
        let expression_l = dstar_l.join(&nil_l);
        Node::Kwnilarg(Box::new(Kwnilarg {
            name_l: nil_l,
            expression_l,
        }))
    }

    pub(crate) fn shadowarg(&self, name_t: Token) -> Result<Node, ()> {
        let name_l = self.loc(&name_t);
        let name = value(name_t);
        self.check_reserved_for_numparam(&name, &name_l)?;

        Ok(Node::Shadowarg(Box::new(Shadowarg {
            name,
            expression_l: name_l,
        })))
    }

    pub(crate) fn blockarg(&self, amper_t: Token, name_t: Token) -> Result<Node, ()> {
        let name_l = self.loc(&name_t);
        let name = value(name_t);
        self.check_reserved_for_numparam(&name, &name_l)?;

        let operator_l = self.loc(&amper_t);
        let expression_l = operator_l.join(&name_l);

        Ok(Node::Blockarg(Box::new(Blockarg {
            name,
            operator_l,
            name_l,
            expression_l,
        })))
    }

    pub(crate) fn procarg0(&self, arg: Node) -> Node {
        match arg {
            Node::Mlhs(inner) => {
                let Mlhs {
                    items,
                    begin_l,
                    end_l,
                    expression_l,
                } = *inner;
                Node::Procarg0(Box::new(Procarg0 {
                    args: items,
                    begin_l,
                    end_l,
                    expression_l,
                }))
            }
            Node::Arg(arg) => Node::Procarg0(Box::new(Procarg0 {
                expression_l: arg.expression_l.clone(),
                args: vec![Node::Arg(arg)],
                begin_l: None,
                end_l: None,
            })),
            other => unreachable!("unsupported procarg0 child {:?}", other),
        }
    }

    //
    // Method calls
    //

    fn call_type_for_dot(&self, dot_t: &Option<Token>) -> MethodCallType {
        match dot_t {
            Some(Token {
                token_type: Lexer::tANDDOT,
                ..
            }) => MethodCallType::CSend,
            _ => MethodCallType::Send,
        }
    }

    pub(crate) fn forwarded_args(&self, dots_t: Token) -> Node {
        Node::ForwardedArgs(Box::new(ForwardedArgs {
            expression_l: self.loc(&dots_t),
        }))
    }

    pub(crate) fn call_method(
        &self,
        receiver: Option<Node>,
        dot_t: Option<Token>,
        selector_t: Option<Token>,
        lparen_t: Option<Token>,
        args: Vec<Node>,
        rparen_t: Option<Token>,
    ) -> Node {
        let begin_l = maybe_node_expr(&receiver.as_ref())
            .or_else(|| self.maybe_loc(&selector_t))
            .unwrap_or_else(|| unreachable!("can't compute begin_l"));
        let end_l = self
            .maybe_loc(&rparen_t)
            .or_else(|| maybe_node_expr(&args.last()))
            .or_else(|| self.maybe_loc(&selector_t))
            .unwrap_or_else(|| unreachable!("can't compute end_l"));

        let expression_l = begin_l.join(&end_l);

        let dot_l = self.maybe_loc(&dot_t);
        let selector_l = self.maybe_loc(&selector_t);
        let begin_l = self.maybe_loc(&lparen_t);
        let end_l = self.maybe_loc(&rparen_t);

        let method_name = maybe_value(selector_t).unwrap_or_else(|| "call".to_owned());

        match self.call_type_for_dot(&dot_t) {
            MethodCallType::Send => Node::Send(Box::new(Send {
                method_name,
                recv: receiver,
                args,
                dot_l,
                selector_l,
                begin_l,
                end_l,
                operator_l: None,
                expression_l,
            })),

            MethodCallType::CSend => Node::CSend(Box::new(CSend {
                method_name,
                recv: receiver.expect("csend node must have a receiver"),
                args,
                dot_l: dot_l.expect("csend node must have &."),
                selector_l: selector_l.expect("csend node must have a method name"),
                begin_l,
                end_l,
                operator_l: None,
                expression_l,
            })),
        }
    }

    pub(crate) fn call_lambda(&self, lambda_t: Token) -> Node {
        Node::Lambda(Box::new(Lambda {
            expression_l: self.loc(&lambda_t),
        }))
    }

    pub(crate) fn block(
        &self,
        method_call: Node,
        begin_t: Token,
        block_args: ArgsType,
        body: Option<Node>,
        end_t: Token,
    ) -> Result<Node, ()> {
        let block_body = body;

        let validate_block_and_block_arg = |args: &Vec<Node>| {
            if let Some(last_arg) = args.last() {
                match last_arg {
                    Node::BlockPass(_) | Node::ForwardedArgs(_) => {
                        self.error(
                            DiagnosticMessage::BlockAndBlockArgGiven,
                            last_arg.inner_ref().expression().clone(),
                        );
                        Err(())
                    }
                    _ => Ok(()),
                }
            } else {
                Ok(())
            }
        };

        match &method_call {
            Node::Yield(inner) => {
                self.error(
                    DiagnosticMessage::BlockGivenToYield,
                    inner.keyword_l.clone(),
                );
                return Err(());
            }
            Node::Send(inner) => {
                validate_block_and_block_arg(&inner.args)?;
            }
            Node::CSend(inner) => {
                validate_block_and_block_arg(&inner.args)?;
            }
            _ => {}
        }

        let rewrite_args_and_loc = |method_args: &[Node],
                                    keyword_expression_l: &Range,
                                    block_args: ArgsType,
                                    block_body: Option<Node>| {
            // Code like "return foo 1 do end" is reduced in a weird sequence.
            // Here, method_call is actually (return).
            let actual_send = method_args[0].clone();

            let begin_l = self.loc(&begin_t);
            let end_l = self.loc(&end_t);
            let expression_l = actual_send.expression().join(&end_l);

            let block = match block_args {
                ArgsType::Args(args) => Node::Block(Box::new(Block {
                    call: actual_send,
                    args,
                    body: block_body,
                    begin_l,
                    end_l,
                    expression_l,
                })),
                ArgsType::Numargs(numargs) => Node::Numblock(Box::new(Numblock {
                    call: actual_send,
                    numargs,
                    body: block_body.expect("numblock always has body"),
                    begin_l,
                    end_l,
                    expression_l,
                })),
            };

            let expr_l = keyword_expression_l.join(block.expression());
            let args = vec![block];

            (args, expr_l)
        };

        match &method_call {
            Node::Send(_)
            | Node::CSend(_)
            | Node::Index(_)
            | Node::Super(_)
            | Node::ZSuper(_)
            | Node::Lambda(_) => {
                let begin_l = self.loc(&begin_t);
                let end_l = self.loc(&end_t);
                let expression_l = method_call.expression().join(&end_l);

                let result = match block_args {
                    ArgsType::Args(args) => Node::Block(Box::new(Block {
                        call: method_call,
                        args,
                        body: block_body,
                        begin_l,
                        end_l,
                        expression_l,
                    })),
                    ArgsType::Numargs(numargs) => Node::Numblock(Box::new(Numblock {
                        numargs,
                        call: method_call,
                        body: block_body.expect("numblock always has body"),
                        begin_l,
                        end_l,
                        expression_l,
                    })),
                };
                return Ok(result);
            }
            _ => {}
        };

        let result = match method_call {
            Node::Return(inner) => {
                let Return {
                    args,
                    keyword_l,
                    expression_l,
                } = *inner;
                let (args, expression_l) =
                    rewrite_args_and_loc(&args, &expression_l, block_args, block_body);
                Node::Return(Box::new(Return {
                    args,
                    keyword_l,
                    expression_l,
                }))
            }
            Node::Next(inner) => {
                let Next {
                    args,
                    keyword_l,
                    expression_l,
                } = *inner;
                let (args, expression_l) =
                    rewrite_args_and_loc(&args, &expression_l, block_args, block_body);
                Node::Next(Box::new(Next {
                    args,
                    keyword_l,
                    expression_l,
                }))
            }
            Node::Break(inner) => {
                let Break {
                    args,
                    keyword_l,
                    expression_l,
                } = *inner;
                let (args, expression_l) =
                    rewrite_args_and_loc(&args, &expression_l, block_args, block_body);
                Node::Break(Box::new(Break {
                    args,
                    keyword_l,
                    expression_l,
                }))
            }
            _ => unreachable!("unsupported method call {:?}", method_call),
        };

        Ok(result)
    }
    pub(crate) fn block_pass(&self, amper_t: Token, value: Node) -> Node {
        let amper_l = self.loc(&amper_t);
        let expression_l = value.expression().join(&amper_l);

        Node::BlockPass(Box::new(BlockPass {
            value,
            operator_l: amper_l,
            expression_l,
        }))
    }

    pub(crate) fn attr_asgn(&self, receiver: Node, dot_t: Token, selector_t: Token) -> Node {
        let dot_l = self.loc(&dot_t);
        let selector_l = self.loc(&selector_t);
        let expression_l = receiver.expression().join(&selector_l);

        let method_name = value(selector_t) + "=";

        match self.call_type_for_dot(&Some(dot_t)) {
            MethodCallType::Send => Node::Send(Box::new(Send {
                method_name,
                recv: Some(receiver),
                args: vec![],
                dot_l: Some(dot_l),
                selector_l: Some(selector_l),
                begin_l: None,
                end_l: None,
                operator_l: None,
                expression_l,
            })),

            MethodCallType::CSend => Node::CSend(Box::new(CSend {
                method_name,
                recv: receiver,
                args: vec![],
                dot_l,
                selector_l,
                begin_l: None,
                end_l: None,
                operator_l: None,
                expression_l,
            })),
        }
    }

    pub(crate) fn index(
        &self,
        recv: Node,
        lbrack_t: Token,
        indexes: Vec<Node>,
        rbrack_t: Token,
    ) -> Node {
        let begin_l = self.loc(&lbrack_t);
        let end_l = self.loc(&rbrack_t);
        let expression_l = recv.expression().join(&end_l);

        Node::Index(Box::new(Index {
            recv,
            indexes,
            begin_l,
            end_l,
            expression_l,
        }))
    }

    pub(crate) fn index_asgn(
        &self,
        recv: Node,
        lbrack_t: Token,
        indexes: Vec<Node>,
        rbrack_t: Token,
    ) -> Node {
        let begin_l = self.loc(&lbrack_t);
        let end_l = self.loc(&rbrack_t);
        let expression_l = recv.expression().join(&end_l);

        Node::IndexAsgn(Box::new(IndexAsgn {
            recv,
            indexes,
            value: None,
            begin_l,
            end_l,
            operator_l: None,
            expression_l,
        }))
    }

    pub(crate) fn binary_op(
        &self,
        receiver: Node,
        operator_t: Token,
        arg: Node,
    ) -> Result<Node, ()> {
        self.value_expr(&receiver)?;
        self.value_expr(&arg)?;

        let selector_l = self.loc(&operator_t);
        let expression_l = join_exprs(&receiver, &arg);

        Ok(Node::Send(Box::new(Send {
            recv: Some(receiver),
            method_name: value(operator_t),
            args: vec![arg],
            dot_l: None,
            selector_l: Some(selector_l),
            begin_l: None,
            end_l: None,
            operator_l: None,
            expression_l,
        })))
    }

    pub(crate) fn match_op(&self, receiver: Node, match_t: Token, arg: Node) -> Result<Node, ()> {
        self.value_expr(&receiver)?;
        self.value_expr(&arg)?;

        let selector_l = self.loc(&match_t);
        let expression_l = join_exprs(&receiver, &arg);

        let result = match self.static_regexp_captures(&receiver) {
            Some(captures) => {
                for capture in captures {
                    self.static_env.declare(&capture);
                }

                Node::MatchWithLvasgn(Box::new(MatchWithLvasgn {
                    re: receiver,
                    value: arg,
                    operator_l: selector_l,
                    expression_l,
                }))
            }
            None => Node::Send(Box::new(Send {
                recv: Some(receiver),
                method_name: String::from("=~"),
                args: vec![arg],
                dot_l: None,
                selector_l: Some(selector_l),
                begin_l: None,
                end_l: None,
                operator_l: None,
                expression_l,
            })),
        };

        Ok(result)
    }

    pub(crate) fn unary_op(&self, op_t: Token, receiver: Node) -> Result<Node, ()> {
        self.value_expr(&receiver)?;

        let selector_l = self.loc(&op_t);
        let expression_l = receiver.expression().join(&selector_l);

        let op = value(op_t);
        let method = if op == "+" || op == "-" { op + "@" } else { op };
        Ok(Node::Send(Box::new(Send {
            recv: Some(receiver),
            method_name: method,
            args: vec![],
            dot_l: None,
            selector_l: Some(selector_l),
            begin_l: None,
            end_l: None,
            operator_l: None,
            expression_l,
        })))
    }

    pub(crate) fn not_op(
        &self,
        not_t: Token,
        begin_t: Option<Token>,
        receiver: Option<Node>,
        end_t: Option<Token>,
    ) -> Result<Node, ()> {
        if let Some(receiver) = receiver {
            self.value_expr(&receiver)?;

            let begin_l = self.loc(&not_t);
            let end_l = self
                .maybe_loc(&end_t)
                .unwrap_or_else(|| receiver.expression().clone());

            let expression_l = begin_l.join(&end_l);

            let selector_l = self.loc(&not_t);
            let begin_l = self.maybe_loc(&begin_t);
            let end_l = self.maybe_loc(&end_t);

            Ok(Node::Send(Box::new(Send {
                recv: Some(self.check_condition(receiver)),
                method_name: "!".to_owned(),
                args: vec![],
                selector_l: Some(selector_l),
                dot_l: None,
                begin_l,
                end_l,
                operator_l: None,
                expression_l,
            })))
        } else {
            let (begin_l, end_l, expression_l) = self.collection_map(&begin_t, &[], &end_t);
            let nil_node = Node::Begin(Box::new(Begin {
                statements: vec![],
                begin_l,
                end_l,
                expression_l,
            }));

            let selector_l = self.loc(&not_t);
            let expression_l = nil_node.expression().join(&selector_l);
            Ok(Node::Send(Box::new(Send {
                recv: Some(nil_node),
                method_name: "!".to_owned(),
                args: vec![],
                selector_l: Some(selector_l),
                dot_l: None,
                begin_l: None,
                end_l: None,
                operator_l: None,
                expression_l,
            })))
        }
    }

    //
    // Control flow
    //

    // Logical operations: and, or

    pub(crate) fn logical_op(
        &self,
        type_: LogicalOp,
        lhs: Node,
        op_t: Token,
        rhs: Node,
    ) -> Result<Node, ()> {
        self.value_expr(&lhs)?;

        let operator_l = self.loc(&op_t);
        let expression_l = join_exprs(&lhs, &rhs);

        let result = match type_ {
            LogicalOp::And => Node::And(Box::new(And {
                lhs,
                rhs,
                operator_l,
                expression_l,
            })),
            LogicalOp::Or => Node::Or(Box::new(Or {
                lhs,
                rhs,
                operator_l,
                expression_l,
            })),
        };
        Ok(result)
    }

    // Conditionals

    pub(crate) fn condition(
        &self,
        cond_t: Token,
        cond: Node,
        then_t: Token,
        if_true: Option<Node>,
        else_t: Option<Token>,
        if_false: Option<Node>,
        end_t: Option<Token>,
    ) -> Node {
        let end_l = self
            .maybe_loc(&end_t)
            .or_else(|| maybe_node_expr(&if_false.as_ref()))
            .or_else(|| self.maybe_loc(&else_t))
            .or_else(|| maybe_node_expr(&if_true.as_ref()))
            .unwrap_or_else(|| self.loc(&then_t));

        let expression_l = self.loc(&cond_t).join(&end_l);
        let keyword_l = self.loc(&cond_t);
        let begin_l = self.loc(&then_t);
        let else_l = self.maybe_loc(&else_t);
        let end_l = self.maybe_loc(&end_t);

        Node::If(Box::new(If {
            cond: self.check_condition(cond),
            if_true,
            if_false,
            keyword_l,
            begin_l,
            else_l,
            end_l,
            expression_l,
        }))
    }

    pub(crate) fn condition_mod(
        &self,
        if_true: Option<Node>,
        if_false: Option<Node>,
        cond_t: Token,
        cond: Node,
    ) -> Node {
        let pre = match (&if_true, &if_false) {
            (None, None) => unreachable!("at least one of if_true/if_false is required"),
            (None, Some(if_false)) => if_false,
            (Some(if_true), None) => if_true,
            (Some(_), Some(_)) => unreachable!("only one of if_true/if_false is required"),
        };

        let expression_l = pre.expression().join(&cond.expression());
        let keyword_l = self.loc(&cond_t);

        Node::IfMod(Box::new(IfMod {
            cond: self.check_condition(cond),
            if_true,
            if_false,
            keyword_l,
            expression_l,
        }))
    }

    pub(crate) fn ternary(
        &self,
        cond: Node,
        question_t: Token,
        if_true: Node,
        colon_t: Token,
        if_false: Node,
    ) -> Node {
        let expression_l = join_exprs(&cond, &if_false);
        let question_l = self.loc(&question_t);
        let colon_l = self.loc(&colon_t);

        Node::IfTernary(Box::new(IfTernary {
            cond,
            if_true,
            if_false,
            question_l,
            colon_l,
            expression_l,
        }))
    }

    // Case matching

    pub(crate) fn when(
        &self,
        when_t: Token,
        patterns: Vec<Node>,
        then_t: Token,
        body: Option<Node>,
    ) -> Node {
        let begin_l = self.loc(&then_t);

        let expr_end_l = maybe_node_expr(&body.as_ref())
            .or_else(|| maybe_node_expr(&patterns.last()))
            .unwrap_or_else(|| self.loc(&when_t));
        let when_l = self.loc(&when_t);
        let expression_l = when_l.join(&expr_end_l);

        Node::When(Box::new(When {
            patterns,
            body,
            keyword_l: when_l,
            begin_l,
            expression_l,
        }))
    }

    pub(crate) fn case(
        &self,
        case_t: Token,
        expr: Option<Node>,
        when_bodies: Vec<Node>,
        else_t: Option<Token>,
        else_body: Option<Node>,
        end_t: Token,
    ) -> Node {
        let keyword_l = self.loc(&case_t);
        let else_l = self.maybe_loc(&else_t);
        let end_l = self.loc(&end_t);
        let expression_l = keyword_l.join(&end_l);

        Node::Case(Box::new(Case {
            expr,
            when_bodies,
            else_body,
            keyword_l,
            else_l,
            end_l,
            expression_l,
        }))
    }

    // Loops

    pub(crate) fn loop_(
        &self,
        loop_type: LoopType,
        keyword_t: Token,
        cond: Node,
        do_t: Token,
        body: Option<Node>,
        end_t: Token,
    ) -> Node {
        let keyword_l = self.loc(&keyword_t);
        let begin_l = self.loc(&do_t);
        let end_l = self.loc(&end_t);
        let expression_l = self.loc(&keyword_t).join(&end_l);

        let cond = self.check_condition(cond);

        match loop_type {
            LoopType::While => Node::While(Box::new(While {
                cond,
                body,
                keyword_l,
                begin_l: Some(begin_l),
                end_l: Some(end_l),
                expression_l,
            })),
            LoopType::Until => Node::Until(Box::new(Until {
                cond,
                body,
                keyword_l,
                begin_l: Some(begin_l),
                end_l: Some(end_l),
                expression_l,
            })),
        }
    }

    pub(crate) fn loop_mod(
        &self,
        loop_type: LoopType,
        body: Node,
        keyword_t: Token,
        cond: Node,
    ) -> Node {
        let expression_l = body.expression().join(&cond.expression());
        let keyword_l = self.loc(&keyword_t);

        let cond = self.check_condition(cond);

        match (loop_type, &body) {
            (LoopType::While, Node::KwBegin(_)) => Node::WhilePost(Box::new(WhilePost {
                cond,
                body,
                keyword_l,
                expression_l,
            })),
            (LoopType::While, _) => Node::While(Box::new(While {
                cond,
                body: Some(body),
                keyword_l,
                expression_l,
                begin_l: None,
                end_l: None,
            })),
            (LoopType::Until, Node::KwBegin(_)) => Node::UntilPost(Box::new(UntilPost {
                cond,
                body,
                keyword_l,
                expression_l,
            })),
            (LoopType::Until, _) => Node::Until(Box::new(Until {
                cond,
                body: Some(body),
                keyword_l,
                expression_l,
                begin_l: None,
                end_l: None,
            })),
        }
    }

    pub(crate) fn for_(
        &self,
        for_t: Token,
        iterator: Node,
        in_t: Token,
        iteratee: Node,
        do_t: Token,
        body: Option<Node>,
        end_t: Token,
    ) -> Node {
        let keyword_l = self.loc(&for_t);
        let operator_l = self.loc(&in_t);
        let begin_l = self.loc(&do_t);
        let end_l = self.loc(&end_t);
        let expression_l = keyword_l.join(&end_l);

        Node::For(Box::new(For {
            iterator,
            iteratee,
            body,
            keyword_l,
            operator_l,
            begin_l,
            end_l,
            expression_l,
        }))
    }

    // Keywords

    pub(crate) fn keyword_cmd(
        &self,
        type_: KeywordCmd,
        keyword_t: Token,
        lparen_t: Option<Token>,
        mut args: Vec<Node>,
        rparen_t: Option<Token>,
    ) -> Result<Node, ()> {
        let keyword_l = self.loc(&keyword_t);

        if type_ == KeywordCmd::Yield && !args.is_empty() {
            if let Some(Node::BlockPass(_)) = args.last() {
                self.error(DiagnosticMessage::BlockGivenToYield, keyword_l);
                return Err(());
            }
        }

        let begin_l = self.maybe_loc(&lparen_t);
        let end_l = self.maybe_loc(&rparen_t);

        let expr_end_l = end_l
            .clone()
            .or_else(|| maybe_node_expr(&args.last()))
            .unwrap_or_else(|| keyword_l.clone());

        let expression_l = keyword_l.join(&expr_end_l);

        let result = match type_ {
            KeywordCmd::Break => Node::Break(Box::new(Break {
                args,
                keyword_l,
                expression_l,
            })),
            KeywordCmd::Defined => Node::Defined(Box::new(Defined {
                value: args.pop().expect("defined? always has an argument"),
                keyword_l,
                begin_l,
                end_l,
                expression_l,
            })),
            KeywordCmd::Next => Node::Next(Box::new(Next {
                args,
                keyword_l,
                expression_l,
            })),
            KeywordCmd::Redo => Node::Redo(Box::new(Redo { expression_l })),
            KeywordCmd::Retry => Node::Retry(Box::new(Retry { expression_l })),
            KeywordCmd::Return => Node::Return(Box::new(Return {
                args,
                keyword_l,
                expression_l,
            })),
            KeywordCmd::Super => Node::Super(Box::new(Super {
                args,
                keyword_l,
                begin_l,
                end_l,
                expression_l,
            })),
            KeywordCmd::Yield => Node::Yield(Box::new(Yield {
                args,
                keyword_l,
                begin_l,
                end_l,
                expression_l,
            })),
            KeywordCmd::Zsuper => Node::ZSuper(Box::new(ZSuper { expression_l })),
        };

        Ok(result)
    }

    // BEGIN, END

    pub(crate) fn preexe(
        &self,
        preexe_t: Token,
        lbrace_t: Token,
        compstmt: Option<Node>,
        rbrace_t: Token,
    ) -> Node {
        let keyword_l = self.loc(&preexe_t);
        let begin_l = self.loc(&lbrace_t);
        let end_l = self.loc(&rbrace_t);
        let expression_l = keyword_l.join(&end_l);

        Node::Preexe(Box::new(Preexe {
            body: compstmt,
            keyword_l,
            begin_l,
            end_l,
            expression_l,
        }))
    }
    pub(crate) fn postexe(
        &self,
        postexe_t: Token,
        lbrace_t: Token,
        compstmt: Option<Node>,
        rbrace_t: Token,
    ) -> Node {
        let keyword_l = self.loc(&postexe_t);
        let begin_l = self.loc(&lbrace_t);
        let end_l = self.loc(&rbrace_t);
        let expression_l = keyword_l.join(&end_l);

        Node::Postexe(Box::new(Postexe {
            body: compstmt,
            keyword_l,
            begin_l,
            end_l,
            expression_l,
        }))
    }

    // Exception handling

    pub(crate) fn rescue_body(
        &self,
        rescue_t: Token,
        exc_list: Option<Node>,
        assoc_t: Option<Token>,
        exc_var: Option<Node>,
        then_t: Option<Token>,
        body: Option<Node>,
    ) -> Node {
        let end_l = maybe_node_expr(&body.as_ref())
            .or_else(|| self.maybe_loc(&then_t))
            .or_else(|| maybe_node_expr(&exc_var.as_ref()))
            .or_else(|| maybe_node_expr(&exc_list.as_ref()))
            .unwrap_or_else(|| self.loc(&rescue_t));

        let expression_l = self.loc(&rescue_t).join(&end_l);
        let keyword_l = self.loc(&rescue_t);
        let assoc_l = self.maybe_loc(&assoc_t);
        let begin_l = self.maybe_loc(&then_t);

        Node::RescueBody(Box::new(RescueBody {
            exc_list: exc_list,
            exc_var: exc_var,
            body: body,
            keyword_l,
            begin_l,
            assoc_l,
            expression_l,
        }))
    }

    pub(crate) fn begin_body(
        &self,
        compound_stmt: Option<Node>,
        rescue_bodies: Vec<Node>,
        else_: Option<(Token, Option<Node>)>,
        ensure: Option<(Token, Option<Node>)>,
    ) -> Option<Node> {
        let mut result: Option<Node>;

        if !rescue_bodies.is_empty() {
            if let Some((else_t, else_)) = else_ {
                let begin_l = maybe_node_expr(&compound_stmt.as_ref())
                    .or_else(|| maybe_node_expr(&rescue_bodies.first()))
                    .unwrap_or_else(|| unreachable!("can't compute begin_l"));

                let end_l = maybe_node_expr(&else_.as_ref()).unwrap_or_else(|| self.loc(&else_t));

                let expression_l = begin_l.join(&end_l);
                let else_l = self.loc(&else_t);

                result = Some(Node::Rescue(Box::new(Rescue {
                    body: compound_stmt,
                    rescue_bodies,
                    else_,
                    else_l: Some(else_l),
                    expression_l,
                })))
            } else {
                let begin_l = maybe_node_expr(&compound_stmt.as_ref())
                    .or_else(|| maybe_node_expr(&rescue_bodies.first()))
                    .unwrap_or_else(|| unreachable!("can't compute begin_l"));

                let end_l = maybe_node_expr(&rescue_bodies.last())
                    .unwrap_or_else(|| unreachable!("can't compute end_l"));

                let expression_l = begin_l.join(&end_l);
                let else_l = self.maybe_loc(&None);

                result = Some(Node::Rescue(Box::new(Rescue {
                    body: compound_stmt,
                    rescue_bodies,
                    else_: None,
                    else_l,
                    expression_l,
                })))
            }
        } else if let Some((else_t, else_)) = else_ {
            let mut statements: Vec<Node> = vec![];

            match compound_stmt {
                Some(Node::Begin(inner)) => statements = (*inner).statements,
                Some(compound_stmt) => statements.push(compound_stmt),
                _ => {}
            }
            let parts = if let Some(else_) = else_ {
                vec![else_]
            } else {
                vec![]
            };
            let (begin_l, end_l, expression_l) = self.collection_map(&Some(else_t), &parts, &None);
            statements.push(Node::Begin(Box::new(Begin {
                statements: parts,
                begin_l,
                end_l,
                expression_l,
            })));

            let (begin_l, end_l, expression_l) = self.collection_map(&None, &statements, &None);
            result = Some(Node::Begin(Box::new(Begin {
                statements,
                begin_l,
                end_l,
                expression_l,
            })))
        } else {
            result = compound_stmt;
        }

        if let Some((ensure_t, ensure)) = ensure {
            let mut ensure_body = if let Some(ensure) = ensure {
                vec![ensure]
            } else {
                vec![]
            };
            let keyword_l = self.loc(&ensure_t);

            let begin_l = maybe_node_expr(&result.as_ref()).unwrap_or_else(|| self.loc(&ensure_t));

            let end_l = maybe_node_expr(&ensure_body.last()).unwrap_or_else(|| self.loc(&ensure_t));

            let expression_l = begin_l.join(&end_l);

            result = Some(Node::Ensure(Box::new(Ensure {
                body: result,
                ensure: ensure_body.pop(),
                keyword_l,
                expression_l,
            })))
        }

        result
    }

    //
    // Expression grouping
    //

    pub(crate) fn compstmt(&self, mut statements: Vec<Node>) -> Option<Node> {
        match &statements[..] {
            [] => None,
            [_] => statements.pop(),
            _ => {
                let (begin_l, end_l, expression_l) = self.collection_map(&None, &statements, &None);
                Some(Node::Begin(Box::new(Begin {
                    statements,
                    begin_l,
                    end_l,
                    expression_l,
                })))
            }
        }
    }

    pub(crate) fn begin(&self, begin_t: Token, body: Option<Node>, end_t: Token) -> Node {
        let begin_l = self.loc(&begin_t);
        let end_l = self.loc(&end_t);
        let expression_l = begin_l.join(&end_l);

        if let Some(mut body) = body {
            match &mut body {
                // Synthesized (begin) from compstmt "a; b" or (mlhs)
                // from multi_lhs "(a, b) = *foo".
                Node::Mlhs(inner) => {
                    inner.begin_l = Some(begin_l);
                    inner.end_l = Some(end_l);
                    inner.expression_l = expression_l;
                    body
                }
                Node::Begin(inner) if inner.begin_l.is_none() && inner.end_l.is_none() => {
                    inner.begin_l = Some(begin_l);
                    inner.end_l = Some(end_l);
                    inner.expression_l = expression_l;
                    body
                }
                _ => {
                    let statements = vec![body];
                    Node::Begin(Box::new(Begin {
                        statements,
                        begin_l: Some(begin_l),
                        end_l: Some(end_l),
                        expression_l,
                    }))
                }
            }
        } else {
            // A nil expression: `()'.
            Node::Begin(Box::new(Begin {
                statements: vec![],
                begin_l: Some(begin_l),
                end_l: Some(end_l),
                expression_l,
            }))
        }
    }

    pub(crate) fn begin_keyword(&self, begin_t: Token, body: Option<Node>, end_t: Token) -> Node {
        let begin_l = self.loc(&begin_t);
        let end_l = self.loc(&end_t);
        let expression_l = begin_l.join(&end_l);

        match body {
            None => {
                // A nil expression: `begin end'.
                Node::KwBegin(Box::new(KwBegin {
                    statements: vec![],
                    begin_l: Some(begin_l),
                    end_l: Some(end_l),
                    expression_l,
                }))
            }
            Some(Node::Begin(inner)) => {
                let Begin { statements, .. } = *inner;
                // Synthesized (begin) from compstmt "a; b".
                Node::KwBegin(Box::new(KwBegin {
                    statements,
                    begin_l: Some(begin_l),
                    end_l: Some(end_l),
                    expression_l,
                }))
            }
            Some(node) => {
                let statements = vec![node];
                Node::KwBegin(Box::new(KwBegin {
                    statements,
                    begin_l: Some(begin_l),
                    end_l: Some(end_l),
                    expression_l,
                }))
            }
        }
    }

    //
    // Pattern matching
    //

    pub(crate) fn case_match(
        &self,
        case_t: Token,
        expr: Node,
        in_bodies: Vec<Node>,
        else_t: Option<Token>,
        else_body: Option<Node>,
        end_t: Token,
    ) -> Node {
        let else_body = match (&else_t, &else_body) {
            (Some(else_t), None) => Some(Node::EmptyElse(Box::new(EmptyElse {
                expression_l: self.loc(else_t),
            }))),
            _ => else_body,
        };

        let keyword_l = self.loc(&case_t);
        let else_l = self.maybe_loc(&else_t);
        let end_l = self.loc(&end_t);
        let expression_l = self.loc(&case_t).join(&end_l);

        Node::CaseMatch(Box::new(CaseMatch {
            expr,
            in_bodies,
            else_body,
            keyword_l,
            else_l,
            end_l,
            expression_l,
        }))
    }

    pub(crate) fn in_match(&self, value: Node, in_t: Token, pattern: Node) -> Node {
        let keyword_l = self.loc(&in_t);
        let expression_l = join_exprs(&value, &pattern);

        Node::InMatch(Box::new(InMatch {
            value,
            pattern,
            operator_l: keyword_l,
            expression_l,
        }))
    }

    pub(crate) fn in_pattern(
        &self,
        in_t: Token,
        pattern: Node,
        guard: Option<Node>,
        then_t: Token,
        body: Option<Node>,
    ) -> Node {
        let keyword_l = self.loc(&in_t);
        let begin_l = self.loc(&then_t);

        let expression_l = maybe_node_expr(&body.as_ref())
            .or_else(|| maybe_node_expr(&guard.as_ref()))
            .unwrap_or_else(|| pattern.expression().clone())
            .join(&keyword_l);

        Node::InPattern(Box::new(InPattern {
            pattern,
            guard,
            body,
            keyword_l,
            begin_l,
            expression_l,
        }))
    }

    pub(crate) fn if_guard(&self, if_t: Token, cond: Node) -> Node {
        let keyword_l = self.loc(&if_t);
        let expression_l = keyword_l.join(cond.expression());

        Node::IfGuard(Box::new(IfGuard {
            cond,
            keyword_l,
            expression_l,
        }))
    }
    pub(crate) fn unless_guard(&self, unless_t: Token, cond: Node) -> Node {
        let keyword_l = self.loc(&unless_t);
        let expression_l = keyword_l.join(cond.expression());

        Node::UnlessGuard(Box::new(UnlessGuard {
            cond,
            keyword_l,
            expression_l,
        }))
    }

    pub(crate) fn match_var(&self, name_t: Token) -> Result<Node, ()> {
        let name_l = self.loc(&name_t);
        let expression_l = name_l.clone();
        let name = value(name_t);

        self.check_lvar_name(&name, &name_l)?;
        self.check_duplicate_pattern_variable(&name, &name_l)?;
        self.static_env.declare(&name);

        Ok(Node::MatchVar(Box::new(MatchVar {
            name,
            name_l,
            expression_l,
        })))
    }

    pub(crate) fn match_hash_var(&self, name_t: Token) -> Result<Node, ()> {
        let expression_l = self.loc(&name_t);
        let name_l = expression_l.adjust_end(-1);

        let name = value(name_t);

        self.check_lvar_name(&name, &name_l)?;
        self.check_duplicate_pattern_variable(&name, &name_l)?;
        self.static_env.declare(&name);

        Ok(Node::MatchVar(Box::new(MatchVar {
            name,
            name_l,
            expression_l,
        })))
    }
    pub(crate) fn match_hash_var_from_str(
        &self,
        begin_t: Token,
        mut strings: Vec<Node>,
        end_t: Token,
    ) -> Result<Node, ()> {
        if strings.len() != 1 {
            self.error(
                DiagnosticMessage::SymbolLiteralWithInterpolation,
                self.loc(&begin_t).join(&self.loc(&end_t)),
            );
            return Err(());
        }

        let result = match strings.remove(0) {
            Node::Str(inner) => {
                let Str {
                    value,
                    begin_l,
                    end_l,
                    expression_l,
                } = *inner;

                let name = value.to_string_lossy();
                let mut name_l = expression_l.clone();

                self.check_lvar_name(&name, &name_l)?;
                self.check_duplicate_pattern_variable(&name, &name_l)?;

                self.static_env.declare(&name);

                if let Some(begin_l) = &begin_l {
                    let begin_pos_d: i32 = begin_l
                        .size()
                        .try_into()
                        .expect("failed to convert usize loc into i32, is it too big?");
                    name_l = name_l.adjust_begin(begin_pos_d)
                }

                if let Some(end_l) = &end_l {
                    let end_pos_d: i32 = end_l
                        .size()
                        .try_into()
                        .expect("failed to convert usize loc into i32, is it too big?");
                    name_l = name_l.adjust_end(-end_pos_d)
                }

                let expression_l = self
                    .loc(&begin_t)
                    .join(&expression_l)
                    .join(&self.loc(&end_t));
                Node::MatchVar(Box::new(MatchVar {
                    name,
                    name_l,
                    expression_l,
                }))
            }
            Node::Begin(inner) => self.match_hash_var_from_str(begin_t, inner.statements, end_t)?,
            _ => {
                self.error(
                    DiagnosticMessage::SymbolLiteralWithInterpolation,
                    self.loc(&begin_t).join(&self.loc(&end_t)),
                );
                return Err(());
            }
        };

        Ok(result)
    }

    pub(crate) fn match_rest(&self, star_t: Token, name_t: Option<Token>) -> Result<Node, ()> {
        let name = match name_t {
            None => None,
            Some(t) => Some(self.match_var(t)?),
        };

        let operator_l = self.loc(&star_t);
        let expression_l = operator_l
            .clone()
            .maybe_join(&maybe_node_expr(&name.as_ref()));

        Ok(Node::MatchRest(Box::new(MatchRest {
            name: name,
            operator_l,
            expression_l,
        })))
    }

    pub(crate) fn hash_pattern(
        &self,
        lbrace_t: Option<Token>,
        kwargs: Vec<Node>,
        rbrace_t: Option<Token>,
    ) -> Node {
        let (begin_l, end_l, expression_l) = self.collection_map(&lbrace_t, &kwargs, &rbrace_t);
        Node::HashPattern(Box::new(HashPattern {
            elements: kwargs,
            begin_l,
            end_l,
            expression_l,
        }))
    }

    pub(crate) fn array_pattern(
        &self,
        lbrack_t: Option<Token>,
        elements: Vec<Node>,
        trailing_comma: Option<Token>,
        rbrack_t: Option<Token>,
    ) -> Node {
        let (begin_l, end_l, expression_l) = self.collection_map(&lbrack_t, &elements, &rbrack_t);
        let expression_l = expression_l.maybe_join(&self.maybe_loc(&trailing_comma));

        if elements.is_empty() {
            return Node::ArrayPattern(Box::new(ArrayPattern {
                elements: vec![],
                begin_l,
                end_l,
                expression_l,
            }));
        }

        if trailing_comma.is_some() {
            Node::ArrayPatternWithTail(Box::new(ArrayPatternWithTail {
                elements,
                begin_l,
                end_l,
                expression_l,
            }))
        } else {
            Node::ArrayPattern(Box::new(ArrayPattern {
                elements,
                begin_l,
                end_l,
                expression_l,
            }))
        }
    }

    pub(crate) fn find_pattern(
        &self,
        lbrack_t: Option<Token>,
        elements: Vec<Node>,
        rbrack_t: Option<Token>,
    ) -> Node {
        let (begin_l, end_l, expression_l) = self.collection_map(&lbrack_t, &elements, &rbrack_t);
        Node::FindPattern(Box::new(FindPattern {
            elements,
            begin_l,
            end_l,
            expression_l,
        }))
    }

    pub(crate) fn const_pattern(
        &self,
        const_: Node,
        ldelim_t: Token,
        pattern: Node,
        rdelim_t: Token,
    ) -> Node {
        let begin_l = self.loc(&ldelim_t);
        let end_l = self.loc(&rdelim_t);
        let expression_l = const_.expression().join(&self.loc(&rdelim_t));

        Node::ConstPattern(Box::new(ConstPattern {
            const_,
            pattern,
            begin_l,
            end_l,
            expression_l,
        }))
    }

    pub(crate) fn pin(&self, pin_t: Token, var: Node) -> Node {
        let operator_l = self.loc(&pin_t);
        let expression_l = var.expression().join(&operator_l);

        Node::Pin(Box::new(Pin {
            var,
            selector_l: operator_l,
            expression_l,
        }))
    }

    pub(crate) fn match_alt(&self, lhs: Node, pipe_t: Token, rhs: Node) -> Node {
        let operator_l = self.loc(&pipe_t);
        let expression_l = join_exprs(&lhs, &rhs);

        Node::MatchAlt(Box::new(MatchAlt {
            lhs,
            rhs,
            operator_l,
            expression_l,
        }))
    }

    pub(crate) fn match_as(&self, value: Node, assoc_t: Token, as_: Node) -> Node {
        let operator_l = self.loc(&assoc_t);
        let expression_l = join_exprs(&value, &as_);

        Node::MatchAs(Box::new(MatchAs {
            value,
            as_,
            operator_l,
            expression_l,
        }))
    }

    pub(crate) fn match_nil_pattern(&self, dstar_t: Token, nil_t: Token) -> Node {
        let operator_l = self.loc(&dstar_t);
        let name_l = self.loc(&nil_t);
        let expression_l = operator_l.join(&name_l);

        Node::MatchNilPattern(Box::new(MatchNilPattern {
            operator_l,
            name_l,
            expression_l,
        }))
    }

    pub(crate) fn match_pair(&self, p_kw_label: PKwLabel, value_node: Node) -> Result<Node, ()> {
        let result = match p_kw_label {
            PKwLabel::PlainLabel(label_t) => {
                self.check_duplicate_pattern_key(&clone_value(&label_t), &self.loc(&label_t))?;
                self.pair_keyword(label_t, value_node)
            }
            PKwLabel::QuotedLabel((begin_t, parts, end_t)) => {
                let label_loc = self.loc(&begin_t).join(&self.loc(&end_t));

                match self.static_string(&parts) {
                    Some(var_name) => self.check_duplicate_pattern_key(&var_name, &label_loc)?,
                    _ => {
                        self.error(DiagnosticMessage::SymbolLiteralWithInterpolation, label_loc);
                        return Err(());
                    }
                }

                self.pair_quoted(begin_t, parts, end_t, value_node)
            }
        };
        Ok(result)
    }

    pub(crate) fn match_label(&self, p_kw_label: PKwLabel) -> Result<Node, ()> {
        match p_kw_label {
            PKwLabel::PlainLabel(label_t) => self.match_hash_var(label_t),
            PKwLabel::QuotedLabel((begin_t, parts, end_t)) => {
                self.match_hash_var_from_str(begin_t, parts, end_t)
            }
        }
    }

    //
    // Verification
    //

    pub(crate) fn check_condition(&self, cond: Node) -> Node {
        match cond {
            Node::Begin(inner) => {
                if inner.statements.len() == 1 {
                    let Begin {
                        statements,
                        begin_l,
                        end_l,
                        expression_l,
                    } = *inner;

                    let stmt = first(statements);
                    let stmt = self.check_condition(stmt);
                    Node::Begin(Box::new(Begin {
                        statements: vec![stmt],
                        begin_l,
                        end_l,
                        expression_l,
                    }))
                } else {
                    Node::Begin(inner)
                }
            }
            Node::And(mut inner) => {
                inner.lhs = self.check_condition(inner.lhs);
                inner.rhs = self.check_condition(inner.rhs);
                Node::And(inner)
            }
            Node::Or(mut inner) => {
                inner.lhs = self.check_condition(inner.lhs);
                inner.rhs = self.check_condition(inner.rhs);
                Node::Or(inner)
            }
            Node::Irange(inner) => {
                let Irange {
                    left,
                    right,
                    operator_l,
                    expression_l,
                } = *inner;
                Node::IFlipFlop(Box::new(IFlipFlop {
                    left: left.map(|node| self.check_condition(node)),
                    right: right.map(|node| self.check_condition(node)),
                    operator_l,
                    expression_l,
                }))
            }
            Node::Erange(inner) => {
                let Erange {
                    left,
                    right,
                    operator_l,
                    expression_l,
                } = *inner;
                Node::EFlipFlop(Box::new(EFlipFlop {
                    left: left.map(|node| self.check_condition(node)),
                    right: right.map(|node| self.check_condition(node)),
                    operator_l,
                    expression_l,
                }))
            }
            Node::Regexp(inner) => Node::MatchCurrentLine(Box::new(MatchCurrentLine {
                expression_l: inner.expression_l.clone(),
                re: Node::Regexp(inner),
            })),
            _ => cond,
        }
    }

    pub(crate) fn check_duplicate_args<'a>(
        &self,
        args: &'a [Node],
        map: &mut HashMap<String, &'a Node>,
    ) {
        for arg in args {
            match arg {
                Node::Arg(_)
                | Node::Optarg(_)
                | Node::Restarg(_)
                | Node::Kwarg(_)
                | Node::Kwoptarg(_)
                | Node::Kwrestarg(_)
                | Node::Shadowarg(_)
                | Node::Blockarg(_) => {
                    self.check_duplicate_arg(arg, map);
                }
                Node::Mlhs(inner) => {
                    self.check_duplicate_args(&inner.items, map);
                }
                Node::Procarg0(inner) => {
                    self.check_duplicate_args(&inner.args, map);
                }
                Node::ForwardArg(_) | Node::Kwnilarg(_) => {}
                _ => unreachable!("unsupported arg type {:?}", arg),
            }
        }
    }

    fn arg_name<'a>(&self, node: &'a Node) -> Option<&'a String> {
        match node {
            Node::Arg(inner) => Some(&inner.name),
            Node::Optarg(inner) => Some(&inner.name),
            Node::Kwarg(inner) => Some(&inner.name),
            Node::Kwoptarg(inner) => Some(&inner.name),
            Node::Shadowarg(inner) => Some(&inner.name),
            Node::Blockarg(inner) => Some(&inner.name),
            Node::Restarg(inner) => inner.name.as_ref(),
            Node::Kwrestarg(inner) => inner.name.as_ref(),
            _ => unreachable!("unsupported arg {:?}", node),
        }
    }

    fn arg_name_loc<'a>(&self, node: &'a Node) -> &'a Range {
        match node {
            Node::Arg(inner) => &inner.expression_l,
            Node::Optarg(inner) => &inner.name_l,
            Node::Kwarg(inner) => &inner.name_l,
            Node::Kwoptarg(inner) => &inner.name_l,
            Node::Shadowarg(inner) => &inner.expression_l,
            Node::Blockarg(inner) => &inner.name_l,
            Node::Restarg(inner) => inner.name_l.as_ref().unwrap_or(&inner.expression_l),
            Node::Kwrestarg(inner) => inner.name_l.as_ref().unwrap_or(&inner.expression_l),

            _ => unreachable!("unsupported arg {:?}", node),
        }
    }

    pub(crate) fn check_duplicate_arg<'a>(
        &self,
        this_arg: &'a Node,
        map: &mut HashMap<String, &'a Node>,
    ) {
        let this_name = match self.arg_name(this_arg) {
            Some(name) => name,
            None => return,
        };

        let that_arg = map.get(this_name);

        match that_arg {
            None => {
                map.insert(this_name.to_owned(), this_arg);
            }
            Some(that_arg) => {
                let that_name = match self.arg_name(*that_arg) {
                    Some(name) => name,
                    None => return,
                };
                if self.arg_name_collides(this_name, that_name) {
                    self.error(
                        DiagnosticMessage::DuplicatedArgumentName,
                        self.arg_name_loc(this_arg).clone(),
                    )
                }
            }
        }
    }

    pub(crate) fn check_assignment_to_numparam(&self, name: &str, loc: &Range) -> Result<(), ()> {
        let assigning_to_numparam = self.context.is_in_dynamic_block()
            && matches!(
                name,
                "_1" | "_2" | "_3" | "_4" | "_5" | "_6" | "_7" | "_8" | "_9"
            )
            && self.max_numparam_stack.has_numparams();

        if assigning_to_numparam {
            self.error(
                DiagnosticMessage::CantAssignToNumparam(name.to_owned()),
                loc.clone(),
            );
            return Err(());
        }
        Ok(())
    }

    pub(crate) fn check_reserved_for_numparam(&self, name: &str, loc: &Range) -> Result<(), ()> {
        match name {
            "_1" | "_2" | "_3" | "_4" | "_5" | "_6" | "_7" | "_8" | "_9" => {
                self.error(
                    DiagnosticMessage::ReservedForNumparam(name.to_owned()),
                    loc.clone(),
                );
                Err(())
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn arg_name_collides(&self, this_name: &str, that_name: &str) -> bool {
        &this_name[0..1] != "_" && this_name == that_name
    }

    pub(crate) fn check_lvar_name(&self, name: &str, loc: &Range) -> Result<(), ()> {
        let first = name
            .chars()
            .next()
            .expect("local variable name can't be empty");
        let rest = &name[1..];

        if (first.is_lowercase() || first == '_')
            && rest.chars().all(|c| c.is_alphanumeric() || c == '_')
        {
            Ok(())
        } else {
            self.error(
                DiagnosticMessage::KeyMustBeValidAsLocalVariable,
                loc.clone(),
            );
            Err(())
        }
    }

    pub(crate) fn check_duplicate_pattern_variable(
        &self,
        name: &str,
        loc: &Range,
    ) -> Result<(), ()> {
        if name.starts_with('_') {
            return Ok(());
        }

        if self.pattern_variables.is_declared(name) {
            self.error(DiagnosticMessage::DuplicateVariableName, loc.clone());
            return Err(());
        }

        self.pattern_variables.declare(name);
        Ok(())
    }

    pub(crate) fn check_duplicate_pattern_key(&self, name: &str, loc: &Range) -> Result<(), ()> {
        if self.pattern_hash_keys.is_declared(name) {
            self.error(DiagnosticMessage::DuplicateKeyName, loc.clone());
            return Err(());
        }

        self.pattern_hash_keys.declare(name);
        Ok(())
    }

    //
    // Helpers
    //

    pub(crate) fn static_string(&self, nodes: &[Node]) -> Option<String> {
        let mut result = String::from("");

        for node in nodes {
            match node {
                Node::Str(inner) => {
                    let value = inner.value.to_string_lossy();
                    result.push_str(&value)
                }
                Node::Begin(inner) => {
                    if let Some(s) = self.static_string(&inner.statements) {
                        result.push_str(&s)
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        Some(result)
    }

    #[cfg(feature = "onig")]
    pub(crate) fn build_static_regexp(
        &self,
        parts: &[Node],
        options: &[char],
        range: &Range,
    ) -> Option<Regex> {
        let source = self.static_string(&parts)?;
        let mut reg_options = RegexOptions::REGEX_OPTION_NONE;
        reg_options |= RegexOptions::REGEX_OPTION_CAPTURE_GROUP;
        if options.contains(&'x') {
            reg_options |= RegexOptions::REGEX_OPTION_EXTEND;
        }

        let bytes = onig::EncodedBytes::ascii(source.as_bytes());
        match Regex::with_options_and_encoding(bytes, reg_options, onig::Syntax::ruby()) {
            Ok(regex) => Some(regex),
            Err(err) => {
                self.error(
                    DiagnosticMessage::RegexError(err.description().to_owned()),
                    range.clone(),
                );
                None
            }
        }
    }

    #[cfg(feature = "onig")]
    pub(crate) fn validate_static_regexp(&self, parts: &[Node], options: &[char], range: &Range) {
        self.build_static_regexp(parts, options, range);
    }

    #[cfg(not(feature = "onig"))]
    pub(crate) fn validate_static_regexp(
        &self,
        _parts: &[Node],
        _options: &[char],
        _range: &Range,
    ) {
    }

    #[cfg(feature = "onig")]
    pub(crate) fn static_regexp_captures(&self, node: &Node) -> Option<Vec<String>> {
        if let Node::Regexp(inner) = node {
            let Regexp {
                parts,
                options,
                expression_l,
                ..
            } = &**inner;

            let mut re_options: &[char] = &[];
            if let Some(options) = options {
                if let Node::RegOpt(inner) = options {
                    re_options = &inner.options;
                }
            };
            let regex = self.build_static_regexp(parts, re_options, expression_l)?;

            let mut result: Vec<String> = vec![];

            regex.foreach_name(|name, _| {
                result.push(name.to_owned());
                true
            });

            return Some(result);
        }
        None
    }

    #[cfg(not(feature = "onig"))]
    pub(crate) fn static_regexp_captures(&self, _node: &Node) -> Option<Vec<String>> {
        None
    }

    pub(crate) fn loc(&self, token: &Token) -> Range {
        Range::new(token.loc.begin, token.loc.end)
    }

    pub(crate) fn maybe_loc(&self, token: &Option<Token>) -> Option<Range> {
        token.as_ref().map(|t| self.loc(t))
    }

    pub(crate) fn collection_map(
        &self,
        begin_t: &Option<Token>,
        parts: &[Node],
        end_t: &Option<Token>,
    ) -> (Option<Range>, Option<Range>, Range) {
        let begin_l = self.maybe_loc(begin_t);
        let end_l = self.maybe_loc(end_t);

        let expr_l = merge_maybe_locs(vec![
            begin_l.clone(),
            collection_expr(&parts),
            end_l.clone(),
        ])
        .unwrap_or_else(|| {
            unreachable!("empty collection without begin_t/end_t, can't build source map")
        });

        (begin_l, end_l, expr_l)
    }

    pub(crate) fn string_map(
        &self,
        begin_t: &Option<Token>,
        parts: &[Node],
        end_t: &Option<Token>,
    ) -> StringMap {
        if let Some(begin_t) = begin_t {
            if clone_value(&begin_t).starts_with("<<") {
                let end_t = end_t
                    .as_ref()
                    .unwrap_or_else(|| unreachable!("heredoc must have end_t"));
                let heredoc_body_l = collection_expr(&parts).unwrap_or_else(|| self.loc(end_t));
                let expression_l = self.loc(begin_t);
                let heredoc_end_l = self.loc(end_t);

                return StringMap::HeredocMap((heredoc_body_l, heredoc_end_l, expression_l));
            }
        }

        StringMap::CollectionMap(self.collection_map(begin_t, parts, end_t))
    }

    pub(crate) fn error(&self, message: DiagnosticMessage, range: Range) {
        self.diagnostics
            .emit(Diagnostic::new(ErrorLevel::Error, message, range))
    }

    pub(crate) fn warn(&self, message: DiagnosticMessage, range: Range) {
        self.diagnostics
            .emit(Diagnostic::new(ErrorLevel::Warning, message, range))
    }

    pub(crate) fn value_expr(&self, node: &Node) -> Result<(), ()> {
        if let Some(void_node) = self.void_value(node) {
            self.error(
                DiagnosticMessage::VoidValueExpression,
                void_node.expression().clone(),
            );
            Err(())
        } else {
            Ok(())
        }
    }

    fn void_value<'a>(&self, node: &'a Node) -> Option<&'a Node> {
        let check_stmts = |statements: &'a Vec<Node>| {
            if let Some(last_stmt) = statements.last() {
                self.void_value(last_stmt)
            } else {
                None
            }
        };

        let check_condition = |if_true: &'a Node, if_false: &'a Node| {
            if self.void_value(if_true).is_some() && self.void_value(if_false).is_some() {
                Some(if_true)
            } else {
                None
            }
        };

        let check_maybe_condition =
            |if_true: &'a Option<Node>, if_false: &'a Option<Node>| match (if_true, if_false) {
                (None, None) | (None, Some(_)) | (Some(_), None) => None,
                (Some(if_true), Some(if_false)) => check_condition(if_true, if_false),
            };

        match node {
            Node::Return(_) | Node::Break(_) | Node::Next(_) | Node::Redo(_) | Node::Retry(_) => {
                Some(node)
            }
            Node::InMatch(inner) => self.void_value(&inner.value),
            Node::Begin(inner) => check_stmts(&inner.statements),
            Node::KwBegin(inner) => check_stmts(&inner.statements),
            Node::If(inner) => check_maybe_condition(&inner.if_true, &inner.if_false),
            Node::IfMod(inner) => check_maybe_condition(&inner.if_true, &inner.if_false),
            Node::IfTernary(inner) => check_condition(&inner.if_true, &inner.if_false),
            Node::And(inner) => self.void_value(&inner.lhs),
            Node::Or(inner) => self.void_value(&inner.lhs),
            _ => None,
        }
    }
}

pub(crate) fn maybe_node_expr(node: &Option<&Node>) -> Option<Range> {
    node.map(|node| node.expression().clone())
}

pub(crate) fn collection_expr(nodes: &[Node]) -> Option<Range> {
    join_maybe_exprs(&nodes.first(), &nodes.last())
}

pub(crate) fn merge_maybe_locs(locs: Vec<Option<Range>>) -> Option<Range> {
    let mut result: Option<Range> = None;
    for loc in locs {
        result = join_maybe_locs(&result, &loc)
    }
    result
}

pub(crate) fn value(token: Token) -> String {
    token.into_string_lossy()
}

pub(crate) fn clone_value(token: &Token) -> String {
    token.to_string_lossy()
}

pub(crate) fn maybe_value(token: Option<Token>) -> Option<String> {
    token.map(value)
}

pub(crate) fn join_exprs(lhs: &Node, rhs: &Node) -> Range {
    lhs.expression().join(rhs.expression())
}

pub(crate) fn join_maybe_exprs(lhs: &Option<&Node>, rhs: &Option<&Node>) -> Option<Range> {
    join_maybe_locs(&maybe_node_expr(&lhs), &maybe_node_expr(&rhs))
}

pub(crate) fn join_maybe_locs(lhs: &Option<Range>, rhs: &Option<Range>) -> Option<Range> {
    match (lhs, rhs) {
        (None, None) => None,
        (None, Some(rhs)) => Some(rhs.clone()),
        (Some(lhs), None) => Some(lhs.clone()),
        (Some(lhs), Some(rhs)) => Some(lhs.join(&rhs)),
    }
}

pub(crate) enum StringMap {
    CollectionMap((Option<Range>, Option<Range>, Range)),
    HeredocMap((Range, Range, Range)),
}

fn first<T>(vec: Vec<T>) -> T {
    vec.into_iter().next().expect("expected vec to have 1 item")
}
