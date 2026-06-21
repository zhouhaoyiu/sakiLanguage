use crate::ast::*;
use crate::lexer::TokenStream;
use crate::token::Token;

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
        let mut stmts = Vec::new();
        while *self.tokens.peek()? != Token::Eof {
            stmts.push(self.parse_statement()?);
        }
        Ok(Program { statements: stmts })
    }

    /// 解析单条语句。
    fn parse_statement(&mut self) -> Result<Stmt, String> {
        match self.tokens.peek()? {
            Token::Ika | Token::Let | Token::Var | Token::Const => self.parse_var_decl(),
            Token::Fn | Token::Function => self.parse_fn_decl(),
            Token::Return => self.parse_return_stmt(),
            Token::Break => self.parse_break_stmt(),
            Token::Continue => self.parse_continue_stmt(),
            Token::If => self.parse_if_stmt(),
            Token::While => self.parse_while_stmt(),
            Token::LBrace => self.parse_block(),
            _ => self.parse_expr_stmt(),
        }
    }

    /// 解析变量声明语句。
    fn parse_var_decl(&mut self) -> Result<Stmt, String> {
        let kind = match self.tokens.advance()? {
            Token::Ika => VarKind::Let,
            Token::Let => VarKind::Let,
            Token::Var => VarKind::Var,
            Token::Const => VarKind::Const,
            _ => return Err("非法的变量声明关键字".to_string()),
        };

        let name = match self.tokens.advance()? {
            Token::Ident(n) => n,
            _ => return Err("期望变量名".to_string()),
        };

        self.tokens.expect(Token::Eq)?;
        let value = self.parse_expression()?;
        self.tokens.expect(Token::Semicolon)?;

        Ok(Stmt::VarDecl(name, value, kind))
    }

    /// 解析函数声明语句。
    fn parse_fn_decl(&mut self) -> Result<Stmt, String> {
        let token = self.tokens.advance()?;
        if !(token == Token::Fn || token == Token::Function) {
            return Err("期望 fn/function".to_string());
        }

        let name = match self.tokens.advance()? {
            Token::Ident(n) => n,
            _ => return Err("期望函数名".to_string()),
        };

        let (params, body) = self.parse_fn_signature()?;
        Ok(Stmt::FnDecl(name, params, body))
    }

    /// 解析可复用的函数签名（参数列表 + 块）。
    fn parse_fn_signature(&mut self) -> Result<(Vec<String>, Vec<Stmt>), String> {
        self.tokens.expect(Token::LParen)?;
        let mut params = Vec::new();
        if *self.tokens.peek()? != Token::RParen {
            loop {
                match self.tokens.advance()? {
                    Token::Ident(p) => params.push(p),
                    _ => return Err("期望参数名".to_string()),
                }

                if *self.tokens.peek()? == Token::Comma {
                    self.tokens.advance()?;
                } else {
                    break;
                }
            }
        }
        self.tokens.expect(Token::RParen)?;

        let Stmt::Block(stmts) = self.parse_block()? else {
            return Err("函数体必须是一个块".to_string());
        };
        Ok((params, stmts))
    }

    /// 解析返回语句。
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

    /// 解析 break 语句。
    fn parse_break_stmt(&mut self) -> Result<Stmt, String> {
        self.tokens.expect(Token::Break)?;
        self.tokens.expect(Token::Semicolon)?;
        Ok(Stmt::Break)
    }

    /// 解析 continue 语句。
    fn parse_continue_stmt(&mut self) -> Result<Stmt, String> {
        self.tokens.expect(Token::Continue)?;
        self.tokens.expect(Token::Semicolon)?;
        Ok(Stmt::Continue)
    }

    /// 解析 if 语句。
    fn parse_if_stmt(&mut self) -> Result<Stmt, String> {
        self.tokens.expect(Token::If)?;
        let condition = self.parse_expression()?;
        let then_branch = self.parse_block()?;
        let else_branch = if *self.tokens.peek()? == Token::Else {
            self.tokens.advance()?;
            if *self.tokens.peek()? == Token::If {
                let stmt = self.parse_if_stmt()?;
                Some(vec![stmt])
            } else {
                let else_block = self.parse_block()?;
                if let Stmt::Block(stmts) = else_block {
                    Some(stmts)
                } else {
                    return Err("if 语句的 else 分支必须是一个块".to_string());
                }
            }
        } else {
            None
        };

        if let Stmt::Block(then_stmts) = then_branch {
            Ok(Stmt::If(condition, then_stmts, else_branch))
        } else {
            Err("if 语句的条件体必须是一个块".to_string())
        }
    }

    /// 解析 while 语句。
    fn parse_while_stmt(&mut self) -> Result<Stmt, String> {
        self.tokens.expect(Token::While)?;
        let condition = self.parse_expression()?;
        let body = self.parse_block()?;

        if let Stmt::Block(stmts) = body {
            Ok(Stmt::While(condition, stmts))
        } else {
            Err("while 语句的循环体必须是一个块".to_string())
        }
    }

    /// 解析表达式语句。
    fn parse_expr_stmt(&mut self) -> Result<Stmt, String> {
        let expr = self.parse_expression()?;
        self.tokens.expect(Token::Semicolon)?;
        Ok(Stmt::ExprStmt(expr))
    }

    /// 解析语句块。
    fn parse_block(&mut self) -> Result<Stmt, String> {
        self.tokens.expect(Token::LBrace)?;
        let mut stmts = Vec::new();
        while *self.tokens.peek()? != Token::RBrace {
            if *self.tokens.peek()? == Token::Eof {
                return Err("语句块缺少 '}'".to_string());
            }
            stmts.push(self.parse_statement()?);
        }
        self.tokens.expect(Token::RBrace)?;
        Ok(Stmt::Block(stmts))
    }

    /// 解析表达式入口。
    fn parse_expression(&mut self) -> Result<Expr, String> {
        self.parse_assignment()
    }

    /// 解析赋值表达式。
    fn parse_assignment(&mut self) -> Result<Expr, String> {
        let expr = self.parse_and_or()?;

        if let Expr::Ident(name) = expr {
            if *self.tokens.peek()? == Token::Eq {
                self.tokens.advance()?;
                let value = self.parse_assignment()?;
                return Ok(Expr::Assign(name, Box::new(value)));
            }
            return Ok(Expr::Ident(name));
        }

        if *self.tokens.peek()? == Token::Eq {
            Err("赋值语句左侧必须是变量".to_string())
        } else {
            Ok(expr)
        }
    }

    /// 解析逻辑与/或表达式。
    fn parse_and_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_equality()?;

        while let Some(op) = match self.tokens.peek()? {
            Token::And | Token::AndAnd => Some(BinOp::And),
            Token::Or | Token::OrOr => Some(BinOp::Or),
            _ => None,
        } {
            self.tokens.advance()?;
            let right = self.parse_equality()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }

        Ok(left)
    }

    /// 解析比较与相等表达式。
    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_add_sub()?;
        while let Some(op) = match self.tokens.peek()? {
            Token::EqEq => Some(BinOp::EqEq),
            Token::EqEqEq => Some(BinOp::EqEqEq),
            Token::Neq => Some(BinOp::Neq),
            Token::NeqEq => Some(BinOp::NeqEq),
            Token::Lt => Some(BinOp::Lt),
            Token::Gt => Some(BinOp::Gt),
            Token::Le => Some(BinOp::Le),
            Token::Ge => Some(BinOp::Ge),
            _ => None,
        } {
            self.tokens.advance()?;
            let right = self.parse_add_sub()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    /// 解析加减表达式。
    fn parse_add_sub(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_mul_div()?;
        while let Some(op) = match self.tokens.peek()? {
            Token::Plus => Some(BinOp::Add),
            Token::Minus => Some(BinOp::Sub),
            _ => None,
        } {
            self.tokens.advance()?;
            let right = self.parse_mul_div()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    /// 解析乘除表达式。
    fn parse_mul_div(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        while let Some(op) = match self.tokens.peek()? {
            Token::Star => Some(BinOp::Mul),
            Token::Slash => Some(BinOp::Div),
            Token::Percent => Some(BinOp::Mod),
            _ => None,
        } {
            self.tokens.advance()?;
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
                Ok(Expr::Unary(UnaryOp::Minus, Box::new(self.parse_unary()?)))
            }
            Token::Plus => {
                self.tokens.advance()?;
                Ok(Expr::Unary(UnaryOp::Plus, Box::new(self.parse_unary()?)))
            }
            Token::Not => {
                self.tokens.advance()?;
                Ok(Expr::Unary(UnaryOp::Not, Box::new(self.parse_unary()?)))
            }
            _ => self.parse_call(),
        }
    }

    /// 解析函数调用和索引后缀表达式。
    fn parse_call(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;

        loop {
            match self.tokens.peek()? {
                Token::LParen => {
                    self.tokens.advance()?;
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
                    expr = Expr::Call(Box::new(expr), args);
                }
                Token::LBracket => {
                    self.tokens.advance()?;
                    let index = self.parse_expression()?;
                    self.tokens.expect(Token::RBracket)?;
                    expr = Expr::Index(Box::new(expr), Box::new(index));
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    /// 解析基本表达式。
    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.tokens.advance()? {
            Token::Int(n) => Ok(Expr::IntLit(n)),
            Token::Str(s) => Ok(Expr::StrLit(s)),
            Token::True => Ok(Expr::BoolLit(true)),
            Token::False => Ok(Expr::BoolLit(false)),
            Token::Null => Ok(Expr::NullLit),
            Token::Undefined => Ok(Expr::UndefinedLit),
            Token::Fn | Token::Function => self.parse_fn_expr_after_consumed(),
            Token::Ident(name) => Ok(Expr::Ident(name)),
            Token::LParen => {
                let expr = self.parse_expression()?;
                self.tokens.expect(Token::RParen)?;
                Ok(expr)
            }
            Token::LBracket => {
                let mut items = Vec::new();
                if *self.tokens.peek()? != Token::RBracket {
                    loop {
                        items.push(self.parse_expression()?);
                        if *self.tokens.peek()? == Token::Comma {
                            self.tokens.advance()?;
                        } else {
                            break;
                        }
                    }
                }
                self.tokens.expect(Token::RBracket)?;
                Ok(Expr::Array(items))
            }
            other => Err(format!("意外的 token: {:?}", other)),
        }
    }

    /// 在 parse_primary 已经消费了 fn/function 的前提下解析函数表达式。
    fn parse_fn_expr_after_consumed(&mut self) -> Result<Expr, String> {
        let (params, body) = self.parse_fn_signature()?;
        Ok(Expr::FunctionExpr(params, body))
    }

}
