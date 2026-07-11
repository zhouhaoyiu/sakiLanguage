# Saki Language

## 项目简介

Saki Language 是一个用 Rust 实现的小型解释型语言，用于学习词法分析、语法分析与树遍历解释执行。

当前支持：变量声明与赋值、函数定义与返回、闭包、整数/字符串/布尔/null/undefined 字面量、算术与比较运算、逻辑运算（and/or/not/! 及 &&/||）、条件语句（if/else）、循环语句（while/for）、break/continue、数组字面量与索引、对象字面量与属性读取、函数表达式、内建输出函数 `saki`。  
  
新增 JS 对齐语法入口：`let`、`const`、`var`、`function` 关键词（与 `ika`/`fn` 兼容）；新增空值语义 `undefined`，并支持 `===`、`!==`、`%` 等操作符。
`const` 不能重赋值；`var` 进入最近的函数/全局作用域；顶层 `return` 会报错。

地震科研内建函数：`read_waveform`、`bandpass`、`window`、`pick`、`ground_motion`、`qc`、`source_inversion`、`export`。

- `read_waveform` 解析 MiniSEED 2/3。MiniSEED 3 验证 CRC32C、SID、extra headers 和可变记录长度；波形编码支持整数、IEEE float32/float64、STEIM-1/2，以及 MiniSEED 2 的 GEOSCOPE、CDSN、SRO、DWWSSN 历史编码。
- `bandpass` 使用项目内实现的四极点 Butterworth 带通与双向零相位滤波，校验 Nyquist 频率、有限值和最小样本数。
- `source_inversion` 使用带部分主元的阻尼最小二乘求六分量全矩张量，返回预测值、残差、RMS、方差缩减和条件数代理。
- `green_functions` 生成均匀各向同性全空间远场 P/S 波幅格林矩阵和走时。
- `finite_fault_inversion` 在矩形断层网格上联合求解非负分布矩，并用阻尼、空间平滑和走向/倾角/滑动角坐标搜索完成非线性反演。

科研边界：Steim-3 及缺少公开互操作定义的 USNSN/Graefenberg/IPG/HGLP/RSTN 历史编码会明确报错。格林函数采用均匀各向同性全空间远场体波模型，不包含分层介质、自由表面、衰减和仪器响应。有限断层反演输出补丁矩与滑移，结果只在输入坐标、介质参数、观测量纲和 Green 核一致时具有物理意义。

性能路径已加入 V8 思路的最小实现：直线脚本可显式编译为 bytecode 执行；对象使用 shape + slots 存储；重复属性访问会记录反馈并升为单态 inline cache；默认解释器仍走树遍历，避免单次 CLI 执行多一次编译成本。

示例（对齐 JS 思想的基础控制流）：

```saki
ika i = 0;
ika sum = 0;

while i < 5 {
    if i == 3 {
        i = i + 1;
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
let total = 0;

for (let i = 0; i < 5; i = i + 1) {
    if (i == 3) {
        continue;
    }
    total = total + i;
}

saki(total);
```

```saki
let user = {name: "saki", age: 1};
saki(user.name);
saki(user.missing);
```

```saki
let wf = read_waveform("demo.mseed");
let filtered = bandpass(wf, 1, 20);
let win = window(filtered, 0, 30);
let p = pick(win, "P");
let pga = ground_motion(win, "PGA");
let report = qc(win);

let greens = [[1, 0, 0, 0, 0, 0], [0, 1, 0, 0, 0, 0], [0, 0, 1, 0, 0, 0], [0, 0, 0, 1, 0, 0], [0, 0, 0, 0, 1, 0], [0, 0, 0, 0, 0, 1]];
let mt = source_inversion(greens, [1, -2, 3, 4, -5, 6]);

let stations = [[10, 0, 0], [0, 10, 0], [-10, 0, 0]];
let source = [0, 0, 8];
let medium = {vp_km_s: 6, vs_km_s: 3, density_kg_m3: 2700};
let gf = green_functions(stations, source, medium, "P");

export(report, "qc.txt");
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
