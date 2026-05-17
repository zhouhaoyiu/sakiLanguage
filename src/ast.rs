#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit(i64),
    StrLit(String),
    BoolLit(bool),
    Ident(String),
    Binary(Box<Expr>, BinOp, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Gt,
    Le,
    Ge,
    EqEq,
    Neq,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Minus,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    VarDecl(String, Expr),
    FnDecl(String, Vec<String>, Vec<Stmt>),
    ExprStmt(Expr),
    Return(Option<Expr>),
    Block(Vec<Stmt>),
}

pub struct Program {
    pub statements: Vec<Stmt>,
}