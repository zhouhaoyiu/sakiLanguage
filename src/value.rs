use std::fmt;

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Str(String),
    Bool(bool),
    Null,
    Function(Vec<String>, Vec<crate::ast::Stmt>),
    NativeFunction(fn(&[Value]) -> Result<Value, String>),
}

// 手动实现 PartialEq，忽略 NativeFunction 的比较
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Function(params1, body1), Value::Function(params2, body2)) => {
                params1 == params2 && body1 == body2
            }
            // 函数指针比较不可靠，直接返回 false
            (Value::NativeFunction(_), Value::NativeFunction(_)) => false,
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Str(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Null => write!(f, "null"),
            Value::Function(params, _) => write!(f, "<fn({})>", params.join(", ")),
            Value::NativeFunction(_) => write!(f, "<native fn>"),
        }
    }
}