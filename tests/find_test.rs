use ruby_parser::traverse::Find;
use ruby_parser::{Lexer, Parser};

fn find(src: &str, pattern: Vec<&str>) -> Option<String> {
    let lexer = Lexer::new(&src.as_bytes().to_vec(), None).ok()?;
    let mut parser = Parser::new(lexer);
    parser.set_debug(false);

    let ast = parser.do_parse()?;
    let node = Find::run(pattern, &ast)?;
    node.expression().source(&parser.yylexer.buffer)
}

#[test]
fn it_finds() {
    let src = "[1,2,3].each { |a| puts a + 1; 42 }";
    let pattern = vec!["body", "stmt[0]", "arg[0]"];

    assert_eq!(Some("a + 1".to_owned()), find(src, pattern))
}

#[test]
fn it_returns_none_if_no_node() {
    let src = "[1,2,3]";
    let pattern = vec!["body"];

    assert_eq!(None, find(src, pattern))
}