use std::fmt;
use crate::environment::Environment;

#[derive(Debug, Clone)]
/// 运行时值类型。
pub enum Value {
    /// 整数值。
    Int(i64),
    /// 字符串值。
    Str(String),
    /// 布尔值。
    Bool(bool),
    /// 空值。
    Null,
    /// 未定义。
    Undefined,
    /// 数组值。
    Array(Vec<Value>),
    /// 用户定义函数。
    Function(Vec<String>, Vec<crate::ast::Stmt>, Environment),
    /// 原生内建函数。
    NativeFunction(fn(&[Value]) -> Result<Value, String>),
}

// 手动实现 PartialEq，忽略 NativeFunction 的比较。
impl PartialEq for Value {
    /// 比较两个运行时值是否相等。
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Undefined, Value::Undefined) => true,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Function(params1, body1, _), Value::Function(params2, body2, _)) => {
                params1 == params2 && body1 == body2
            }
            (Value::NativeFunction(_), Value::NativeFunction(_)) => false,
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    /// 将运行时值格式化为可读字符串。
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Str(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Null => write!(f, "null"),
            Value::Undefined => write!(f, "undefined"),
            Value::Array(items) => {
                let rendered = items
                    .iter()
                    .map(|item| item.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "[{}]", rendered)
            }
            Value::Function(params, _, _) => write!(f, "<fn({})>", params.join(", ")),
            Value::NativeFunction(_) => write!(f, "<native fn>"),
        }
    }
}
