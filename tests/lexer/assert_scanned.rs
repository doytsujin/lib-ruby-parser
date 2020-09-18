#[macro_export]
macro_rules! setup_lexer {
    () => {
        {
            use ruby_parser::lexer::State;
            State::new("")
        }
    };
}

#[macro_export]
macro_rules! set_lex_state {
    ($state:ident, $lex_state:ident) => {
        {
            use ruby_parser::lexer::lex_states::*;
            $state.set_lex_state($lex_state);
        }
    };
}

#[macro_export]
macro_rules! assert_scanned {
    ($state:expr, $input:expr, $(:$token_type:tt, $value:expr, [$begin:expr, $end:expr]),*) => {
        {
            use ruby_parser::lexer::{Token, TokenType};
            $state.set_source($input);
            let actual_tokens = $state.tokenize_until_eof();

            let token_types: Vec<TokenType>     = vec![$(Token::$token_type),*];
            let token_values: Vec<&str>         = vec![$($value),*];
            let begins: Vec<usize>              = vec![$($begin),*];
            let ends: Vec<usize>                = vec![$($end),*];

            let mut expected_tokens: Vec<Token> = vec![];

            for (idx, token_type) in token_types.iter().enumerate() {
                let token_type = token_type.clone();
                let token_value = token_values[idx].to_owned();
                let begin = begins[idx];
                let end = ends[idx];

                let token = token_type(token_value, begin, end);
                expected_tokens.push(token);
            }

            assert_eq!(actual_tokens, expected_tokens);
        }
    };
}