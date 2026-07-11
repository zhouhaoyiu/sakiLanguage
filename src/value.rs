use crate::environment::Environment;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
/// 对象形状：属性名到 slot 的稳定映射。
pub struct ObjectShape {
    keys: Vec<String>,
    offsets: BTreeMap<String, usize>,
}

impl ObjectShape {
    /// 从属性顺序创建对象形状。
    pub fn new(keys: Vec<String>) -> Self {
        let offsets = keys
            .iter()
            .enumerate()
            .map(|(index, key)| (key.clone(), index))
            .collect();
        ObjectShape { keys, offsets }
    }

    /// 形状唯一标识，用于 inline cache。
    pub fn id(&self) -> usize {
        self as *const Self as usize
    }

    /// 返回属性 slot。
    pub fn slot(&self, key: &str) -> Option<usize> {
        self.offsets.get(key).copied()
    }

    /// 返回属性名列表。
    pub fn keys(&self) -> &[String] {
        &self.keys
    }
}

#[derive(Debug, Clone)]
/// 对象值：共享形状 + 紧凑 slot。
pub struct ObjectValue {
    shape: Rc<ObjectShape>,
    slots: Vec<Value>,
}

impl ObjectValue {
    /// 创建对象值。
    pub fn new(shape: Rc<ObjectShape>, slots: Vec<Value>) -> Self {
        ObjectValue { shape, slots }
    }

    /// 形状唯一标识。
    pub fn shape_id(&self) -> usize {
        self.shape.id()
    }

    /// 返回属性 slot。
    pub fn slot(&self, key: &str) -> Option<usize> {
        self.shape.slot(key)
    }

    /// 按 slot 读取值。
    pub fn get_slot(&self, slot: usize) -> Option<&Value> {
        self.slots.get(slot)
    }

    /// 按属性名读取值。
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.slot(key).and_then(|slot| self.get_slot(slot))
    }

    /// 返回对象形状。
    pub fn shape(&self) -> &ObjectShape {
        &self.shape
    }
}

impl PartialEq for ObjectValue {
    fn eq(&self, other: &Self) -> bool {
        self.shape.keys() == other.shape.keys() && self.slots == other.slots
    }
}

#[derive(Debug, Clone)]
/// 运行时值类型。
pub enum Value {
    /// 整数值。
    Int(i64),
    /// 浮点值，供科学计算内建函数使用。
    Float(f64),
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
    /// 对象值。
    Object(ObjectValue),
    /// 用户定义函数。
    Function(Vec<String>, Vec<crate::ast::Stmt>, Environment),
    /// 原生内建函数。
    NativeFunction(fn(&[Value]) -> Result<Value, String>),
}

impl Value {
    /// 构造对象值。
    pub fn object(entries: Vec<(&str, Value)>) -> Self {
        let keys = entries
            .iter()
            .map(|(key, _)| key.to_string())
            .collect::<Vec<_>>();
        let slots = entries.into_iter().map(|(_, value)| value).collect();
        Value::Object(ObjectValue::new(Rc::new(ObjectShape::new(keys)), slots))
    }

    /// 读取对象属性。
    pub fn property(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(object) => object.get(key),
            _ => None,
        }
    }
}

// 手动实现 PartialEq，忽略 NativeFunction 的比较。
impl PartialEq for Value {
    /// 比较两个运行时值是否相等。
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Undefined, Value::Undefined) => true,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => a == b,
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
            Value::Float(n) => write!(f, "{}", n),
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
            Value::Object(entries) => {
                let rendered = entries
                    .shape()
                    .keys()
                    .iter()
                    .enumerate()
                    .map(|(slot, key)| {
                        format!(
                            "{}: {}",
                            key,
                            entries.get_slot(slot).unwrap_or(&Value::Undefined)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{{{}}}", rendered)
            }
            Value::Function(params, _, _) => write!(f, "<fn({})>", params.join(", ")),
            Value::NativeFunction(_) => write!(f, "<native fn>"),
        }
    }
}
