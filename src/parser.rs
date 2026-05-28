use crate::types::{Expr, Value, TokenData, Param, ValueType, OpaqueValue};
use crate::lexer::{Token};
use std::collections::HashMap;

pub type MacroResolver = Box<dyn Fn(String) -> Result<Expr, String>>;

pub struct Parser {
    tokens: Vec<(Token, TokenData)>,
    pos: usize,
    filename: String,
    macro_resolver: MacroResolver,
}

impl Parser {
    pub fn new(tokens: Vec<(Token, TokenData)>, filename: String, macro_resolver: MacroResolver) -> Self {
        Self {
            tokens,
            pos: 0,
            filename,
            macro_resolver,
        }
    }

    pub fn parse(&mut self) -> Result<Expr, String> {
        self.skip_newlines();
        let mut stmts = vec![];

        // 1. Consume Macro Includes
        while !self.is_eof() && matches!(self.peek(), Token::At) {
            stmts.push(self.parse_include()?);
            self.skip_newlines();
        }

        if self.is_eof() {
            return Err("Syntax Error: Script is empty.".into());
        }

        // 2. Parse exactly ONE TaskDef (FuncDef or Block)
        let main_task = if matches!(self.peek(), Token::LParen) && self.is_func_def_start() {
            self.parse_func_def()?
        } else if matches!(self.peek(), Token::LBrace) {
            self.parse_block()?
        } else {
            return Err("Syntax Error: Expected main task definition (a closure or a block).".into());
        };
        stmts.push(main_task);

        // 3. Assert EOF
        self.skip_newlines();
        if !self.is_eof() {
            return Err("Syntax Error: Unexpected code outside of main task. A Hank script must contain exactly one Task definition.".into());
        }

        if stmts.len() == 1 {
            return Ok(stmts.remove(0));
        }
        let td_root = match &stmts[0] {
            Expr::Block(_, td) | Expr::Assign(_, _, td) | Expr::Literal(_, td) | 
            Expr::Ident(_, _, td) | Expr::Field(_, _, td) | Expr::FuncDef(_, _, td) | 
            Expr::FuncCall(_, _, td) | Expr::UnOp(_, _, td) | Expr::Object(_, td) | 
            Expr::Array(_, td) | Expr::FlowControl { token: td, .. } => td.clone(),
        };
        Ok(Expr::Block(stmts, td_root))
    }

    fn parse_statement(&mut self) -> Result<Expr, String> {
        self.skip_newlines();
        match self.peek() {
            Token::Question => self.parse_flow_control(),
            Token::Caret => self.parse_return(),
            Token::At => self.parse_include(),
            _ => self.parse_expression(),
        }
    }

    fn parse_flow_control(&mut self) -> Result<Expr, String> {
        let td = self.consume(Token::Question)?;
        self.consume(Token::LParen)?;
        let condition = self.parse_expression()?;
        self.consume(Token::RParen)?;
        
        let success = self.parse_block()?;
        
        let mut fallback = None;
        let mut rescue = None;
        let mut catch_var = None;
        
        let saved_pos = self.pos;
        self.skip_newlines();
        if matches!(self.peek(), Token::Colon) {
            self.consume(Token::Colon)?;
            fallback = Some(Box::new(self.parse_block()?));
        } else if matches!(self.peek(), Token::Rescue) {
            // handled below
            self.pos = saved_pos;
        } else {
            self.pos = saved_pos;
        }

        self.skip_newlines();
        if matches!(self.peek(), Token::Rescue) {
            self.consume(Token::Rescue)?;
            self.consume(Token::LParen)?;
            catch_var = Some(self.consume_identifier()?);
            self.consume(Token::RParen)?;
            rescue = Some(Box::new(self.parse_block()?));
        }

        Ok(Expr::FlowControl {
            condition: Box::new(condition),
            success: Box::new(success),
            fallback,
            rescue,
            catch_var,
            token: td,
        })
    }

