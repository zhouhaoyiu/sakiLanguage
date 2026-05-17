use crate::ast::*;
use crate::token::Token;
use crate::lexer::TokenStream;
/// 递归下降语法分析器。
pub struct Parser {
    /// 词法 Token 流。
    tokens: TokenStream,
}

impl Parser {
    /// 从源码创建解析器。
    pub fn new(source: &str) -> Self {
        Parser {
            tokens: TokenStream::new(source),
        }
    }

    /// 解析为程序根节点。
    pub fn parse_program(&mut self) -> Result<Program, String> {
        // 收集顶层语句。
        let mut stmts = Vec::new();
        while *self.tokens.peek()? != Token::Eof {
            stmts.push(self.parse_statement()?);
        }
        Ok(Program { statements: stmts })
    }

    /// 解析单条语句。
    fn parse_statement(&mut self) -> Result<Stmt, String> {
        match self.tokens.peek()? {
            Token::Ika => self.parse_var_decl(),
            Token::Fn => self.parse_fn_decl(),
            Token::Return => self.parse_return_stmt(),
            Token::LBrace => self.parse_block(),
            _ => self.parse_expr_stmt(),
        }
    }

    /// 解析变量声明语句。
    fn parse_var_decl(&mut self) -> Result<Stmt, String> {
        self.tokens.expect(Token::Ika)?;
        // 读取变量名。
        let name = if let Token::Ident(n) = self.tokens.advance()? {
            n
        } else {
            return Err("期望变量名".to_string());
        };
        self.tokens.expect(Token::Eq)?;
        // 解析初始化表达式。
        let value = self.parse_expression()?;
        self.tokens.expect(Token::Semicolon)?;
        Ok(Stmt::VarDecl(name, value))
    }

    /// 解析函数声明语句。
    fn parse_fn_decl(&mut self) -> Result<Stmt, String> {
        self.tokens.expect(Token::Fn)?;
        // 读取函数名。
        let name = if let Token::Ident(n) = self.tokens.advance()? {
            n
        } else {
            return Err("期望函数名".to_string());
        };
        self.tokens.expect(Token::LParen)?;
        // 解析形参列表。
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
        // 解析函数体语句块。
        let body = self.parse_block()?;
        if let Stmt::Block(stmts) = body {
            Ok(Stmt::FnDecl(name, params, stmts))
        } else {
            Ok(Stmt::FnDecl(name, params, vec![body]))
        }
    }

    /// 解析返回语句。
    fn parse_return_stmt(&mut self) -> Result<Stmt, String> {
        self.tokens.expect(Token::Return)?;
        // 可选的返回表达式。
        let expr = if *self.tokens.peek()? == Token::Semicolon {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.tokens.expect(Token::Semicolon)?;
        Ok(Stmt::Return(expr))
    }

    /// 解析表达式语句。
    fn parse_expr_stmt(&mut self) -> Result<Stmt, String> {
        // 解析表达式主体。
        let expr = self.parse_expression()?;
        self.tokens.expect(Token::Semicolon)?;
        Ok(Stmt::ExprStmt(expr))
    }

    /// 解析语句块。
    fn parse_block(&mut self) -> Result<Stmt, String> {
        self.tokens.expect(Token::LBrace)?;
        // 收集块内语句。
        let mut stmts = Vec::new();
        while *self.tokens.peek()? != Token::RBrace {
            stmts.push(self.parse_statement()?);
        }
        self.tokens.expect(Token::RBrace)?;
        Ok(Stmt::Block(stmts))
    }

    /// 解析表达式入口。
    fn parse_expression(&mut self) -> Result<Expr, String> {
        self.parse_and_or()
    }

    /// 解析逻辑与/或表达式。
    fn parse_and_or(&mut self) -> Result<Expr, String> {
        // 解析左侧表达式。
        let mut left = self.parse_equality()?;
        loop {
            // 读取运算符。
            let op = match self.tokens.peek()? {
                Token::And => BinOp::And,
                Token::Or => BinOp::Or,
                _ => break,
            };
            self.tokens.advance()?;
            // 解析右侧表达式。
            let right = self.parse_equality()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    /// 解析比较与相等表达式。
    fn parse_equality(&mut self) -> Result<Expr, String> {
        // 解析左侧表达式。
        let mut left = self.parse_add_sub()?;
        loop {
            // 读取运算符。
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
            // 解析右侧表达式。
            let right = self.parse_add_sub()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    /// 解析加减表达式。
    fn parse_add_sub(&mut self) -> Result<Expr, String> {
        // 解析左侧表达式。
        let mut left = self.parse_mul_div()?;
        loop {
            // 读取运算符。
            let op = match self.tokens.peek()? {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.tokens.advance()?;
            // 解析右侧表达式。
            let right = self.parse_mul_div()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    /// 解析乘除表达式。
    fn parse_mul_div(&mut self) -> Result<Expr, String> {
        // 解析左侧表达式。
        let mut left = self.parse_unary()?;
        loop {
            // 读取运算符。
            let op = match self.tokens.peek()? {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                _ => break,
            };
            self.tokens.advance()?;
            // 解析右侧表达式。
            let right = self.parse_unary()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    /// 解析一元表达式。
    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.tokens.peek()? {
            Token::Minus => {
                self.tokens.advance()?;
                // 解析一元操作数。
                let expr = self.parse_unary()?;
                Ok(Expr::Unary(UnaryOp::Minus, Box::new(expr)))
            }
            Token::Not => {
                self.tokens.advance()?;
                // 解析一元操作数。
                let expr = self.parse_unary()?;
                Ok(Expr::Unary(UnaryOp::Not, Box::new(expr)))
            }
            _ => self.parse_primary(),
        }
    }

    /// 解析基本表达式。
    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.tokens.advance()? {
            Token::Int(n) => Ok(Expr::IntLit(n)),
            Token::Str(s) => Ok(Expr::StrLit(s)),
            Token::True => Ok(Expr::BoolLit(true)),
            Token::False => Ok(Expr::BoolLit(false)),
            Token::Ident(name) => {
                if *self.tokens.peek()? == Token::LParen {
                    self.tokens.expect(Token::LParen)?;
                    // 解析实参列表。
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
                // 解析括号内表达式。
                let expr = self.parse_expression()?;
                self.tokens.expect(Token::RParen)?;
                Ok(expr)
            }
            other => Err(format!("意外的 token: {:?}", other)),
        }
    }
}