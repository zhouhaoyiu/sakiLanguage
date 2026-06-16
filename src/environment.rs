use std::collections::HashMap;
use crate::value::Value;

#[derive(Clone)]
struct Binding {
    value: Value,
    mutable: bool,
}

#[derive(Clone)]
/// 运行时变量作用域。
pub struct Environment {
    /// 当前作用域的变量表。
    variables: HashMap<String, Binding>,
    /// 指向外层作用域。
    parent: Option<Box<Environment>>,
}

impl Environment {
    /// 创建顶层作用域并注册内建函数。
    pub fn new() -> Self {
        // 初始化环境对象。
        let mut env = Environment {
            variables: HashMap::new(),
            parent: None,
        };
        // 注册内建函数 saki
        env.variables.insert(
            "saki".to_string(),
            Binding {
                value: Value::NativeFunction(native_saki),
                mutable: false,
            },
        );
        env.variables.insert(
            "null".to_string(),
            Binding {
                value: Value::Null,
                mutable: false,
            },
        );
        env.variables.insert(
            "undefined".to_string(),
            Binding {
                value: Value::Undefined,
                mutable: false,
            },
        );
        // 返回初始化后的环境。
        env
    }

    /// 创建带父作用域的新环境。
    pub fn new_enclosed(parent: Environment) -> Self {
        Environment {
            variables: HashMap::new(),
            parent: Some(Box::new(parent)),
        }
    }

    /// 在作用域链中查找变量。
    pub fn get(&self, name: &str) -> Result<Value, String> {
        // 先查当前作用域。
        if let Some(val) = self.variables.get(name) {
            Ok(val.value.clone())
        } else if let Some(parent) = &self.parent {
            // 递归查找父作用域。
            parent.get(name)
        } else {
            // 未找到变量时返回错误。
            Err(format!("未定义的变量 '{}'", name))
        }
    }

    // 变量赋值。
    pub fn set(&mut self, name: &str, value: Value) -> Result<(), String> {
        if self.variables.contains_key(name) {
            let binding = self.variables.get_mut(name).unwrap();
            if !binding.mutable {
                return Err(format!("变量 '{}' 是只读变量", name));
            }
            binding.value = value;
            return Ok(());
        }

        if let Some(parent) = &mut self.parent {
            parent.set(name, value)
        } else {
            Err(format!("未定义的变量 '{}'", name))
        }
    }

    /// 在当前作用域中定义变量。
    pub fn define(&mut self, name: &str, value: Value) {
        self.define_with(name, value, true);
    }

    /// 在当前作用域中定义变量，并设置可变性。
    pub fn define_with(&mut self, name: &str, value: Value, mutable: bool) {
        // 写入变量绑定。
        self.variables.insert(
            name.to_string(),
            Binding {
                value,
                mutable,
            },
        );
    }

    /// 从当前作用域返回到父作用域。如果不存在父作用域则返回自身。
    pub fn pop_parent(self) -> Self {
        if let Some(parent) = self.parent {
            *parent
        } else {
            self
        }
    }
}

/// 内建输出函数：打印参数并换行。
fn native_saki(args: &[Value]) -> Result<Value, String> {
    // 逐个输出参数。
    for val in args {
        print!("{}", val);
    }
    // 追加换行。
    println!();
    // 返回空值。
    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn i64v(n: i64) -> Value {
        Value::Int(n)
    }

    #[test]
    fn define_and_get_local_variable() {
        let mut env = Environment::new();
        env.define("x", i64v(1));
        assert_eq!(env.get("x").unwrap(), i64v(1));
    }

    #[test]
    fn set_updates_current_scope_first() {
        let mut env = Environment::new();
        env.define("x", i64v(1));
        env.set("x", i64v(2)).unwrap();
        assert_eq!(env.get("x").unwrap(), i64v(2));
    }

    #[test]
    fn set_walks_to_parent_scope() {
        let mut outer = Environment::new();
        outer.define("x", i64v(1));
        let mut inner = Environment::new_enclosed(outer);
        inner.set("x", i64v(9)).unwrap();
        assert_eq!(inner.get("x").unwrap(), i64v(9));
    }

    #[test]
    fn set_undefined_variable_returns_error() {
        let mut env = Environment::new();
        let err = env.set("missing", Value::Null).unwrap_err();
        assert_eq!(err, "未定义的变量 'missing'");
    }
}
