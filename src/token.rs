#[derive(Debug, Clone, PartialEq)]
/// 词法 token 定义。
pub enum Token {
    Ika,  // 定义变量
    Fn,  // 定义函数
    Return, // 返回值
    If, // if 关键字
    Else, // else 关键字
    While, // while 关键字
    True, // 布尔真
    False, // 布尔假
    And, // 逻辑与
    Or, // 逻辑或
    Not, // 逻辑非
    Int(i64), // 整数字面量
    Str(String), // 字符串字面量
    Ident(String), // 标识符
    Plus, // 加号
    Minus, // 减号
    Star, // 乘号
    Slash, // 除号
    Eq, // 赋值号
    EqEq, // 相等比较
    Neq, // 不等比较
    Lt, // 小于
    Gt, // 大于
    Le, // 小于等于
    Ge, // 大于等于
    LParen, // 左括号
    RParen, // 右括号
    LBrace, // 左花括号
    RBrace, // 右花括号
    Comma, // 逗号
    Semicolon, // 分号
    Eof, // 文件结束
}