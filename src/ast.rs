#[derive(Debug, Clone, PartialEq)]
/// 表达式节点。
pub enum Expr {
    /// 整数字面量。
    IntLit(i64),
    /// 字符串字面量。
    StrLit(String),
    /// 布尔字面量。
    BoolLit(bool),
    /// 标识符引用。
    Ident(String),
    /// 二元运算表达式。
    Binary(Box<Expr>, BinOp, Box<Expr>),
    /// 一元运算表达式。
    Unary(UnaryOp, Box<Expr>),
    /// 函数调用表达式。
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
/// 二元运算符。
pub enum BinOp {
    /// 加法。
    Add,
    /// 减法。
    Sub,
    /// 乘法。
    Mul,
    /// 除法。
    Div,
    /// 小于比较。
    Lt,
    /// 大于比较。
    Gt,
    /// 小于等于比较。
    Le,
    /// 大于等于比较。
    Ge,
    /// 相等比较。
    EqEq,
    /// 不等比较。
    Neq,
    /// 逻辑与。
    And,
    /// 逻辑或。
    Or,
}

#[derive(Debug, Clone, PartialEq)]
/// 一元运算符。
pub enum UnaryOp {
    /// 取负。
    Minus,
    /// 逻辑非。
    Not,
}

#[derive(Debug, Clone, PartialEq)]
/// 语句节点。
pub enum Stmt {
    /// 变量声明语句。
    VarDecl(String, Expr),
    /// 函数声明语句。
    FnDecl(String, Vec<String>, Vec<Stmt>),
    /// 表达式语句。
    ExprStmt(Expr),
    /// 返回语句。
    Return(Option<Expr>),
    /// 语句块。
    Block(Vec<Stmt>),
}

/// 程序根节点。
pub struct Program {
    /// 顶层语句列表。
    pub statements: Vec<Stmt>,
}