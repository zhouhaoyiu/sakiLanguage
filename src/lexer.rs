use crate::token::Token;

pub struct Lexer {
    source: Vec<char>,
    pos: usize,
    current: Option<char>,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        let chars: Vec<char> = source.chars().collect();
        let current = chars.first().copied();
        Lexer {
            source: chars,
            pos: 0,
            current,
        }
    }

    fn advance(&mut self) {
        self.pos += 1;
        self.current = self.source.get(self.pos).copied();
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn number(&mut self) -> Token {
        let mut num_str = String::new();
        while let Some(ch) = self.current {
            if ch.is_ascii_digit() {
                num_str.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        Token::Int(num_str.parse().unwrap())
    }

    fn string(&mut self) -> Result<Token, String> {
        self.advance(); // 跳过开头 "
        let mut s = String::new();
        while let Some(ch) = self.current {
            if ch == '"' {
                self.advance(); // 跳过结尾 "
                return Ok(Token::Str(s));
            } else {
                s.push(ch);
                self.advance();
            }
        }
        Err("未闭合的字符串".to_string())
    }

    fn identifier(&mut self) -> Token {
        let mut ident = String::new();
        while let Some(ch) = self.current {
            if ch.is_alphanumeric() || ch == '_' {
                ident.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        match ident.as_str() {
            "ika" => Token::Ika,
            "fn" => Token::Fn,
            "return" => Token::Return,
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            "true" => Token::True,
            "false" => Token::False,
            "and" => Token::And,
            "or" => Token::Or,
            "not" => Token::Not,
            _ => Token::Ident(ident),
        }
    }

    pub fn next_token(&mut self) -> Result<Token, String> {
        self.skip_whitespace();
        match self.current {
            None => Ok(Token::Eof),
            Some(ch) => match ch {
                '+' => {
                    self.advance();
                    Ok(Token::Plus)
                }
                '-' => {
                    self.advance();
                    Ok(Token::Minus)
                }
                '*' => {
                    self.advance();
                    Ok(Token::Star)
                }
                '/' => {
                    self.advance();
                    Ok(Token::Slash)
                }
                '(' => {
                    self.advance();
                    Ok(Token::LParen)
                }
                ')' => {
                    self.advance();
                    Ok(Token::RParen)
                }
                '{' => {
                    self.advance();
                    Ok(Token::LBrace)
                }
                '}' => {
                    self.advance();
                    Ok(Token::RBrace)
                }
                ',' => {
                    self.advance();
                    Ok(Token::Comma)
                }
                ';' => {
                    self.advance();
                    Ok(Token::Semicolon)
                }
                '=' => {
                    self.advance();
                    if self.current == Some('=') {
                        self.advance();
                        Ok(Token::EqEq)
                    } else {
                        Ok(Token::Eq)
                    }
                }
                '!' => {
                    self.advance();
                    if self.current == Some('=') {
                        self.advance();
                        Ok(Token::Neq)
                    } else {
                        Err("意外的字符 '!'".to_string())
                    }
                }
                '<' => {
                    self.advance();
                    if self.current == Some('=') {
                        self.advance();
                        Ok(Token::Le)
                    } else {
                        Ok(Token::Lt)
                    }
                }
                '>' => {
                    self.advance();
                    if self.current == Some('=') {
                        self.advance();
                        Ok(Token::Ge)
                    } else {
                        Ok(Token::Gt)
                    }
                }
                '"' => self.string(),
                ch if ch.is_ascii_digit() => Ok(self.number()),
                ch if ch.is_alphabetic() || ch == '_' => Ok(self.identifier()),
                _ => Err(format!("意外的字符 '{}'", ch)),
            },
        }
    }
}

pub struct TokenStream {
    lexer: Lexer,
    peeked: Option<Token>,
}

impl TokenStream {
    pub fn new(source: &str) -> Self {
        TokenStream {
            lexer: Lexer::new(source),
            peeked: None,
        }
    }

    pub fn peek(&mut self) -> Result<&Token, String> {
        if self.peeked.is_none() {
            self.peeked = Some(self.lexer.next_token()?);
        }
        Ok(self.peeked.as_ref().unwrap())
    }

    pub fn advance(&mut self) -> Result<Token, String> {
        if let Some(tok) = self.peeked.take() {
            Ok(tok)
        } else {
            self.lexer.next_token()
        }
    }

    pub fn expect(&mut self, expected: Token) -> Result<(), String> {
        let tok = self.advance()?;
        if tok == expected {
            Ok(())
        } else {
            Err(format!("期望 {:?}，但得到 {:?}", expected, tok))
        }
    }
}