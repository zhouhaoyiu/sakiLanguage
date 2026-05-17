token.rs	Token 枚举定义
lexer.rs	词法分析器（字符流 → Token 流）
ast.rs	AST 节点定义（表达式、语句、运算符）
parser.rs	递归下降解析器（Token 流 → AST）
value.rs	运行时值类型（整数、字符串、函数等）
environment.rs	变量作用域与符号表
interpreter.rs	树遍历解释器
main.rs	REPL 入口