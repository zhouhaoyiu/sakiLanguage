use crate::value::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone)]
struct Binding {
    value: Value,
    mutable: bool,
}

#[derive(Debug, Clone)]
struct EnvironmentData {
    variables: HashMap<String, Binding>,
    parent: Option<Environment>,
    function_scope: bool,
}

#[derive(Debug, Clone)]
/// 运行时变量作用域。
pub struct Environment {
    inner: Rc<RefCell<EnvironmentData>>,
}

impl Environment {
    /// 创建顶层作用域并注册内建函数。
    pub fn new() -> Self {
        let env = Environment {
            inner: Rc::new(RefCell::new(EnvironmentData {
                variables: HashMap::new(),
                parent: None,
                function_scope: true,
            })),
        };
        env.define_with("saki", Value::NativeFunction(native_saki), false);
        env.define_with("null", Value::Null, false);
        env.define_with("undefined", Value::Undefined, false);
        env
    }

    /// 创建块作用域。
    pub fn new_enclosed(parent: Environment) -> Self {
        Self::new_child(parent, false)
    }

    /// 创建函数作用域。
    pub fn new_function(parent: Environment) -> Self {
        Self::new_child(parent, true)
    }

    fn new_child(parent: Environment, function_scope: bool) -> Self {
        Environment {
            inner: Rc::new(RefCell::new(EnvironmentData {
                variables: HashMap::new(),
                parent: Some(parent),
                function_scope,
            })),
        }
    }

    /// 在作用域链中查找变量。
    pub fn get(&self, name: &str) -> Result<Value, String> {
        let mut env = Some(self.clone());
        while let Some(current) = env {
            let data = current.inner.borrow();
            if let Some(binding) = data.variables.get(name) {
                return Ok(binding.value.clone());
            }
            env = data.parent.clone();
        }
        Err(format!("未定义的变量 '{}'", name))
    }

    /// 变量赋值。
    pub fn set(&self, name: &str, value: Value) -> Result<(), String> {
        let mut env = Some(self.clone());
        while let Some(current) = env {
            let mut data = current.inner.borrow_mut();
            if let Some(binding) = data.variables.get_mut(name) {
                if !binding.mutable {
                    return Err(format!("变量 '{}' 是只读变量", name));
                }
                binding.value = value;
                return Ok(());
            }
            env = data.parent.clone();
        }
        Err(format!("未定义的变量 '{}'", name))
    }

    /// 在当前作用域中定义可写变量。
    pub fn define(&self, name: &str, value: Value) {
        self.define_with(name, value, true);
    }

    /// 在当前作用域中定义变量，并设置可变性。
    pub fn define_with(&self, name: &str, value: Value, mutable: bool) {
        self.inner
            .borrow_mut()
            .variables
            .insert(name.to_string(), Binding { value, mutable });
    }

    /// var 进入最近的函数/全局作用域。
    pub fn define_var(&self, name: &str, value: Value) {
        let mut target = self.clone();
        loop {
            let parent = {
                let data = target.inner.borrow();
                if data.function_scope {
                    None
                } else {
                    data.parent.clone()
                }
            };

            if let Some(parent) = parent {
                target = parent;
            } else {
                target.define(name, value);
                return;
            }
        }
    }
}

/// 内建输出函数：打印参数并换行。
fn native_saki(args: &[Value]) -> Result<Value, String> {
    for val in args {
        print!("{}", val);
    }
    println!();
    Ok(Value::Null)
}
