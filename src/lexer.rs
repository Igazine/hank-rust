use crate::types::TokenData;

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Identifier(String),
    Number(f64),
    String(String),
    
    Assign,    // =
    Question,  // ?
    Colon,     // :
    Rescue,    // ~
    At,        // @
    Hash,      // #
    Not,       // !
    Caret,     // ^
    Dot,       // .
    Comma,     // ,
    
    LParen,    // (
    RParen,    // )
    LBrace,    // {
    RBrace,    // }
    LBracket,  // [
    RBracket,  // ]
    
    Newline,
    EOF,
}

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
    line: usize,
    line_start: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
            line: 1,
            line_start: 0,
        }
    }

    pub fn tokenize(&mut self) -> Vec<(Token, TokenData)> {
        let mut tokens = vec![];

        while self.pos < self.input.len() {
            let char = self.input[self.pos];

            if char.is_whitespace() {
                if char == '\n' {
                    tokens.push((Token::Newline, self.td()));
                    self.line += 1;
                    self.pos += 1;
                    self.line_start = self.pos;
                } else {
                    self.pos += 1;
                }
                continue;
            }

            if char == '/' && self.peek() == Some('/') {
                self.skip_comment();
                continue;
            }

            if char == '-' && self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                tokens.push((self.read_number(), self.td()));
                continue;
            }

            if char.is_ascii_digit() {
                tokens.push((self.read_number(), self.td()));
                continue;
            }

            if char.is_alphabetic() || char == '_' {
                tokens.push((self.read_identifier(), self.td()));
                continue;
            }

            if char == '"' || char == '\'' {
                tokens.push((self.read_string(char), self.td()));
                continue;
            }

            let token = match char {
                '=' => Token::Assign,
                '?' => Token::Question,
                ':' => Token::Colon,
                '~' => Token::Rescue,
                '@' => Token::At,
                '#' => Token::Hash,
                '!' => Token::Not,
                '^' => Token::Caret,
                '.' => Token::Dot,
                ',' => Token::Comma,
                '(' => Token::LParen,
                ')' => Token::RParen,
                '{' => Token::LBrace,
                '}' => Token::RBrace,
                '[' => Token::LBracket,
                ']' => Token::RBracket,
                _ => {
                    // Skip unknown characters or handle error
                    self.pos += 1;
                    continue;
                }
            };

            tokens.push((token, self.td()));
            self.pos += 1;
        }

        tokens.push((Token::EOF, self.td()));
        tokens
    }

    fn skip_comment(&mut self) {
        while self.pos < self.input.len() && self.input[self.pos] != '\n' {
            self.pos += 1;
        }
    }

    fn read_number(&mut self) -> Token {
        let start = self.pos;
        if self.input[self.pos] == '-' {
            self.pos += 1;
        }
        while self.pos < self.input.len() && (self.input[self.pos].is_ascii_digit() || self.input[self.pos] == '.') {
            self.pos += 1;
        }
        let s: String = self.input[start..self.pos].iter().collect();
        Token::Number(s.parse().unwrap_or(0.0))
    }

    fn read_identifier(&mut self) -> Token {
        let start = self.pos;
        self.pos += 1;
        while self.pos < self.input.len() && (self.input[self.pos].is_alphanumeric() || self.input[self.pos] == '_') {
            self.pos += 1;
        }
        let s: String = self.input[start..self.pos].iter().collect();
        Token::Identifier(s)
    }

    fn read_string(&mut self, quote: char) -> Token {
        self.pos += 1; // skip quote
        let mut val = String::new();
        while self.pos < self.input.len() && self.input[self.pos] != quote {
            if self.input[self.pos] == '\\' {
                self.pos += 1;
                if self.pos >= self.input.len() { break; }
                match self.input[self.pos] {
                    'n' => val.push('\n'),
                    't' => val.push('\t'),
                    c => val.push(c),
                }
            } else {
                val.push(self.input[self.pos]);
            }
            self.pos += 1;
        }
        self.pos += 1; // skip quote
        Token::String(val)
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos + 1).cloned()
    }

    fn td(&self) -> TokenData {
        TokenData {
            line: self.line,
            line_text: self.get_current_line_text(),
        }
    }

    fn get_current_line_text(&self) -> String {
        let mut end = self.pos;
        while end < self.input.len() && self.input[end] != '\n' {
            end += 1;
        }
        self.input[self.line_start..end].iter().collect()
    }
}
