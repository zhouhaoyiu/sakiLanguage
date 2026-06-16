use crate::token::Token;

/// 将字符流转换为 Token 流的词法分析器。
pub struct Lexer {
    /// 源码字符序列。
    source: Vec<char>,
    /// 当前读取位置。
    pos: usize,
    /// 当前字符。
    current: Option<char>,
}

impl Lexer {
    /// 从源码文本创建词法分析器。
    pub fn new(source: &str) -> Self {
        let chars: Vec<char> = source.chars().collect();
        let current = chars.first().copied();
        Lexer {
            source: chars,
            pos: 0,
            current,
        }
    }

    /// 前进到下一个字符。
    fn advance(&mut self) {
        self.pos += 1;
        self.current = self.source.get(self.pos).copied();
    }

    /// 跳过空白字符。
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// 跳过单行注释。
    fn skip_line_comment(&mut self) {
        while let Some(ch) = self.current {
            self.advance();
            if ch == '\n' {
                break;
            }
        }
    }

    /// 读取整数 token。
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

    /// 读取字符串 token（支持双引号与单引号）。
    fn string_with_quote(&mut self, quote: char) -> Result<Token, String> {
        self.advance(); // 跳过开头引号
        let mut s = String::new();
        while let Some(ch) = self.current {
            if ch == quote {
                self.advance(); // 跳过结尾引号
                return Ok(Token::Str(s));
            }
            s.push(ch);
            self.advance();
        }
        Err("未闭合的字符串".to_string())
    }

    /// 读取标识符或关键字 token。
    fn identifier(&mut self) -> Token {
        let mut ident = String::new();
        while let Some(ch) = self.current {
            if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                ident.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        match ident.as_str() {
            "ika" => Token::Ika,
            "fn" => Token::Fn,
            "function" => Token::Function,
            "let" => Token::Let,
            "var" => Token::Var,
            "const" => Token::Const,
            "return" => Token::Return,
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            "break" => Token::Break,
            "continue" => Token::Continue,
            "null" => Token::Null,
            "undefined" => Token::Undefined,
            "true" => Token::True,
            "false" => Token::False,
            "and" => Token::And,
            "or" => Token::Or,
            "not" => Token::Not,
            _ => Token::Ident(ident),
        }
    }

    /// 获取下一个 token。
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
                    if self.current == Some('/') {
                        self.skip_line_comment();
                        self.next_token()
                    } else {
                        Ok(Token::Slash)
                    }
                }
                '%' => {
                    self.advance();
                    Ok(Token::Percent)
                }
                '(' => {
                    self.advance();
                    Ok(Token::LParen)
                }
                ')' => {
                    self.advance();
                    Ok(Token::RParen)
                }
                '[' => {
                    self.advance();
                    Ok(Token::LBracket)
                }
                ']' => {
                    self.advance();
                    Ok(Token::RBracket)
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
                        if self.current == Some('=') {
                            self.advance();
                            Ok(Token::EqEqEq)
                        } else {
                            Ok(Token::EqEq)
                        }
                    } else {
                        Ok(Token::Eq)
                    }
                }
                '!' => {
                    self.advance();
                    if self.current == Some('=') {
                        self.advance();
                        if self.current == Some('=') {
                            self.advance();
                            Ok(Token::NeqEq)
                        } else {
                            Ok(Token::Neq)
                        }
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
                '&' => {
                    self.advance();
                    if self.current == Some('&') {
                        self.advance();
                        Ok(Token::AndAnd)
                    } else {
                        Err("意外的字符 '&'".to_string())
                    }
                }
                '|' => {
                    self.advance();
                    if self.current == Some('|') {
                        self.advance();
                        Ok(Token::OrOr)
                    } else {
                        Err("意外的字符 '|'".to_string())
                    }
                }
                '"' => self.string_with_quote('"'),
                '\'' => self.string_with_quote('\''),
                ch if ch.is_ascii_digit() => Ok(self.number()),
                ch if ch.is_alphabetic() || ch == '_' || ch == '$' => Ok(self.identifier()),
                _ => Err(format!("意外的字符 '{}'", ch)),
            },
        }
    }
}

/// 支持预读的 Token 流。
pub struct TokenStream {
    /// 底层词法分析器。
    lexer: Lexer,
    /// 预读的 token。
    peeked: Option<Token>,
}

impl TokenStream {
    /// 从源码创建 Token 流。
    pub fn new(source: &str) -> Self {
        TokenStream {
            lexer: Lexer::new(source),
            peeked: None,
        }
    }

    /// 查看下一个 token，但不前进。
    pub fn peek(&mut self) -> Result<&Token, String> {
        if self.peeked.is_none() {
            self.peeked = Some(self.lexer.next_token()?);
        }
        Ok(self.peeked.as_ref().unwrap())
    }

    /// 获取下一个 token 并前进。
    pub fn advance(&mut self) -> Result<Token, String> {
        if let Some(tok) = self.peeked.take() {
            Ok(tok)
        } else {
            self.lexer.next_token()
        }
    }

    /// 断言下一个 token 类型。
    pub fn expect(&mut self, expected: Token) -> Result<(), String> {
        let tok = self.advance()?;
        if tok == expected {
            Ok(())
        } else {
            Err(format!("期望 {:?}，但得到 {:?}", expected, tok))
        }
    }
}
