# Saki Language

## 项目简介

Saki Language 是一个用 Rust 实现的小型解释型语言，用于学习词法分析、语法分析与树遍历解释执行。

当前支持：变量声明、函数定义与返回、整数/字符串/布尔字面量、算术与比较运算、逻辑运算（and/or/not）、内建输出函数 `saki`。

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