    fn parse_expression(&mut self) -> Result<Expr, String> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expr, String> {
        let expr = self.parse_primary()?;

        if matches!(self.peek(), Token::Assign) {
            if let Expr::Ident(name, false, td) = expr {
                self.consume(Token::Assign)?;
                let value = self.parse_expression()?;
                return Ok(Expr::Assign(name, Box::new(value), td));
            } else {
                return Err(self.error("Invalid assignment target"));
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let (token, td) = self.tokens[self.pos].clone();
        let expr = match token {
            Token::At => self.parse_include()?,
            Token::LParen => {
                if self.is_func_def_start() {
                    self.parse_func_def()?
                } else {
                    self.pos += 1;
                    let e = self.parse_expression()?;
                    self.consume(Token::RParen)?;
                    e
                }
            },
            Token::LBrace => {
                if self.is_object_literal() {
                    self.parse_object_literal()?
                } else {
                    self.parse_block()?
                }
            },
            Token::LBracket => self.parse_array_literal()?,
            Token::Not => {
                self.pos += 1;
                Expr::UnOp("!".into(), Box::new(self.parse_primary()?), td)
            },
            Token::Hash => {
                self.pos += 1;
                Expr::Ident(self.consume_identifier()?, true, td)
            },
            Token::Identifier(id) => {
                self.pos += 1;
                Expr::Ident(id, false, td)
            },
            Token::String(s) => {
                self.pos += 1;
                Expr::Literal(Value::String(s), td)
            },
            Token::Number(n) => {
                self.pos += 1;
                Expr::Literal(Value::Number(n), td)
            },
            Token::Caret => self.parse_return()?,
            _ => return Err(self.error(&format!("Unexpected token: {:?}", token))),
        };
        
        self.finish_primary(expr)
    }

    fn finish_primary(&mut self, mut expr: Expr) -> Result<Expr, String> {
        loop {
            let (token, td) = self.peek_full();
            match token {
                Token::Dot => {
                    self.pos += 1;
                    expr = Expr::Field(Box::new(expr), self.consume_identifier()?, td);
                },
                Token::LParen => {
                    expr = Expr::FuncCall(Box::new(expr), self.parse_arg_list()?, td);
                },
                _ => break,
            }
        }
        Ok(expr)
    }

    fn is_func_def_start(&self) -> bool {
        let mut p = self.pos + 1;
        let mut depth = 1;
        while p < self.tokens.len() && depth > 0 {
            match &self.tokens[p].0 {
                Token::LParen => depth += 1,
                Token::RParen => depth -= 1,
                _ => {}
            }
            p += 1;
        }
        while p < self.tokens.len() && matches!(self.tokens[p].0, Token::Newline) { p += 1; }
        p < self.tokens.len() && matches!(self.tokens[p].0, Token::LBrace)
    }

    fn parse_func_def(&mut self) -> Result<Expr, String> {
        let td = self.peek_td();
        self.consume(Token::LParen)?;
        let mut params = vec![];
        if !matches!(self.peek(), Token::RParen) {
            params.push(self.parse_param()?);
            while matches!(self.peek(), Token::Comma) {
                self.consume(Token::Comma)?;
                params.push(self.parse_param()?);
            }
        }
        self.consume(Token::RParen)?;
        let body = self.parse_block()?;
        Ok(Expr::FuncDef(params, Box::new(body), td))
    }

    fn parse_param(&mut self) -> Result<Param, String> {
        let mut is_optional = false;
        if matches!(self.peek(), Token::Question) {
            self.consume(Token::Question)?;
            is_optional = true;
        }
        let name = self.consume_identifier()?;
        let mut default_value = None;
        if matches!(self.peek(), Token::Assign) {
            self.consume(Token::Assign)?;
            default_value = Some(Box::new(self.parse_expression()?));
            is_optional = true;
        }
        Ok(Param { name, is_optional, default_value })
    }

    fn parse_block(&mut self) -> Result<Expr, String> {
        let td = self.consume(Token::LBrace)?;
        let mut stmts = vec![];
        while !matches!(self.peek(), Token::RBrace) && !self.is_eof() {
            self.skip_newlines();
            if matches!(self.peek(), Token::RBrace) { break; }
            stmts.push(self.parse_statement()?);
        }
        self.consume(Token::RBrace)?;
        Ok(Expr::Block(stmts, td))
    }

    fn is_object_literal(&self) -> bool {
        let mut p = self.pos + 1;
        while p < self.tokens.len() && matches!(self.tokens[p].0, Token::Newline) { p += 1; }
        if p >= self.tokens.len() { return false; }
        if matches!(self.tokens[p].0, Token::RBrace) { return true; }
        if let Token::Identifier(_) = &self.tokens[p].0 {
            let mut next = p + 1;
            while next < self.tokens.len() && matches!(self.tokens[next].0, Token::Newline) { next += 1; }
            return next < self.tokens.len() && matches!(self.tokens[next].0, Token::Colon);
        }
        false
    }

    fn parse_object_literal(&mut self) -> Result<Expr, String> {
        let td = self.consume(Token::LBrace)?;
        let mut fields = HashMap::new();
        while !matches!(self.peek(), Token::RBrace) && !self.is_eof() {
            self.skip_newlines();
            if matches!(self.peek(), Token::RBrace) { break; }
            let key = self.consume_identifier()?;
            self.consume(Token::Colon)?;
            fields.insert(key, self.parse_expression()?);
            if matches!(self.peek(), Token::Comma) { self.consume(Token::Comma)?; }
        }
        self.consume(Token::RBrace)?;
        Ok(Expr::Object(fields, td))
    }

    fn parse_array_literal(&mut self) -> Result<Expr, String> {
        let td = self.consume(Token::LBracket)?;
        let mut items = vec![];
        while !matches!(self.peek(), Token::RBracket) && !self.is_eof() {
            self.skip_newlines();
            if matches!(self.peek(), Token::RBracket) { break; }
            items.push(self.parse_expression()?);
            if matches!(self.peek(), Token::Comma) { self.consume(Token::Comma)?; }
        }
        self.consume(Token::RBracket)?;
        Ok(Expr::Array(items, td))
    }

    fn parse_arg_list(&mut self) -> Result<Vec<Expr>, String> {
        self.consume(Token::LParen)?;
        let mut args = vec![];
        self.skip_newlines();
        if !matches!(self.peek(), Token::RParen) {
            args.push(self.parse_expression()?);
            loop {
                self.skip_newlines();
                if matches!(self.peek(), Token::Comma) {
                    self.consume(Token::Comma)?;
                    self.skip_newlines();
                    args.push(self.parse_expression()?);
                } else { break; }
            }
        }
        self.skip_newlines();
        self.consume(Token::RParen)?;
        Ok(args)
    }

    fn parse_return(&mut self) -> Result<Expr, String> {
        let td = self.consume(Token::Caret)?;
        let mut val = Expr::Literal(Value::Void, td.clone());
        if !self.is_eof() && !matches!(self.peek(), Token::Newline | Token::RBrace | Token::RBracket | Token::Comma | Token::RParen) {
            val = self.parse_expression()?;
        }
        Ok(Expr::UnOp("^".into(), Box::new(val), td))
    }

    fn parse_include(&mut self) -> Result<Expr, String> {
        let td = self.consume(Token::At)?;
        let raw_path = match self.peek() {
            Token::String(s) => { self.pos += 1; s },
            _ => return Err(self.error("Syntax Error: The '@' macro strictly requires a string literal path (e.g., @ \"utils\"). Identifier shorthand is not allowed.")),
        };

        let task_ast = (self.macro_resolver)(raw_path.clone())?;
        let task_name = std::path::Path::new(&raw_path).file_stem().unwrap().to_string_lossy().to_string();

        Ok(Expr::Assign(task_name, Box::new(task_ast), td))
    }

    fn consume_identifier(&mut self) -> Result<String, String> {
        match self.peek() {
            Token::Identifier(id) => {
                self.pos += 1;
                Ok(id)
            },
            _ => Err(self.error(&format!("Expected identifier, found {:?}", self.peek()))),
        }
    }

    fn consume(&mut self, token: Token) -> Result<TokenData, String> {
        let (t, td) = self.tokens[self.pos].clone();
        if t == token {
            self.pos += 1;
            Ok(td)
        } else {
            Err(self.error(&format!("Expected {:?}, found {:?}", token, t)))
        }
    }

    fn peek(&self) -> Token {
        self.tokens[self.pos].0.clone()
    }

    fn peek_td(&self) -> TokenData {
        self.tokens[self.pos].1.clone()
    }

    fn peek_full(&self) -> (Token, TokenData) {
        self.tokens[self.pos].clone()
    }

    fn skip_newlines(&mut self) {
        while self.pos < self.tokens.len() && matches!(self.tokens[self.pos].0, Token::Newline) {
            self.pos += 1;
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len() || matches!(self.tokens[self.pos].0, Token::EOF)
    }

    fn error(&self, msg: &str) -> String {
        let td = self.peek_td();
        format!("ERROR: {} in {} at\n\t{}:\t{}", msg, self.filename, td.line, td.line_text)
    }
}
