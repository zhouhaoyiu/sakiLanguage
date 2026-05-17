use crate::ast::*;
use crate::token::Token;
use crate::lexer::TokenStream;   // 修正点：从 lexer 导入 TokenStream

pub struct Parser {
    tokens: TokenStream,
}

impl Parser {
    pub fn new(source: &str) -> Self {
        Parser {
            tokens: TokenStream::new(source),
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, String> {
        let mut stmts = Vec::new();
        while *self.tokens.peek()? != Token::Eof {
            stmts.push(self.parse_statement()?);
        }
        Ok(Program { statements: stmts })
    }

    fn parse_statement(&mut self) -> Result<Stmt, String> {
        match self.tokens.peek()? {
            Token::Ika => self.parse_var_decl(),
            Token::Fn => self.parse_fn_decl(),
            Token::Return => self.parse_return_stmt(),
            Token::LBrace => self.parse_block(),
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_var_decl(&mut self) -> Result<Stmt, String> {
        self.tokens.expect(Token::Ika)?;
        let name = if let Token::Ident(n) = self.tokens.advance()? {
            n
        } else {
            return Err("期望变量名".to_string());
        };
        self.tokens.expect(Token::Eq)?;
        let value = self.parse_expression()?;
        self.tokens.expect(Token::Semicolon)?;
        Ok(Stmt::VarDecl(name, value))
    }

    fn parse_fn_decl(&mut self) -> Result<Stmt, String> {
        self.tokens.expect(Token::Fn)?;
        let name = if let Token::Ident(n) = self.tokens.advance()? {
            n
        } else {
            return Err("期望函数名".to_string());
        };
        self.tokens.expect(Token::LParen)?;
        let mut params = Vec::new();
        if *self.tokens.peek()? != Token::RParen {
            loop {
                if let Token::Ident(p) = self.tokens.advance()? {
                    params.push(p);
                } else {
                    return Err("期望参数名".to_string());
                }
                if *self.tokens.peek()? == Token::Comma {
                    self.tokens.advance()?;
                } else {
                    break;
                }
            }
        }
        self.tokens.expect(Token::RParen)?;
        let body = self.parse_block()?;
        if let Stmt::Block(stmts) = body {
            Ok(Stmt::FnDecl(name, params, stmts))
        } else {
            Ok(Stmt::FnDecl(name, params, vec![body]))
        }
    }

    fn parse_return_stmt(&mut self) -> Result<Stmt, String> {
        self.tokens.expect(Token::Return)?;
        let expr = if *self.tokens.peek()? == Token::Semicolon {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.tokens.expect(Token::Semicolon)?;
        Ok(Stmt::Return(expr))
    }

    fn parse_expr_stmt(&mut self) -> Result<Stmt, String> {
        let expr = self.parse_expression()?;
        self.tokens.expect(Token::Semicolon)?;
        Ok(Stmt::ExprStmt(expr))
    }

    fn parse_block(&mut self) -> Result<Stmt, String> {
        self.tokens.expect(Token::LBrace)?;
        let mut stmts = Vec::new();
        while *self.tokens.peek()? != Token::RBrace {
            stmts.push(self.parse_statement()?);
        }
        self.tokens.expect(Token::RBrace)?;
        Ok(Stmt::Block(stmts))
    }

    fn parse_expression(&mut self) -> Result<Expr, String> {
        self.parse_and_or()
    }

    fn parse_and_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_equality()?;
        loop {
            let op = match self.tokens.peek()? {
                Token::And => BinOp::And,
                Token::Or => BinOp::Or,
                _ => break,
            };
            self.tokens.advance()?;
            let right = self.parse_equality()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_add_sub()?;
        loop {
            let op = match self.tokens.peek()? {
                Token::EqEq => BinOp::EqEq,
                Token::Neq => BinOp::Neq,
                Token::Lt => BinOp::Lt,
                Token::Gt => BinOp::Gt,
                Token::Le => BinOp::Le,
                Token::Ge => BinOp::Ge,
                _ => break,
            };
            self.tokens.advance()?;
            let right = self.parse_add_sub()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_add_sub(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_mul_div()?;
        loop {
            let op = match self.tokens.peek()? {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.tokens.advance()?;
            let right = self.parse_mul_div()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_mul_div(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.tokens.peek()? {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                _ => break,
            };
            self.tokens.advance()?;
            let right = self.parse_unary()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.tokens.peek()? {
            Token::Minus => {
                self.tokens.advance()?;
                let expr = self.parse_unary()?;
                Ok(Expr::Unary(UnaryOp::Minus, Box::new(expr)))
            }
            Token::Not => {
                self.tokens.advance()?;
                let expr = self.parse_unary()?;
                Ok(Expr::Unary(UnaryOp::Not, Box::new(expr)))
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.tokens.advance()? {
            Token::Int(n) => Ok(Expr::IntLit(n)),
            Token::Str(s) => Ok(Expr::StrLit(s)),
            Token::True => Ok(Expr::BoolLit(true)),
            Token::False => Ok(Expr::BoolLit(false)),
            Token::Ident(name) => {
                if *self.tokens.peek()? == Token::LParen {
                    self.tokens.expect(Token::LParen)?;
                    let mut args = Vec::new();
                    if *self.tokens.peek()? != Token::RParen {
                        loop {
                            args.push(self.parse_expression()?);
                            if *self.tokens.peek()? == Token::Comma {
                                self.tokens.advance()?;
                            } else {
                                break;
                            }
                        }
                    }
                    self.tokens.expect(Token::RParen)?;
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            Token::LParen => {
                let expr = self.parse_expression()?;
                self.tokens.expect(Token::RParen)?;
                Ok(expr)
            }
            other => Err(format!("意外的 token: {:?}", other)),
        }
    }
}