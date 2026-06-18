# Saki Language

## 项目简介

Saki Language 是一个用 Rust 实现的小型解释型语言，用于学习词法分析、语法分析与树遍历解释执行。

当前支持：变量声明与赋值、函数定义与返回、闭包、整数/字符串/布尔/null/undefined 字面量、算术与比较运算、逻辑运算（and/or/not 及 &&/||）、条件语句（if/else）、循环语句（while）、break/continue、数组字面量与索引、函数表达式、内建输出函数 `saki`。  
  
新增 JS 对齐语法入口：`let`、`const`、`var`、`function` 关键词（与 `ika`/`fn` 兼容）；新增空值语义 `undefined`，并支持 `===`、`!==`、`%` 等操作符。
`const` 不能重赋值；`var` 进入最近的函数/全局作用域；顶层 `return` 会报错。

示例（对齐 JS 思想的基础控制流）：

```saki
ika i = 0;
ika sum = 0;

while i < 5 {
    if i == 3 {
        continue;
    }
    sum = sum + i;
    i = i + 1;
}

saki("sum=", sum);
```

## 语法示例

```saki
ika x = 10;
ika y = 20;

fn add(a, b) {
	return a + b;
}

ika ok = true and not false;
saki("sum = ", add(x, y));
saki("ok = ", ok);
```

```saki
fn make_counter() {
    ika n = 0;
    return fn() {
        n = n + 1;
        return n;
    };
}

ika counter = make_counter();
saki(counter());
saki(counter());
```

## 如何运行

- 运行脚本文件：

```bash
cargo run -- hello.saki
```

- 启动 REPL：

```bash
cargo run
```

在 REPL 中输入 `exit` 退出。

## 目录结构

- src/token.rs：Token 枚举定义
- src/lexer.rs：词法分析器（字符流 → Token 流）
- src/ast.rs：AST 节点定义（表达式、语句、运算符）
- src/parser.rs：递归下降解析器（Token 流 → AST）
- src/value.rs：运行时值类型（整数、字符串、函数等）
- src/environment.rs：变量作用域与符号表
- src/interpreter.rs：树遍历解释器
- src/main.rs：REPL 入口

## 注释风格说明

代码中的注释以中文简短说明用途，覆盖函数与关键变量，便于阅读与维护。
