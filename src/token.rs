#[derive(Debug, Clone, PartialEq)]
/// 词法 token 定义。
pub enum Token {
    Ika,  // 定义变量
    Fn,  // 定义函数
    Let, // let 声明
    Var, // var 声明
    Const, // const 声明
    Function, // function 声明/表达式关键字别名
    Return, // 返回值
    If, // if 关键字
    Else, // else 关键字
    While, // while 关键字
    Break, // break
    Continue, // continue
    Null, // null
    Undefined, // undefined
    True, // 布尔真
    False, // 布尔假
    And, // 逻辑与
    Or, // 逻辑或
    Not, // 逻辑非
    EqEqEq, // 严格相等
    NeqEq, // 严格不等
    Int(i64), // 整数字面量
    Str(String), // 字符串字面量
    Ident(String), // 标识符
    Plus, // 加号
    Minus, // 减号
    Star, // 乘号
    Slash, // 除号
    Percent, // 取模
    AndAnd, // &&
    OrOr, // ||
    Eq, // 赋值号
    EqEq, // 相等比较
    Neq, // 不等比较
    Lt, // 小于
    Gt, // 大于
    Le, // 小于等于
    Ge, // 大于等于
    LParen, // 左括号
    RParen, // 右括号
    LBracket, // 左中括号
    RBracket, // 右中括号
    LBrace, // 左花括号
    RBrace, // 右花括号
    Comma, // 逗号
    Semicolon, // 分号
    Eof, // 文件结束
}
