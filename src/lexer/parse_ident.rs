use crate::State;
use crate::lexer::{Token, TokenType, LexState};
use crate::lexer::lex_char::LexChar;
use crate::lexer::lex_states::*;
use crate::lexer::reserved_word;

impl State {
    pub fn parser_is_identchar(&self) -> bool {
        !self.p.eofp && self.is_identchar(self.p.lex.pcur - 1, self.p.lex.pend)
    }

    pub fn tokenize_ident(&mut self, _last_state: &LexState) -> String {
        let ident = self.tok();
        self.set_yyval_name(&ident);
        ident
    }

    // This method is called is_local_id in MRI, not sure why
    pub fn is_var_name(&self, ident: &str) -> bool {
        // FIXME: unclear what it can be
        // MRI has some weird logic of comparing given ID with tLAST_OP_ID
        // and then checking & ID_SCOPE_MASK
        if let Some(first_char) = ident.chars().next() {
            return !first_char.is_uppercase()
        }
        false
    }

    pub fn parse_ident(&mut self, c: &LexChar, cmd_state: bool) -> TokenType {
        let mut c = c.clone();
        let mut result: TokenType;
        let last_state: LexState = self.p.lex.state.clone();
        let ident: String;

        loop {
            if !c.is_ascii() { /* mb = ENC_CODERANGE_UNKNOWN */ }
            if self.tokadd_mbchar(&c).is_err() { return Token::END_OF_INPUT }
            c = self.nextc();

            if !self.parser_is_identchar() { break }
        }

        if (c == '!' || c == '?') && !self.peek('=') {
            result = Token::tFID;
            self.tokadd(&c);
        } else if c == '=' && self.is_lex_state_some(EXPR_FNAME) &&
                (!self.peek('~') && !self.peek('>') && (!self.peek('=') || (self.peek_n('>', 1)))) {
            result = Token::tIDENTIFIER;
            self.tokadd(&c)
        } else {
            result = Token::tCONSTANT; /* assume provisionally */
            self.pushback(&c)
        }
        self.tokfix();

        if self.is_label_possible(cmd_state) {
            if self.is_label_suffix(0) {
                self.set_lex_state(EXPR_ARG|EXPR_LABELED);
                self.nextc();
                self.set_yyval_name(&self.tok());
                return Token::tLABEL;
            }
        }
        if /* mb == ENC_CODERANGE_7BIT && */ !self.is_lex_state_some(EXPR_DOT) {
            if let Some(kw) = reserved_word(&self.tok()) {
                let state: LexState = self.p.lex.state.clone();
                if state.is_some(EXPR_FNAME) {
                    self.set_lex_state(EXPR_ENDFN);
                    self.set_yyval_name(&self.tok());
                    return kw.id.clone();
                }
                self.set_lex_state(kw.state);
                if self.is_lex_state_some(EXPR_BEG) {
                    self.p.command_start = true
                }
                if kw.id == Token::kDO {
                    if self.is_lambda_beginning() {
                        self.p.lex.lpar_beg = -1; /* make lambda_beginning_p() == FALSE in the body of "-> do ... end" */
                        return Token::kDO_LAMBDA
                    }
                    if self.is_cond_active() { return Token::kDO_COND }
                    if self.is_cmdarg_active() && !state.is_some(EXPR_CMDARG) {
                        return Token::kDO_BLOCK
                    }
                    return Token::kDO
                }
                if state.is_some(EXPR_BEG | EXPR_LABELED) {
                    return kw.id.clone()
                } else {
                    if kw.id != kw.modifier_id {
                        self.set_lex_state(EXPR_BEG | EXPR_LABEL)
                    }
                    return kw.modifier_id.clone()
                }
            }
        }

        if self.is_lex_state_some(EXPR_BEG_ANY | EXPR_ARG_ANY | EXPR_DOT) {
            if cmd_state {
                self.set_lex_state(EXPR_CMDARG);
            } else {
                self.set_lex_state(EXPR_ARG);
            }
        } else if self.p.lex.state.is(EXPR_FNAME) {
            self.set_lex_state(EXPR_ENDFN)
        } else {
            self.set_lex_state(EXPR_END)
        }

        ident = self.tokenize_ident(&last_state);
        if result == Token::tCONSTANT && self.is_var_name(&ident) { result = Token::tIDENTIFIER }
        if !last_state.is_some(EXPR_DOT|EXPR_FNAME) &&
            result == Token::tIDENTIFIER && /* not EXPR_FNAME, not attrasgn */
            self.is_lvar_defined(&ident) {
            self.set_lex_state(EXPR_END|EXPR_LABEL);
        }

        result
    }

}