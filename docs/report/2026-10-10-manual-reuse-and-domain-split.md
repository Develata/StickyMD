# 人工验收复用与产品域拆细：设计提案（2026-10-10）

> 状态：**提案，待 USER 批准**（第 1 轮审查后的修正见文末"修订"一节）。根因 3 修改 plan 11 的人工验收合同，属于骨架级变化；按 AGENTS.md 先报告、
> 经 USER 批准写入 `docs/plan/` 后才实施。本文不是契约。

## 背景与授权

USER 要求"如果一个模块没有被动过，那么对应的测试就不需要再跑一遍"，并在 2026-10-09 批准按四个根因修复；
根因 1、2（U1）已推送为 `fix/qualification-ledger-reuse`（`972aad4..fcf99dc`，九轮审查，末轮 PASS）。
2026-10-10 USER 指示："之后再去把根因3和根因4做好，同样的，还是和gpt交互多轮迭代优化清楚以后再分批
commit + push。"

- 根因 3：24 项人工验收（P12-Mxx 中的 Manual 项）不在账本里，绑定当前候选，换候选必须全部重做。
- 根因 4：产品域划分过粗（`EDITOR`、`PREVIEW` 进入几乎所有模块），复用很少触发。

早先方案审查对人工复用给出的阻断条件：依赖闭包必须有调用链依据（`app/input.rs` 同时分发窗口、Preview 与
滚轮）；环境身份必须完整（含补丁号）；同一指纹后来的 `MANUAL_FAIL` 必须优先；M34 Clean VM 保持精确制品
绑定；保留 Tier C 语义、候选绑定的 waiver 与当前候选汇总。

## 已核实现状

- **合同**（plan 11）：`MANUAL PASS` 指"当前提交上的正式人工矩阵已执行"；"一次性终端命令、未提交脚本、
  主观观察、旧 commit 收据均不能把人工项从 NOT TESTED 提升为 PASS"；"最终 manual PASS 仍额外绑定 Promoted
  Candidate"；waiver 只接受具体 `WAIVER-P12-Mxx`，身份来自同一 Source Freeze。
- **收据**：单文件 `dist/evidence/manual-acceptance.json`，绑定候选的 source/EXE/ZIP/version；续录与 readiness
  共用 `manual_receipt::validated_cases`（U1 第八、九轮）。
- **人工项**（24 项）：Tier A：M01、M02（输入法视觉）、M05（Alt+Tab 后恢复并输入）、M11/M12（左右 dock）、
  M18–M20（220×120 三种模式）、M21（缩放）、M22（透明度）、M23（主题）、M24–M26（Markdown/公式/图片视觉）；
  Tier B：M34（Clean VM）、M35–M37（多屏与拔插）、M38–M40（125/150/200% DPI）；Tier C：M41–M43（睡眠、RDP、
  负坐标显示器）。
- **域映射**（`module_ledger/fingerprint.rs`）：`app/input.rs` 只归 `EDITOR`，但它分发 IME、按键（缩放、剪贴板、
  保存/导出快捷键）、鼠标按键、光标移动与滚轮。现有自动化模块都包含 `EDITOR`，因此这一低估目前没有造成漏判；
  一旦人工项使用更细的域，它就会漏判。
- **发布历史**：v0.1.2→v0.1.3 的产品改动只有 `flow/editor.rs` 与 render crate 的 `lib.rs`、`math_text.rs`；
  v0.1.1→v0.1.2 改了约 44 个产品文件，集中在 Preview 与 source。v0.1.1、v0.1.2 发布时人工项均为 NOT TESTED，
  即目前没有任何可复用的人工 PASS；第一次采用新机制的版本仍需完整人工基线。

## 根因 3：人工项进入账本

### 模型

1. 每个人工项是一个账本模块 `manual-p12-mXX`，复用共享存储、指纹定位、锁与严格记录。evidence 是该项的单条
   观察文档（case、状态、备注、观察时的候选身份、环境身份、记录时间）。
2. **人工项指纹** = 该项声明的产品域输入 + harness 输入（人工记录器与引导会话代码、`phase-12.md` 中该行、
   `docs/plan/11` 与相关 plan 章节）+ **环境身份**。
3. **环境身份**按项声明所需维度，写入指纹：Windows 完整版本（含 UBR 补丁号）、显示器拓扑（数量、排列、每屏
   DPI）、系统主题设置、输入法 profile 与版本（M01/M02/M22/M36 等涉及 IME 的项）。补丁号变化会使声明了 Windows
   版本的项失效；这是审查要求的保守取舍，代价如实接受。
4. **记录最新观察，而非最新成功**：每个 `(项, 指纹)` 只保存最近一次正式观察。`MANUAL_FAIL` 覆盖此前的
   `MANUAL_PASS`，readiness 遇到 FAIL 即阻断；只有之后的 PASS 能再覆盖它。
5. **M34 Clean VM 不复用**：其指纹包含候选 ZIP 的 SHA-256，等价于逐候选重做（它验证的就是这份包在干净机器
   上运行）。
6. **Tier C 与 waiver 不变**：Tier C 在自动化覆盖全部通过时允许 NOT_TESTED；waiver 仍是绑定 Source Freeze 版本
   的 `WAIVER-P12-Mxx`，不进入账本、不跨版本复用。
7. **候选绑定汇总**：readiness 为当前候选写出人工汇总，逐项标明 `RAN PASS`、`REUSED PASS (origin candidate)`、
   `WAIVED`、`NOT TESTED` 或 `FAILED`。发布记录仍绑定当前候选，复用只改变"观察来自哪次运行"。
8. **记录器**：引导会话只列出 `RUN REQUIRED` 的项；每录入一项立即登记到账本（原子发布），中断不丢已录项。

### 域闭包（初始保守）

在根因 4 完成前，人工项的产品域取**全部产品域**，与审查要求一致。这意味着：任何产品代码改动都会使全部人工
项失效；只有"仅工具/文档/版本号"的版本能复用人工结果。v0.1.3 这类改了编辑器代码的版本仍需全部重测。

### plan 11 需要的修改（待批准）

- 将"旧 commit 收据均不能把人工项从 NOT TESTED 提升为 PASS"改为：人工项的正式观察按"项指纹（产品域、harness、
  环境身份）"复用；指纹不变的最近一次正式观察有效，FAIL 优先；M34 逐候选。
- `MANUAL PASS` 的定义从"当前提交上执行"改为"与当前项指纹兼容的最近一次正式观察为 PASS"，并保留 RAN/REUSED
  区分。
- 新增人工项的环境身份维度与采集方式。

## 根因 4：产品域拆细

### 步骤

1. **分类诚实化**（无产品代码改动）：所有分发/路由文件按其实际分发的全部域归类（例如 `app/input.rs` →
   `EDITOR|SHELL|PREVIEW|PERSISTENCE|EXPORT`：鼠标路径到达工具栏、滚动条与窗口（SHELL）、Preview 拖选
（PREVIEW）与 Source（EDITOR），快捷键到达保存与导出）。新增"每个产品文件都必须被显式分类"的守卫测试，
   不再依赖 `ALL_PRODUCT` 兜底。
2. **按域拆分分发**（产品代码重构，行为不变）：把 `app/input.rs`（713 行，已超过 500 行审视线）拆为编辑输入
   （IME、文本键、撤销）、窗口/壳层输入（鼠标拖动、dock 相关）、缩放、快捷键路由；路由层只做分派并归入所有
   目标域，但改动频率低。`preview_input.rs` 已独立。
3. **逐项声明人工项与自动化模块的域**：以调用链为依据，例如 M11/M12（dock）→ `SHELL`；M24（Markdown 视觉）→
   `PREVIEW`；M25 → `PREVIEW|MATH`；M26 → `PREVIEW|IMAGES`；M35/M37 → `SHELL`；涉及输入的项包含 `EDITOR`。

### 预期收益（以 v0.1.3 回放，环境不变且此前有 PASS 为前提）

v0.1.3 的改动落在 `EDITOR`（`flow/editor.rs`）与数学文本转换（`math_text.rs`，拆细后归 `EDITOR|MATH`）。按上面
的声明，M11、M12、M19、M23、M24、M26、M35、M37 约 8 项可复用，其余 16 项仍需重测。没有根因 4，全部 24 项重测。
这一数字依赖逐项域声明，需在审查中逐项核对。

### 风险

- 共享状态耦合：同一进程内不同域通过 `StickyApp` 状态互相影响，路径域只是近似；以调用链与已有自动化覆盖为依据，
  无法证明时归入更宽的域。
- 重构风险：拆分 `input.rs` 改动产品代码，需全部单元测试与 smoke 覆盖，且下一个候选必然重跑全部模块。

## 实施分批（批准后）

1. 根因 4 第 1 步（分类诚实化 + 守卫测试）。
2. 根因 3（人工项账本、环境身份、记录器、readiness 汇总、plan 11 修改）。
3. 根因 4 第 2、3 步（`input.rs` 拆分与逐项域声明）。

每批经 gpt-6-astra 多轮审查后提交并推送。

## 对规格的影响

plan 11 `#module-success-ledger` 与人工验收相关段落、`phase-12.md` 人工项说明、`phase-14.md` 新增验收项、
coverage matrix。

## 修订（第 1 轮审查后，2026-10-10）

gpt-6-astra（max）第 1 轮给出 4 个 BLOCKER、3 个 SHOULD_FIX，逐条对照代码与合同核实后**全部成立**。本节修正
上文；与上文冲突处以本节为准。

### 收益更正（先读）

- **v0.1.3 回放 = 0 项可复用**，不是"约 8 项"。`v0.1.2..972aad4` 除 `flow/editor.rs`、render `lib.rs`、
  `math_text.rs`、`math_text/tests.rs` 外还改了 `docs/plan/07_editor_and_ime.md` 与 `phase-11-b.md`；按现行规则
  （plan 11 `#module-success-ledger`："`docs/plan/`、`docs/acceptance-cases/` 因跨域约束仍对全部功能模块保守传播"，
  实现见 `fingerprint.rs:194`），在任何产品域推理之前全部模块已失效。即使忽略合同变化，render `lib.rs` 归
  `PREVIEW|EDITOR|MATH|IMAGES|EXPORT`，M19/M24/M26 不成立；M23 断言公式、图片与控件，也不成立。
- **拆细后的上限**：只有 M11/M12/M35/M37 有可能，且每项都要先证明 `window_guards`
  （`window_runtime.rs:252`）读到的 composition、搜索、导出、恢复、资产事务、保存状态的写入方都在闭包内；
  按现状这些写入方覆盖大部分产品域，实际可能接近 0。
- **补丁窗口**：环境身份含 UBR，Windows 每次累积更新（每月一次，另有可选预览更新）都使所有声明 Windows 版本的项
  失效。现实的复用场景是：**同一台机器、同一套显示器、同一补丁窗口内，且该版本没有改动该项的任何输入**——
  典型是只改发布流程、release notes、README 或非全局工具代码后重建候选。
- 因此根因 3 的主要价值不是"产品改动后少测"，而是**候选重建时不重测**，以及让下一次完整人工基线以可复用的
  格式落盘。

### 新核实的事实

- plan 11:316"可模块化 manual observation 使用 Last Successful Module Receipt"来自 `748c3d6`（2026-08-30），
  早于 U1，与 :237/:243（旧 commit 收据不能提升人工项）矛盾；"可模块化"在全仓库没有定义。这是现存矛盾。
- 账本合同"记录只允许 `PASSED`"，`01_terminology.md` 的 Last Successful Module Receipt"失败或中止不得覆盖"；
  `record.rs` 只接受 `PASSED`，`store.rs` 每模块保留 8 份、没有条件写入。记录 FAIL 是第二种证据策略，需要新术语。
- waiver：plan 11:258 只接受逐项 key，:652–655 允许 Tier B 组 waiver，代码（`manual_readiness.rs`、
  `decisions.rs`）接受 `WAIVER-TIER-B-v{version}`。这也是现存矛盾，上文"waiver 不变"的现状描述不准确。
- 记录器：逐项记录器（`manual.rs` `record`）覆盖全部 24 项；引导记录器只覆盖 14 个 Tier A 项，且 G2-04
  （M11/M12）、G2-05（M18–M20）一次回答写给多项。
- `windows_build::is_known` 接受三段版本；`ver` 实际输出四段（第四段即 UBR）。
- 工具已有一处 FFI 先例（`atomic_evidence.rs` 的 `MoveFileExW`），没有 COM。
- 产品字体按候选族名顺序取第一个存在的（`fonts.rs`，首选 `仿宋_GB2312`，通常需用户自行安装），因此相同
  Windows 版本不等于相同字体环境。
- plan 11:91 要求"当前与前一个受支持 Windows 11 版本"，现有工具不建模这一要求；这是现存缺口，不由根因 3 解决，
  单独提请注意。

### 修正后的设计

1. **一个注册表、共用存储设施、两种证据策略。** 自动化模块保持 Last Successful Module Receipt。人工项采用新
   术语 **Latest Manual Observation**：每个 `(项, 指纹)` 保存最近一次**完成**的观察，状态只能是 `MANUAL_PASS`
   或 `MANUAL_FAIL`；独立记录类型与校验；readiness 对人工项走人工策略，不进入"所有模块必须 PASS"的通用分支。
2. **生命周期。** `NOT_TESTED`、跳过、中止从不写入账本，也从不覆盖已有记录。`MANUAL_FAIL` 不参与清理（PASS
   仍按每项 8 份清理），直到同一指纹上一次完整 PASS 替换它。FAIL 优先于 waiver 与 Tier C 的非阻断规则。
   写入用条件更新：记录器把展示给观察者的当前记录版本（evidence 摘要或"无"）随提交带回，在写锁内比较，不一致
   即拒绝并要求重新观察；不用时间戳决定先后。写入失败使命令失败，不输出汇总。提供对已有 PASS 显式重测的入口，
   用于登记后来发现的 FAIL。引导会话按项提交，一步覆盖多项时逐项询问状态。
3. **指纹。** 继承现有共享/GLOBAL 规则（manifest、lock、toolchain、registry、指纹算法、证据校验器等），版本号
   规范化沿用现有窄规则；再加该项产品域、harness（记录器、引导步骤、场景表、环境 schema）、合同输入与环境身份。
   上文"只有仅工具/文档/版本号的版本能复用"改为"**不影响该项输入**的改动可复用"。
4. **环境分三类，每项逐一声明。**
   - **主机事实**（readiness 时与当前主机比较，工具经子进程或文件读取自动采集，不新增 FFI）：Windows 四段版本
     （`ver`，含 UBR；人工与 exact evidence 一并收紧为四段，`windows_build.rs` 属全局输入）、系统与应用主题
     （`reg query`）、字体目录清单摘要（系统与用户字体目录的文件名+大小，不读字体字节）。采集失败或 `UNKNOWN`
     即不可复用。
   - **场景参数**（逐场景记录，只检查是否覆盖该项要求的全部场景，不与当前主机比较）：DPI 档位、显示器拓扑、
     主题模式与运行时切换、M21 的缩放档位×视图。由观察者声明并标注来源为"声明"。场景表放在工具内，按
     plan 07/11 与 `phase-12.md` 逐项列出（例如 M01：微软拼音 × 100/150/200%；M23：Light、Dark、System、
     运行时切换）；任一场景 FAIL 则该项 FAIL，缺场景则不能登记 PASS。
   - **工具无法可靠识别的外部依赖 → 不复用，绑定精确制品**：第三方输入法会静默自动更新，TSF profile 不含产品
     版本，微软拼音还有"使用以前版本"开关，相同 profile 名不能证明行为相同。因此断言 IME 行为的项——M01、M02、
     M05、M18、M22、M36、M41——与 M34 一样每份新 ZIP 重测。以后若加入可靠的 IME 采集再放开。
5. **M34** 绑定 ZIP 的 SHA-256 与声明的 Clean VM 基线：同一份字节重新晋升可复用；新 ZIP 即使 EXE 相同也重测。
6. **waiver**：保留现有 Tier B 组 waiver，解析为具体的 Tier B 项集合；把 plan 11:258 改为与 :652–655 一致
   （逐项 key 或绑定版本与 Source Freeze 的 Tier B 组 key，不接受跨 tier 或全局 blanket）。waiver 不写成 PASS，
   不能越过已知 FAIL，不跨版本继承。
7. **候选汇总是派生结果，不是第二份权威。** 每次 readiness 重新读取最新记录；汇总列出完整候选身份，以及逐项
   指纹、场景覆盖、观察记录摘要、origin 与 waiver 引用。旧格式人工收据缺少环境与场景字段，不自动导入。
8. **根因 4 的域证据。** `app/input.rs` 的诚实分类还要包括 `MATH`（工具栏 `ConvertMath`）与 `IMAGES|ASSETS`
   （粘贴），即接近全部产品域。缩窄必须给出"入口 → 调用/回调/Effect → 共享状态读写 → 验收断言"的依据，共享
   状态的写入方进入闭包；无法证明时保持宽域。显式分类守卫是附加检查，运行时遇到未分类文件仍保守回退到全部
   产品域。拆文件只改善职责边界，不能当作解耦证明。

### plan 修改范围（整条人工证据链）

- plan 11：:237 `MANUAL PASS` 定义（与当前项指纹兼容、场景完整、最新完成观察为 PASS，并区分 RAN/REUSED）；
  :243 旧收据规则；:258–259 waiver；:316 把"可模块化 manual observation"改为指向人工策略；
  `#module-success-ledger` 增加人工策略（记录 FAIL、FAIL 不清理、条件写入、NOT_TESTED 不写入）；:336–345
  记录器与 Windows 四段版本；环境三分类与 IME 项、M34 的绑定规则。
- `01_terminology.md`：新增 Latest Manual Observation。
- 投影：`phase-12.md` 人工项说明、`phase-14.md` 新验收项、coverage matrix。
- **单独一项，不捆绑**：若要让 `docs/plan/` 的修改只影响相关章节的模块（如 07 只影响编辑/输入项），需要另改
  plan 11 中"`docs/plan/`、`docs/acceptance-cases/` 对全部功能模块保守传播"一句。这是唯一能让 v0.1.3 这类
  "改了一章 plan"的版本不全量失效的改动，但它放宽合同传播，需单独评估。

### 成本

| 项 | 成本 |
| --- | --- |
| 人工记录类型、人工策略、readiness 分支 | 中 |
| FAIL 保留与条件写入（存储层新增） | 小到中 |
| 场景表与逐场景记录（引导与逐项两个记录器） | 中 |
| 主机事实采集（`ver`、`reg query`、字体目录清单） | 小 |
| plan 11、术语、投影文档 | 中 |
| 根因 4 第 1 步：分类诚实化 + 守卫 | 小 |
| 根因 4 第 2 步：拆分 `input.rs` | 中；下一个候选全量重跑；收益只在职责边界 |
| 根因 4 第 3 步：逐项缩窄 | 证据成本高，按上文分析收益可能接近 0 |

### 选项与建议

- **选项 1（完整）**：上述修正后的根因 3 + 根因 4 全部三步。
- **选项 2（最小，建议）**：根因 3 按修正后的设计实施（产品域保守取全部产品域，IME 项与 M34 绑定 ZIP）；根因 4
  只做第 1 步。`input.rs` 拆分以后按职责边界单独做，不以复用为理由；逐项缩窄等有低成本的证据方法再议。

建议选项 2。理由：正确性优先；有时效性的是**记录格式**——下一次完整人工基线（无论落在 v0.1.3 重建还是
v0.1.4）只有按新格式记录，之后才可能复用；而缩窄产品域的收益小、证明成本高。

分批（选项 2）：根因 4 第 1 步 → 根因 3（plan/术语/投影文档一批，工具实现一批）。每批经 gpt-6-astra 审查后
提交并推送。

## 修订（第 2 轮审查后，2026-10-10）

第 2 轮：第一轮 7 项中 5 项判为已修、2 项部分修复；新增 2 个 BLOCKER 与若干 SHOULD_FIX，核实后全部成立：
字体按内部 family 名解析（`fonts.rs`；fontdb 0.23.0 `load_system_fonts` 读取 `%SYSTEMROOT%\Fonts` 与用户
`AppData\Local`、`AppData\Roaming` 下的 `Microsoft\Windows\Fonts`，cosmic-text 0.19.0 按系统 locale 选择回退），
文件名+大小不是内容身份；产品主题取 winit 的有效主题（`lifecycle.rs:86`），不是两个注册表偏好值；plan 11:319、
:433、`docs/acceptance-cases/AGENTS.md:16` 与 `phase-12.md` 前言仍写着"当前提交/exact candidate 收据"。
本节修正第 1 轮修订；冲突处以本节为准。

### 环境模型：被测环境、目标环境、readiness 所在机器三者分开

第 1 轮修订把"readiness 时与当前主机比较"作为环境判定，这是错的：开发机、Clean VM、前一个 Windows 版本的
机器各自产生合法观察，不可能同时等于执行 readiness 的那台机器。修正为：

1. **被测环境身份**：记录器在被测机器上采集，随观察写入，并在提交时重新采集比对（不一致即拒绝登记）。字段：
   Windows 功能版本与 edition（`reg query` 读 `DisplayVersion`、`EditionID`）、四段构建号（`cmd /d /c ver`，
   恰好四个数字段）、系统 locale、高对比度状态、**字体内容摘要**（fontdb 读取的三个目录下全部字体文件，按排序后
   的相对路径流式计算 SHA-256，不整份读入内存；本机约 618 MB + 97 MB，实现时实测耗时）。任一字段采集失败、
   超时或格式不符时，这次观察不能登记为有效 PASS。
2. **目标环境档案**：每次 Source Freeze 后，观察者在每台目标机器上运行一次档案采集，生成绑定该 Source Freeze
   的档案收据（字段同上）。这份收据就是"本次发布要求覆盖的环境"的当前陈述；需要哪些档案由合同决定
   （见下文待决事项），不能从已有 PASS 反推。
3. **readiness**：对每个"项 × 必需档案"，有效当且仅当存在指纹兼容、场景完整、被测环境身份等于该档案本次
   采集身份、状态为 PASS 的最新观察。readiness 在哪台机器运行不影响判定。

因此 Windows 打补丁后，下次档案采集的身份不同，该档案下的项需要重测；没打补丁、没装卸字体、没改 locale
时可以复用。

### 不复用的项（绑定当前候选 ZIP）

- 断言 IME 行为的 7 项：M01、M02、M05、M18、M22、M36、M41（第 2 轮确认清单与 `phase-12.md` 一致）。
- M34：ZIP 只证明制品身份，SmartScreen 信誉随下载历史变化；同字节重新晋升也重测。
- M42：RDP 客户端版本与连接设置不在采集范围内。

其余 15 项可按上面的环境模型复用。

### 主题

M23 的场景是 Light、Dark、System 与 System 下的运行时切换（两个方向）及实际结果；最终停在哪个主题不影响覆盖。
其他视觉项把主题作为前置条件（Light，非高对比度）记录在场景里。注册表的主题偏好值只作观察元数据。高对比度
开启时该次观察不能登记为有效 PASS（合同没有覆盖高对比度）。

### 逐项场景表

"必需场景"全部完成且 PASS 才能登记 PASS；任一场景 FAIL 立即登记该项 FAIL。场景参数由观察者声明，来源标为
"声明"。

| 项 | 必需场景 | 前置条件 | 合同来源 | 复用 |
| --- | --- | --- | --- | --- |
| M01 | 微软拼音 × 100/150/200% | Light | plan 07 Verification；功能由 G4-06 自动化 | 否 |
| M02 | WeChat/WeType × 100/150/200%；环境缺失记 NOT TESTED | Light | 同上 | 否 |
| M05 | Alt+Tab 离开后分别经点击、托盘、传感区恢复并输入 | — | plan 09 Tool Window/焦点/IME | 否 |
| M11 | 真实 mixed-DPI 双屏，左 dock：失焦收起、传感区展开、DIP/物理换算 | 双屏 mixed DPI | plan 09 dock、plan 11 :608 | 是 |
| M12 | 同上，右 dock | 同上 | 同上 | 是 |
| M18 | 220×120 Source 输入与滚动，caret/selection/IME 不被遮挡 | — | plan 09 220×120 | 否 |
| M19 | 220×120 Preview 滚动、选择、链接 | — | 同上 | 是 |
| M20 | 220×120 Split 输入与两栏滚动 | — | 同上 | 是 |
| M21 | 50/100/300% × Source/Preview/Split；滚轮缩放与 reset | — | phase-12 M21 | 是 |
| M23 | Light、Dark、System、System 下双向运行时切换 | 非高对比度 | plan 09 主题 | 是 |
| M24 | 代表性 Markdown fixture 的 Preview 视觉与选择/链接 | Light | phase-12 M24 | 是 |
| M25 | 正确与错误公式 fixture | Light | phase-12 M25 | 是 |
| M26 | PNG/JPEG/WebP/GIF、超限图片、视口下方图片的滚动显示 | Light | phase-12 M26 | 是 |
| M34 | Clean Windows 11 VM 解压运行、无额外 runtime、信誉提示与 README 一致 | Clean VM 基线（声明） | plan 11 | 否 |
| M35 | 双屏同 DPI：拖动、dock、托盘恢复 | 双屏同 DPI | plan 09 双显示器 | 是 |
| M36 | 双屏 mixed DPI：拖动、dock、IME、Preview | 双屏 mixed DPI | 同上 | 否 |
| M37 | 运行中断开当前显示器后恢复窗口 | 双屏 | plan 09 拔线 | 是 |
| M38 | 125%：输入、Preview、dock | 单屏 125% | plan 11 :91 DPI | 是 |
| M39 | 150%：同上 | 单屏 150% | 同上 | 是 |
| M40 | 200%：同上 | 单屏 200% | 同上 | 是 |
| M41 | 睡眠恢复后输入、保存、dock | — | plan 09 sleep/resume | 否 |
| M42 | RDP 重连后输入、保存、dock；无环境记 NOT TESTED | RDP | plan 09 RDP | 否 |
| M43 | 副屏在左（x<0）与在上（y<0）各一次：恢复、dock、比例保存 | 物理负坐标布局 | plan 09 双显示器左侧/上方 | 是 |

场景表放在工具内（带 `plan_ref`），属于 harness 输入；改动场景表即使相关项失效。

### 生命周期（补全）

- 状态分四种：`Absent`（无记录）、`Pass`、`Fail`、`Invalid`（记录存在但 evidence 不可读、摘要不符或校验失败）。
- 当前指纹 × 当前档案下的 `Fail` 与 `Invalid` 都阻断，且优先于 waiver 与 Tier C；`Invalid` 不回退到更早的 PASS，
  不当作 NOT_TESTED。恢复方式只有一次正式重测。
- 其他指纹或其他档案下的历史 FAIL：显示为历史，不阻断当前判定，也不称为已修复；P0/P1 问题仍走独立的发布阻断。
- 只有登记 PASS 要求场景完整；任一场景观察到 FAIL 立即登记该项 FAIL，之后中断不会丢失它。
- 开始观察与提交时都核对 Source Freeze、候选、项指纹与被测环境身份；任一变化即拒绝登记。条件写入冲突时必须
  重新读取当前记录并重新观察，不自动刷新版本后重交旧答案。
- 正式人工重测与普通诊断分开：前者写入账本，后者不写（plan 11 :433 需相应修改）。

### plan 修改范围（追加）

除第 1 轮修订所列，还需修改：plan 11 :319（readiness 接受的证据类别加入人工观察）、:433（正式人工重测与诊断的
区别）、Owned Objects/Outputs 与人工操作说明；`docs/acceptance-cases/AGENTS.md:16`；`phase-12.md` 前言；
新增目标环境档案收据。自动化的 Last Successful Module Receipt 术语与失败策略保持原义。新人工证据校验器与
场景表属于全局或 harness 输入，不能归入"只做展示"的排除项。汇总不能单独授权 READY，必须能从记录引用重新验证。

### 待 USER 决定

1. **plan 11:91 的"当前与前一个受支持 Windows 11 版本"**。工具目前只记一个 Windows 构建号，从未执行这一要求。
   - (a) 按合同建模：必需档案 = 当前版本 + 前一个版本（按功能版本与 edition 区分，不是两个补丁号）；缺档案时
     NOT_READY，除非 USER 对该档案给出绑定版本与 Source Freeze 的明确 waiver。需要第二台机器或 VM。
   - (b) 修改合同：前一个版本改为可选。
   建议 (a)：不放宽合同，缺口以 waiver 显式可见，与现有发布特例的做法一致。
2. **选项 2 是否采纳**（维持第 1 轮修订的建议）。

## Resolution（2026-10-10）

USER 选择：plan 11:91 按 **(a)** 建模（当前与前一个受支持 Windows 11 版本均为必需目标环境档案，缺档案时
NOT_READY，除非 USER 对该档案给出绑定版本与 Source Freeze 的明确 waiver）；采纳**选项 2**（根因 3 按修正后
的设计实施，产品域保守取全部产品域；根因 4 只做第 1 步）。第 3 轮设计审查的结论落入实施前的最终设计。

## 第 3 轮设计审查（2026-10-10，USER 决定之后收到）

gpt-6-astra（max）第 3 轮：第 2 轮意见多数已修；新增 3 个 BLOCKER，均针对**跨候选复用**本身：

1. "本次采集"与"不复用"缺少可校验的轮次身份：Source Freeze 与 Candidate 只含内容字段，同源再次 Freeze 或同字节
   再次晋升身份相同，旧档案可以冒充新一轮采集。需要本轮资格化/晋升标识，由现场采集器读取，且不能混入可复用项
   的内容指纹。
2. 条件写入冲突仍可吞掉已观察到的 FAIL（A 发布 PASS 后 B 的 FAIL 提交被拒，B 中断，账本仍是 PASS）。冲突中的
   正式失败必须留下阻断记录，或对同一观察键单会话排他。
3. 环境采集没有绑定实际被测进程：先启动产品再改字体或 locale，开始与提交两次采集都是新环境，产品却仍用启动时
   的字体数据库。正式会话应在采集后启动并绑定被测实例。

SHOULD_FIX：locale 应为 `GetUserPreferredUILanguages`（cosmic-text 经 sys-locale 读取），不是系统 locale；
fontdb 按未排序的目录枚举加载，同 family/style/weight 时先出现者胜，排序后的内容集合抹去了这一顺序；显示适配器、
驱动与本地/虚拟会话身份未采集（产品 CPU 光栅化，但经 GDI `BitBlt` 与 DWM 合成，并使用 layered-window alpha）；
场景表漏了 M22（本提案新增的错误；补回后仍是 9 项不复用、15 项可复用）；多机器观察如何进入同一账本未定义
（存储只在同一 clone 内共享）；(a) 的版本选择（架构、servicing channel、edition、冻结日期）、项与档案的适用关系、
环境级 waiver 的政策含义未定。另指出 plan 09 手工矩阵中 Esc、手动收起与透明度 70/96/100 在 P12 没有归属，
属于**既有**缺口。NEEDS_VERIFICATION：字体摘要只读实测约 688 MiB、WSL 流式 SHA-256 约 7.5–8.8 s；若每项开始
与提交各扫一次，每环境读取量约 33 GiB，需要合并采集。

审查同时建议把根因 3 分两步：**先建立新格式与完整观察记录（仍绑定当前候选，不跨候选复用），再在上述身份问题
闭合后开启复用**。这是已批准范围内的先后顺序，不缩小范围；采纳为实施顺序：

1. 根因 4 第 1 步（本批，见下节）。
2. 根因 3-A：逐项逐场景的观察记录、目标环境档案、两个 Windows 版本的覆盖、FAIL 优先与四种状态，**仍绑定当前
   候选**；补回 M22，并处理 Esc/手动收起/透明度档位的归属。
3. 根因 3-B：轮次身份、进程绑定、冲突下的失败持久化、locale 与字体加载顺序、显示栈身份、多机器导入，闭合后
   开启跨候选复用。

## 根因 4 第 1 步实施记录（2026-10-10）

- `fingerprint.rs` 的产品域改为三张显式表：`EVERY_SESSION`（每次会话的启动路径或事件循环每轮执行的代码，归全部
  产品域）、`NOT_YET_NARROWED`（此前落入兜底、尚未追踪闭包的文件，保持全部产品域）、`FEATURE_SOURCES`（只经某个
  功能到达的代码与其域）。没有显式规则的产品文件运行时仍归全部产品域。
- 新守卫 `every_product_source_has_an_explicit_rule`：真实 checkout 中落入兜底的产品文件与不再命中任何 tracked 文件
  的规则都使测试失败；两种变异（删一条规则、加一条不存在的规则）均实测失败。
- **只放宽不缩窄**：172 个产品相关文件逐一比较前后掩码，没有任何文件的掩码失去位；71 个文件掩码变宽。对四个窄模块
  新增的失效：
  - 16 个窗口文件（`app/{controls,toolbar_paint,window_geometry_runtime,window_interaction,window_runtime}.rs`、
    `flow/window/*`、`platform/windows/{monitor,native_message,tool_window,tray,window_opacity,window_topmost}.rs`）
    新增使 Performance 与 G3 失效——它们在启动路径上（例如 `window_runtime.rs` 记录 `guards_ready` 启动诊断，
    `lifecycle.rs` 启动时创建托盘、应用透明度与 Tool Window 样式），原先只归 `SHELL`；
  - 16 个启动、note 加载与持久化文件（`startup/*`、`persistence/*`、`flow/persistence.rs`、`*_runtime.rs` 中的持久化
    胶水、`platform/windows/{file_watch,program_dir,single_instance}.rs`）新增使 G5 失效——G5 渲染用例写入 note 后由
    `startup/bootstrap.rs` 经 `persistence` 加载；
  - `app/export_runtime.rs` 新增使 Performance 与 G4 失效（它是 `StickyApp` 胶水，`export_in_flight` 被窗口守卫读取）。
  其余变宽（如 `app/input.rs` 从 `EDITOR` 到全部产品域、剪贴板加 `ASSETS|IMAGES`、`atomic_file.rs` 加 `EXPORT`）不改变
  现有模块判定，但对日后的更细消费者是诚实的。
- 模块掩码核对：导出是保留源文本的 Markdown，只经 `render/preview` 的图片引用改写（该目录掩码含 `EXPORT`），不经数学
  渲染；G3 掩码不含 `MATH` 成立，未修改任何模块掩码。
- plan 11 `#module-success-ledger` 增加分类原则一段；`phase-14.md` 新增 P14-A72；coverage matrix 与治理范围同步到 A72。
- 验证：Windows `cargo test -p stickymd-smoke --locked` 396 + 25 通过（17 ignored），clippy `-D warnings`、fmt、
  phase-00 治理通过；WSL Debian 回放 Linux CI：clippy 通过，326 + 19 通过。

### 代码审查第 1 轮（2026-10-10）

gpt-6-astra（max）复现了"172 个文件、71 个变宽、0 个丢位"与 `domains()` 不变，但给出 2 个 BLOCKER、2 个 SHOULD_FIX，
核实后全部成立并已修正：

- BLOCKER：`flow/{save,recovery,reconciliation}.rs` 只归 `PERSISTENCE|STARTUP`，与 G5 无交集；但 `StickyApp::new`
  创建 `RecoveryCoordinator`，事件循环每轮调用 `tick_autosave`，G5 粘贴用例等待 note 落盘后又经 reconciliation。
- BLOCKER：`atomic_file.rs` 与 G5 无交集；`config/storage.rs` 的 `save_config` 经 `atomic_publish` 写配置，G5 切换视图
  后等待配置落盘。
- SHOULD_FIX：只改产品源码的定向 CI 跳过 smoke 包测试，守卫不一定运行。改为由 `phase 00` 治理调用
  `verify_product_classification`（CI plan 作业每次运行），单元测试调用同一函数；实测在未分类产品文件出现时
  `phase 00` 失败。
- SHOULD_FIX：`flow/editor.rs`、`interaction/search.rs` 在 `app.rs` 启动时初始化，`flow/preview.rs` 启动时 `show`、
  每轮 `tick`，`assets/{storage,safe_boundary}.rs` 在 `main.rs` 启动时执行，caret overlay 在默认 Source 首帧后使用；
  这些已移入 `EVERY_SESSION`。`app/` 下只处理导出等功能的文件仍归全部产品域，注释改为如实说明理由：它们共享
  `StickyApp` 可变状态，每轮代码读取（如 `export_in_flight`、`asset_paste_pending` 进入窗口守卫）。

修正后对四个窄模块新增的失效：Performance 17、G3 16、G4 1、G5 20 个文件（172 个文件中 81 个掩码变宽，0 个丢位）。
新增回归 `every_session_persistence_invalidates_every_module`：note 加载、恢复检查、autosave、reconciliation、配置存储、
原子发布与资产存储文件都必须使每个模块失效。plan 11 分类段落与 P14-A72 同步补入编辑后的持久化与治理检查。
验证：Windows 397 + 25 通过，clippy、fmt、phase-00 通过；WSL Linux 回放 clippy 通过、327 + 19 通过、phase-00 通过。

### 代码审查第 2 轮（2026-10-10）

第 1 轮修复全部属实，复算仍为 81 个变宽、0 个丢位。1 个 BLOCKER（自 HEAD 遗留）：正式 Performance 计划按名称过滤运行
Release 单元基准，其中 Phase 7 的 `phase7_export_release_baseline` 位于 `export/mod.rs` 并调用 `export_snapshot`，但
Performance 掩码不含 `EXPORT`，只改导出代码时旧 Performance 记录仍可复用。修正：Performance 掩码加入 `EXPORT`
（导出目录因此新增使 Performance 失效，合计 18 个文件）；新增回归 `performance_baselines_outside_its_features_invalidate_it`。
逐一核对含 Performance 基准的产品文件，修正后掩码均与 Performance 相交。其余结论：剩余 `FEATURE_SOURCES` 与各模块
实际执行路径一致（G3 五个用例都不切换 Preview/Split，导出不经数学排版；Performance 的 typical seed 含行内公式但以
Source 视图启动，不做数学排版）；治理调用 `git ls-files` 与 CI plan、release 工作流兼容，没有 Git 元数据的源码副本会
失败，现有工作流不使用这种环境。

未做：按 Performance 任务的名称过滤自动推导"含基准的文件必须与掩码相交"的通用守卫；当前以回归列出已知文件。

验证：Windows 398 + 25 通过，clippy、fmt、phase-00 通过；WSL Linux 回放 clippy 通过、328 + 19 通过、phase-00 通过。

## 根因 3-A 设计稿（2026-10-14，待审查）

范围：人工验收的**记录模型**——逐项逐场景、目标环境档案、两个 Windows 版本的覆盖、FAIL 优先、多机器导入。
观察仍绑定当前候选（source/EXE/ZIP/version），**不做跨候选复用**；字体、locale、显示栈身份、轮次身份、进程绑定与
条件写入属于复用（3-B），本批不实现。理由：3-A 中环境身份只用于判定"这条观察覆盖了哪个目标 Windows 版本"，
功能版本、edition 与架构不会在一次会话内变化；而字体/locale 只影响"旧观察能否代表新环境"，那是 3-B 的问题。

### 已核实的现状

- phase-12 共 24 个 Manual 项（14 Tier A、7 Tier B、3 Tier C）；P12-M44 是 exact 自动化项，不在人工集合中。
- Esc 收起与手动收起由 Phase 8 reducer 自动化持有（P08-A05、P08-D058..D060）；透明度 40/70/96/100 视觉是
  P08-M06，P12 只有 M22 的 40%。plan 09 Verification 把它们笼统列为"手工矩阵"，与现行分工不一致。
- 全工作区（含 smoke 工具）以 `+crt-static` 构建，工具二进制可在未装 VC runtime 的机器上运行。
- 现有收据单文件 `dist/evidence/manual-acceptance.json`（schema 1），每项一个状态，Windows 构建只记一个字段。

### 设计

1. **用例与场景注册表**（工具内，`plan_ref` 指向 plan 11）：24 项与 phase-12 的 Manual 集合精确相等（治理检查），
   每项列出必需场景（第 2 轮修订的场景表，补回 **M22：透明度 40/70/96/100 × 输入、预览、IME**，吸收 P08-M06 进入
   发布矩阵）。plan 09 Verification 改为区分：Esc/手动收起归 Phase 8 自动化，透明度四档归 M22。
2. **目标环境档案**：两个角色 `current`、`previous`。Source Freeze 之后由 USER 以交互命令声明一份目标收据（绑定
   Source Freeze 身份），每个角色写明 Windows 11 功能版本（如 `25H2`）、edition 族（Home/Pro）与架构（x64）。plan 写明
   选择规则：冻结日期时、面向通用设备的 General Availability Channel 中仍在服务期的最新与次新功能版本，排除只面向
   特定新设备的分支；工具不联网，不自行推断。
3. **观察记录**（收据 schema 3，仍是同一文件、仍绑定当前候选）：只追加的观察日志，每条为
   `{seq, case, scenario, role, status ∈ {MANUAL_PASS, MANUAL_FAIL}, note, observed, declared, session}`。
   `observed` 由记录器在被测机器上采集：`DisplayVersion`、`EditionID`（`reg query`）、四段构建号（`cmd /d /c ver`，
   恰好四个数字段）、架构；任一采集失败则拒绝登记。`declared` 为场景参数（DPI、拓扑、主题等），标注来源为声明。
   观察的 `role` 由 `observed` 与目标收据匹配得出，不由观察者选择；匹配不到任何角色则拒绝登记。
4. **状态推导**（每个 `(项, 角色)`）：`Absent`（无观察）→ NOT_TESTED；某场景最新观察为 FAIL → `Fail`；全部必需场景
   最新观察为 PASS → `Pass`；日志损坏或身份不符 → `Invalid`。FAIL 在该场景观察到时立即追加，之后中断不会丢失；
   只有该场景之后的 PASS 才覆盖它。NOT_TESTED、跳过、中止从不写入。
5. **并发**：同一工作区同一时刻只允许一个记录会话（`dist/evidence/` 下的 OS 排他锁，进程退出即释放）；追加在锁内
   完成并原子替换收据。3-A 只有一个写入入口（本地记录或导入），因此不需要条件写入。
6. **多机器**：其他机器（前一 Windows 版本、Clean VM）用工具二进制的**观察包**模式记录：只需候选 ZIP，不需要仓库；
   候选身份由本地计算的 ZIP/EXE 摘要给出，包内含目标收据摘要、采集环境与观察。主工作区 `import` 在锁内严格校验
   （schema、候选身份与当前候选一致、目标收据摘要一致、项/场景已知、无重复 seq），按包内顺序追加。M34 在 Clean VM
   中先完成 StickyMD 首次运行的观察，**之后**才复制并运行工具记录（工具是静态链接的单文件，不安装 runtime）。
7. **readiness**：每个 `(项, 角色)`：`Fail`/`Invalid` 阻断，且优先于 waiver 与 Tier C；`Absent` 按 tier 与 waiver 处理；
   全部 `Pass` 为 `MANUAL PASS`。waiver：保留逐项 key 与 Tier B 组 key；新增档案级 key
   `WAIVER-PROFILE-PREVIOUS-v{version}`（绑定 Source Freeze），只把 `(项, previous)` 的 `Absent` 处置为 `USER WAIVED`，
   不覆盖 FAIL/Invalid。汇总逐项逐角色列出状态、场景覆盖与 waiver 引用。
8. **引导记录器**：一步覆盖多项时逐项询问；一项多场景时逐场景询问。
9. **迁移**：schema 1 收据缺少场景与角色，不导入；readiness 报告其已被取代。

### 待 USER 决定（GPT 审查后带建议提交）

- **项 × 角色的适用关系**：(i) 全部 24 项在两个角色上都做（严格读法，人工时间约翻倍）；(ii) 全部项做 `current`，
  `previous` 只做依赖 OS 集成的子集（需在 plan 中逐项列出）。注意产品字体来自系统字体，纯渲染项也随 Windows 版本
  变化。
- **档案级 waiver 的结论**：缺 `previous` 时以 `USER WAIVED` 计入 READY（与现有逐项 waiver 一致），还是保持技术
  `NOT_READY`、走发布特例。

### plan 修改（3-A 部分）

plan 11：:91 角色与选择规则；:237 `MANUAL PASS` 定义改为逐项逐角色、场景完整；:258 waiver（Tier B 组 key 与档案 key）；
:336–345 记录器（场景、逐项、会话锁、观察包导入、Windows 四段）。`01_terminology.md`：Target Environment Profile、
Manual Observation。plan 09 Verification 分工。`phase-12.md`：M22 场景。`phase-14.md`：新验收项。复用相关的 :243、
:316 与 `#module-success-ledger` 人工策略留给 3-B。

### 3-A 设计审查第 1 轮与修订（2026-10-14）

gpt-6-astra（max）：5 个 BLOCKER、3 个 SHOULD_FIX、1 个 NEEDS_VERIFICATION，核实后全部成立。修订如下（与上节冲突处以
本节为准）。

1. **迟到的观察包不能清除较新的 FAIL。** 每条观察有稳定 ID（`session_id + local_seq`，重复导入幂等）与 `base`：会话开始
   （或主工作区导出观察任务）时日志的登记长度。某场景的 PASS 只有在 `base` 已包含该场景的最新 FAIL 时才覆盖它——即观察者
   看到过这个失败并做了正式重测；否则 FAIL 保留，状态为 `Fail`。不需要分布式锁或向量时钟。
2. **上下文身份。** 日志头绑定 candidate receipt 摘要（含 workflow run/attempt/artifact id；新一次构建即新身份）、目标收据
   摘要与场景注册表摘要。目标收据含随机上下文 ID，日志存在后不可改；要改目标必须归档当前日志、建立新上下文，旧 waiver
   不继承。同一 artifact 原样重新晋升得到同一 candidate，不视为新一轮（3-A 不建模信誉等随时间变化的外部状态；第 2 轮修订
   中"M34 同字节重新晋升也重测"属于 3-B 的复用规则）。
3. **目标匹配谓词。** 目标收据逐角色记录：选择日期与微软依据（release health 表的版本与日期）、功能版本、对应构建分支
   （如 25H2 ↔ 26200）、允许的 `EditionID` 白名单（具体 SKU，不用 Home/Pro 这类政策标签）、原生架构。采集：
   `DisplayVersion`、`CurrentBuild`、`UBR`、`EditionID`、原生 `PROCESSOR_ARCHITECTURE`（`reg query`）、Insider 登记状态
   （`WindowsSelfHost\Applicability`）。角色当且仅当功能版本、构建分支、SKU、架构全部相符且未登记 Insider 时匹配，且
   必须唯一；`DisplayVersion` 缺失或匹配不唯一即拒绝登记。导入与 readiness 重新计算角色。审查查得按微软当前发布表，
   2026-10-14 的两个目标应为 26H2 与 25H2——这是 USER 声明时的事实，工具不联网推断。
4. **完整的状态推导**（每个 `(项, 角色)`）：证据无效 → `Invalid`；任一必需场景的有效最新观察为 FAIL → `Fail`；全部必需
   场景 PASS → `Pass`；其余 → `NOT_TESTED`，附已覆盖/缺失场景（零覆盖与部分覆盖只是元数据）。waiver 处置的是未完成的
   `(项, 角色)`，不删除已有 PASS。
5. **覆盖映射，而不只是 24 项。** 每条 plan 07/09/11 人工断言与每个 Phase 8 人工行（P08-M01..M15）必须映射到 P12 人工项
   的场景、或已批准的 exact 自动化入口，或经批准注明退役。逐行核对的结果：
   - 已覆盖：P08-M01/M02（M11/M12 + M38–M40）、M04/M13（P14-A30、G4-06 + M01/M02）、M05 的托盘菜单与生命周期
     （P12-M06..M09，G4-01）、M07（M23）、M08（P12-M44，G4-05）、M11（M36、M43）、M14（M24、M25、M23、M38–M40）、
     M15（M26、M28–M30）。
   - **缺口**（早于本提案）：顶部 dock 视觉（P08-M03 的视觉部分；时序由 M10/G4-02 自动化）；Alt+F4 隐藏到托盘
     （P08-D031，无自动化）；两份复制目录互相独立（P08-M09）；主显示器切换与上下排列（P08-M10）；Windows 11 外壳视觉
     契约——圆角、阴影、置顶按钮、控件淡化、自定义缩放、辅助功能（P08-M12）；DPI/主题下滚动条可抓取且不遮字
     （plan 09）；真实 Esc 与按钮收起（reducer 由 P08-A05 自动化，但真实键盘入口在 composition 时先取消 preedit 并返回，
     不进入窗口 intent，二者不等价）。
   - 处理：Esc/按钮收起（含 composition 中按 Esc 只取消 preedit 的预期）并入 M11/M12；顶部 dock 视觉并入 M38–M40 的
     dock 场景；滚动条并入 M23 与 M38–M40；主显示器切换与上下排列并入 M35；Alt+F4 并入 M05；新增两项 Manual：
     **外壳视觉契约**与**两份便携副本互相独立**（新 ID 接在 P12-M44 之后）。M22 吸收 P08-M06 的全部断言（背景、文字、公式、
     图片、控件、阴影与滑块/数字输入提交）。治理检查"每个 P08 人工行都有映射"。
6. **观察包与被测实例。** 主工作区导出观察任务（候选 receipt 摘要、目标摘要、场景注册表摘要、`base`）；独立模式校验 ZIP
   及其中 EXE 与任务一致，导入复用现有 candidate 校验路径。记录器把已校验的 ZIP 解压到会话目录、按场景放入 fixture，
   自行启动该 EXE，并在观察中记录 EXE 摘要与进程 ID；观察者只观察这个实例。M34 例外：Clean VM 中先完成首次运行观察，
   之后才运行工具，观察标为人工声明并记录 VM 基线与包的取得方式。威胁模型如实写明：信任 USER，不防伪造观察。
7. **锁与生命周期。** 独立且不被替换的 sidecar 锁 `dist/evidence/manual.lock`（每个 worktree 各一份；`dist/evidence/` 本就
   按 worktree 隔离），覆盖"读旧状态到原子提交"；记录、导入、目标声明取排他锁，readiness 取共享锁；等待有上限。FAIL 在
   可选备注之前落盘；写失败非零退出；观察包整体校验后一次提交。日志损坏为 `Invalid` 并阻断；重建日志需要 USER 决定 key
   `RESET-MANUAL-LOG-<digest>`，汇总中可见。上限：观察包 1 MiB、单个日志 10 000 条、备注 2 KiB。
8. **plan 修改补全。** 本批就在 plan 11:316 处说明人工观察是独立的证据类别（含 FAIL，绑定当前候选），不是 Last Successful
   Module Receipt；同步 Owned Objects/Outputs、readiness 接受规则、上下文失效、失败恢复、引导记录、waiver 范围；plan 09
   Verification 的分工；Phase 8→12 映射；`phase-12.md` 前言、全部场景与两个新项；`phase-14.md`；coverage matrix。:243 的
   旧 commit 禁令在 3-A 仍成立，保留。
9. **待实测**：实际分发的工具 EXE 的导入表，以及在没有仓库、Rust、VS、额外 VC runtime 的 VM 中运行独立模式。

审查对两项 USER 决定的意见：(i) 两个角色都覆盖完整注册表（仍按 tier/waiver 规则处置，不是 48 项无条件 PASS）；档案级
waiver 计入 READY 并在汇总中逐项列出 `USER WAIVED`，同时写明这是"发布资格满足批准政策"而非"技术验收全部完成"；目标
收据缺失、损坏或含糊属于 authority 无效，不能用同一 waiver 绕过。

### 3-A 设计审查第 2 轮与修订（2026-10-14）

第 2 轮：只追加日志内的 `base` 规则、部分完成状态、候选身份（同一原始 artifact 原样重新晋升视为同一 candidate）已成立；
新增 4 个 BLOCKER、2 个 SHOULD_FIX，核实后全部成立。不涉及政策的部分修订如下：

1. **日志重建即新上下文。** 任何清空、重建或丢失日志后的恢复都生成新的随机上下文 ID；旧任务、旧观察包与旧 waiver 都不
   继承。同 ID 同内容的重复导入是 no-op（不重新分配序号），同 ID 不同内容拒绝。记录器在 FAIL 落盘后提示"重新开始重测
   任务"，提交旧答案时不自动刷新 `base`。
2. **观察任务携带内容而非只有摘要**：完整的候选 receipt、目标收据与场景注册表片段随任务导出，无仓库的记录器据此校验
   ZIP/EXE。
3. **按场景定义被测实例**（复用 exact harness 现有的候选校验、解压与逐用例副本设施）：同目录单实例场景用同一 canonical
   目录或其别名第二次启动；两份副本项登记两个目录两个进程；M05 识别被测实例的托盘项；M37/M41/M42 保留原进程与 note，
   不得靠重启或重新 seed 恢复，预期断连期间的环境探测不判产品 FAIL；M34 保存首次运行的人工声明、VM 基线与取得方式，工具
   后来启动的实例不替代首次运行事实。会话目录正常与异常退出都有清理和有界保留；日志锁不代替交互桌面独占，记录器开始前
   检查没有其他 StickyMD 实例。
4. **注册表驱动 tier 与 waiver 范围**：ID、tier、必需场景、waiver 范围都由同一注册表声明，记录器、校验与 `decisions.rs`
   的 key 校验都读取它，不再依赖 `M01..M44` 的编号范围。
5. **plan 修改补充**：`01_terminology.md` 的 Promoted Candidate lifetime 为 3-A 的同 artifact 人工上下文写出窄范围规则，
   同时保留每次 Promote 重验 exact-byte evidence；plan 11:559"任何人工观察前先检查环境"为 M34 写明例外（事后记录的
   是人工声明，不冒充事前环境证据）。
6. **"非 Insider"改为可执行的判定**：不探测 Insider 登记状态（没有一手契约保证该注册表键能识别所有通道，尤其 Release
   Preview）。目标收据为每个角色列出 USER 依据微软发布历史声明的**获准完整构建号**（`CurrentBuild.UBR`）及来源；观察的
   完整构建号必须在列表内。构建号相同即同一份系统位，与机器登记在哪个通道无关。原生架构取 `IsWow64Process2` 定义的
   原生机器类型（不用进程环境变量）。注册表读取写明路径、视图与类型；任一项读取失败即拒绝登记。代价：资格化期间机器
   打补丁后构建号不在列表内，需要新上下文。

### 待 USER 决定（第 2 轮后）

1. 项 × 角色：(i) 两个角色覆盖完整注册表 / (ii) `previous` 只做子集。审查与我都建议 (i)。
2. 缺 `previous` 档案：READY 并逐项列出 `USER WAIVED`（写明"符合批准的发布政策"而非"技术验收全部完成"）/ 保持 NOT_READY
   走发布特例。审查与我都建议前者；目标收据缺失、损坏或含糊不能用 waiver 绕过。
3. **Esc 语义（产品合同）**：plan 09 写"Esc/手动按钮无条件立即收起"，实现中 IME composition（取消 preedit）与 Search 先消费
   Esc。建议把输入优先级写进 plan 09：composition、Search、弹出控件先消费，未被消费时才进入窗口收起。
4. **覆盖缺口的处理方式**：Phase 8 人工义务映射到 P12 需要逐断言粒度（左右 dock 的 100% 与 `>16 DIP` 脱离、Right/Top dock
   的 IME 路径、保存失败时取消退出、Topmost 与托盘同步、最小化/dock 状态下第二次启动与前台激活、Alt+F4、两份副本的
   托盘区分、外壳视觉与边缘/角落调整大小等），部分可扩充 G4 自动化，其余为人工场景，可能新增两项 Tier A（外壳视觉、两份
   副本），注册表变为 26 项 × 2 角色。这些缺口早于本提案。建议**单独作为 3-A 之后的一批**（覆盖补全），3-A 先在现有 24 项
   上建立记录模型，缺口在报告与汇总中如实列出。

## Resolution（2026-10-14，3-A 范围）

USER 选择：(1) `previous` 角色覆盖完整注册表；(2) 缺 `previous` 档案时以 USER waiver 计入 READY，汇总逐项列出 `USER WAIVED`
并写明"符合批准的发布政策"，目标收据缺失、损坏或含糊不能用 waiver 绕过；(3) Esc 输入优先级写入 plan 09：IME composition、
Search、弹出控件先消费，未被消费时才收起窗口；(4) Phase 8 人工义务的覆盖补全作为 3-A 之后单独一批，3-A 在现有 24 项上
建立记录模型，缺口在报告与 readiness 汇总中如实列出。

### 3-A 文档批次起草中发现的问题（2026-10-14）

核对 Esc 的真实消费顺序（`input.rs`、`window_interaction.rs`、`preview_input.rs`）：透明度数字输入 → 恢复类模态守卫 →
Search → **Preview 焦点（Esc 只清除 Preview 选区，从不收起窗口）** → IME composition → 窗口收起。USER 批准的优先级只列了
composition、Search 与弹出控件；Preview 焦点下 Esc 永不收起窗口这一点未经批准，plan 09 草稿暂未写入，待 USER 决定：
保持现状并写入合同，或改为"有 Preview 选区时清除选区，否则收起"（需要改产品代码）。

### 3-A 设计审查第 3 轮（2026-10-14）与 Resolution

第 3 轮（两次因服务端满载失败后的第三次运行）：模型主体收敛，唯一 BLOCKER 是 Esc 行为表未闭合；SHOULD_FIX：构建号相同
不等于系统位或环境等价、注册表类型与逐场景采集时点、观察任务需携带待解除的 FAIL、所有人工 waiver 绑定上下文与
`(项, 角色)` 集合；NEEDS_VERIFICATION：独立记录器需拆开仓库校验入口并实测。另建议场景归约与登记顺序无关、readiness
始终输出已知缺口、读取前先查字节上限。均已写入 plan 11 草稿。

USER 决定（2026-10-14）：Preview 获焦时 Esc **只在有 Preview 选区时**清除选区并消费，否则继续按规则收起。plan 09 写入完整的
Esc 行为表。与现有实现相比需要两处产品改动，单独提交：透明度弹层打开但数字输入未获焦时 Esc 关闭弹层（现为不消费）；
Preview 获焦且无选区时 Esc 继续收起（现为只清除选区）。其余分支与现有实现一致（Search 字段 composition 中 Esc 由 IME
消费，见 `search_controller.rs`）。

实施分批（按第 3 轮建议）：1a 契约与投影文档；1b Esc 产品改动；2 注册表、上下文、日志、归约、readiness；3 环境采集、
被测实例与记录器；4 观察任务导出、独立记录与整包导入。每批经 gpt-6-astra 审查后提交推送。

### 3-A 第 1a 批文档审查第 1 轮（2026-10-14）

3 个 BLOCKER、7 个 SHOULD_FIX，核实后成立并已修：Esc 第 2 行改为实际的输入守卫（恢复待处理、资产对账进行中、窗口不接受
编辑输入），外部修改冲突不在此列；档案 waiver 写明是限定于 `previous` 的跨 tier 例外，旧"只绑定版本、source 与
case/group"改为引用统一规则；readiness 接受集合加入当前上下文中经归约的人工观察；`ver` 条款限定于 exact evidence；
记录、导入与 readiness 共用匹配谓词并重算角色；M34 例外覆盖开始采集与启动归属，事后采集与人工声明分开；plan 04 的
收起优先级与 AC-022 改为指已送达的窗口 intent 并引用 plan 09 的 Esc 顺序；schema 1 收据保留并报告已被取代；`RESET`
摘要对象为损坏日志字节；被测实例补回保留原 note、不得重新 seed、预期断连不判 FAIL、会话目录清理与有界保留；guided
session 的 `NOT_TESTED` 只出现在会话输出中。

未采纳的修法：审查建议现在就为记录模型添加标为 `NOT TESTED` 的验收行。治理合同只允许 Automated 行为
`AUTOMATED PASS` 或 `BLOCKED`（`governance.rs` 的 mode/status 校验），未实现不等于"已知失败"；因此这些行随第 2–4 批
实现一起加入（预定 P14-A73..A77：目标收据与角色匹配、日志与归约、上下文与恢复、记录器与被测实例、观察任务与导入）。

### 第 1a 批审查第 2 轮与第 1b 批审查第 1 轮（2026-10-14）

1a：PASS / no blocker；3 个 SHOULD_FIX 已修：导出观察任务在共享锁内从同一快照取上下文、`base` 与未解除 FAIL；日志首次创建、
缺失、为空、损坏与达到条数上限的处理（缺失视为丢失，不当作首次初始化；可读日志重置时未解除 FAIL 带入新上下文）；
plan 11 增加不带状态的验证清单，coverage matrix 注明随 3-A 第 2–4 批进入 Phase 14 矩阵。

1b：1 个 BLOCKER——Preview 只把无修饰键的 Esc 识别为 Escape，Shift/Ctrl+Esc 在 Preview 焦点下被吞掉；修正为不论修饰键。
SHOULD_FIX：取消透明度弹层未结束滑块拖动（`ControlState::cancel` 现在一并结束拖动与数字编辑）；原单元断言不保护行为
变化（新增纯函数测试：弹层取消的四种焦点/拖动组合、`preview_escape` 对默认、`caret(17)`、正反向非空选区的判定，以及
修饰键）。NEEDS_VERIFICATION：Split 中焦点移到 Preview 后可能残留 Source preedit；编辑区 composition 的 Esc 判定已移到
输入守卫之后、Search 与 Preview 之前，与 plan 09 的顺序一致。验证：`stickymd-win` 268 个测试通过，clippy 通过。真实桌面
的 IME 事件顺序、修饰键投递、拖动取消与长按穿过多层状态留待覆盖补全批，不宣称已验收。

确认轮（2026-10-14）：1b PASS / no blocker；1a PASS / no blocker，2 个 SHOULD_FIX 已修——目标收据声明的同一事务写入只有头部的
零观察日志，此后缺失、零字节或损坏一律视为丢失；满额重置时未解除 FAIL 按 `(项, 角色, 场景)` 合并带入，保证重置后有追加
容量。真实桌面的 Esc 行为（IME 事件顺序、修饰键投递、拖动取消、长按跨层）仍待覆盖补全批实测。

## 3-A 第 2 批实施记录（2026-10-14）

新模块 `qualification/manual_observation/`：`registry`（24 项、tier、session、逐项场景与已知覆盖缺口；测试保证与
`phase-12.md` 的 Manual 集合相等、14/7/3）、`target`（目标收据严格解析与随机上下文 ID）、`role`（匹配谓词）、`log`
（日志格式、上限与幂等追加）、`reduce`（与登记顺序无关的场景归约，以及重置时的未解除 FAIL 计算，两者共用同一规则）、
`store`（sidecar 锁、带上限读取、声明、重置、合并带入与归档；写入顺序先日志后目标收据）、`waiver`（key 与
`(项, 角色)` 集合的一致性）。`decisions` 的 waiver 记录增加上下文与集合，`--manual-scope` 写入时校验，readiness 读取时
再校验；`RESET-MANUAL-LOG-<digest>` 成为可记录的决定。`manual_readiness` 改为读取上下文、归约、按 tier 与 waiver 处置，
readiness 普通输出与 JSON（schema 3）都包含逐项逐角色汇总、旧格式提示与已知覆盖缺口。成功账本的有界锁等待抽为共享
`file_lock`（列为全局输入）。schema 1 收据模块删除，文件保留并报告已被取代。

过渡状态（如实说明）：观察记录器随第 3 批到来，本批期间 `acceptance manual run/guided` 拒绝执行，而不是继续写已被
取代的收据；`list`、`status`、新增的 `target`（USER 交互声明）与 `reset` 可用。被测机器上的环境事实采集也在第 3 批。

验证：Windows `cargo test -p stickymd-smoke --locked` 全部通过，clippy `-D warnings`、fmt、phase-00 通过；WSL Linux 回放
clippy 通过、356 + 19 通过、phase-00 通过。新增验收行 P14-A73..A75。

### 第 2 批代码审查第 1 轮（2026-10-14）

第一次运行中断、未写报告（按约定重试一次）；中断前指出 reset 先移走活动文件再写新文件，可能被当作首次初始化——已改为
复制归档、活动文件保留到新日志与新目标依次替换，任何仍可解析的日志都携带未解除 FAIL。重试结果：3 个 BLOCKER、6 个
SHOULD_FIX、1 个 NICE_TO_HAVE、1 个 NEEDS_VERIFICATION，核实后全部成立并已修：

- BLOCKER：reset 的未解除 FAIL 计算不验证环境，一条无效 PASS 就能清掉有效 FAIL。现在归约与 reset 共用 `invalidity`；
  reset 只有在日志确实绑定到某个目标时才让有效 PASS 解除 FAIL，否则携带全部 FAIL。
- BLOCKER：决定收据的读改写没有锁，并发更新可恢复已撤销的 waiver；reset 在取锁前读取批准。现在决定收据有独立
  sidecar 锁，所有写入是锁内读改写；锁顺序固定为"人工存储锁先于决定锁"；`declare`/`reset` 在存储排他锁内调用时才
  读取批准；readiness 在两把锁内取得上下文与决定的一致快照。
- BLOCKER：readiness 先读 candidate、人工检查再读一次文件算摘要，可能混用两个 candidate。现在 `read_candidate_snapshot`
  从同一次读取的字节同时给出 candidate 与摘要，readiness 与人工命令都只用它。
- SHOULD_FIX：携带的 FAIL 在目标变化后被判无效、无法解除 → 携带的 FAIL 保留原角色（只能阻断，且日志解析要求携带项
  必为 FAIL），之后看到它的有效 PASS 可以解除。超过字节上限的日志无法授权重置 → 重置摘要改为对文件流式计算，写入
  前检查新日志大小。目标收据接受 `Home`/`Pro` 与不存在的日期 → 拒绝这两个政策标签并校验真实日历日期。readiness
  缺少重置来源、已覆盖场景与"USER WAIVED 只表示符合批准政策"的说明 → 均写入普通输出与 JSON。账本错误会跳过人工
  汇总输出 → 改为先打印账本错误再始终打印人工汇总；无 candidate 时也报告旧收据。过渡状态未同步 → 操作指南加过渡
  说明，P12-A10 与 P14-A12 改写证据，新增"`run/guided` 拒绝且不写文件"的测试。
- NICE_TO_HAVE：批量追加的 ID 去重改为一次建索引（线性）。
- NEEDS_VERIFICATION：新增有界等待超时测试（共享 `file_lock`），以及 Windows 上真实的写入失败注入（目标收据只读，
  替换在日志之后失败）：结果为阻断状态，旧目标收据字节不变，批准后重新声明仍带入 FAIL。

验证：Windows `cargo test -p stickymd-smoke --locked` 432 + 25 通过，clippy、fmt、phase-00 通过。

### 第 2 批代码审查第 2 轮（2026-10-14）

第 1 轮修复经源码核对成立；新增 2 个 BLOCKER、2 个 SHOULD_FIX，核实后已修：

- BLOCKER：readiness 读两次决定（发布门一次、waiver 一次），交错的撤销与批准可产生从未成立过的 READY。现在 readiness 先取
  人工存储共享锁，在锁内只读一次决定，发布门与人工政策共用这一份快照；人工检查不再自行读取决定（回归测试：磁盘上没有
  决定收据时，调用方提供的 waiver 仍生效）。
- BLOCKER：公共证据写入入口可覆盖人工日志、目标收据、归档、sidecar 锁与决定收据。现在 `validate_public_evidence_path`
  经别名解析拒绝这些路径（测试含 `./` 别名与归档子路径，且被拒后日志字节不变）。
- SHOULD_FIX：readiness 收据写入失败会跳过人工汇总 → 先打印汇总再传播写入错误。reset 把决定读取或锁超时误报为"需要
  批准" → 批准回调返回 `Result<bool, String>`，读取或锁错误原样上报（测试）。

验证：Windows 435 + 25 通过，clippy、fmt、phase-00 通过。readiness 写入失败时的输出顺序由代码结构保证，未做输出捕获测试。
Linux 回放发现 `shared_readers_coexist_and_exclude_a_writer` 偶发失败（Unix flock 属于打开的文件描述，其他测试 fork 的子进程
在 exec 前短暂持有）；释放后的获取改为有界重试，锁超时测试释放后的等待放宽到 5 s。Linux 连续三次 364 + 19 通过、
phase-00 通过。

### 第 2 批代码审查第 3 轮（确认轮，2026-10-14）

单快照与锁顺序（三条路径 manual S→decision S、manual S→decision X、manual X→decision S 无环）、错误传播与公共证据入口
的路径保护经核对成立。新增 2 个 BLOCKER、1 个 SHOULD_FIX，已修：

- BLOCKER：release 的 checksums、SBOM、package 发布与 notices 等入口按调用方路径直接写，可覆盖人工存储与决定收据。
  `release::execute` 在分派前对所有调用方指定的输出文件复用 `validate_public_evidence_path`，对输出目录用新的
  `validate_public_output_directory`（拒绝成功账本存储与人工归档）；回归测试覆盖四类入口，被拒后日志字节不变。
- BLOCKER：目标声明在等待 USER 输入前读取 candidate，提交时不再核对；其间 Promote 新 candidate 后，旧声明会把新
  candidate 的上下文按"另一 candidate"直接替换。`store::declare/reset` 现在在排他锁内重读 candidate 摘要，与开始时
  不同即拒绝且不改动任何文件（回归测试）。
- SHOULD_FIX：锁测试的重试循环会把 `TryLockError::Error` 当作成功 → 三种结果显式匹配。

验证：Windows 437 + 25 通过，clippy、fmt、phase-00 通过；Linux 回放见下。
Linux 回放：clippy 通过，连续两次 366 + 19 通过，phase-00 通过。

### 第 2 批代码审查第 4 轮（2026-10-14）

锁内重读 candidate 与锁测试三分支修复成立。1 个 BLOCKER、2 个 SHOULD_FIX、1 个 NICE_TO_HAVE，已修：

- BLOCKER：release 防护按仓库根解释相对路径，写入方按工作目录解释，从子目录调用时检查与写入不是同一文件。防护现在先用
  与写入方相同的 `std::path::absolute` 解析再检查（回归测试：相对工作目录的输出指向存储时被拒）。
- SHOULD_FIX：派生输出（目录内的 ZIP、`SHA256SUMS.txt`、默认 SBOM）可经存储文件处的符号链接改写人工日志。存储读取
  与 sidecar 锁现在拒绝链接（Unix 回归测试），因此派生文件写到哪里都改不到存储读取的内容；派生文件名本身与存储文件名
  不重合。
- SHOULD_FIX：尚未创建的保留文件可被输出操作占成目录（如 `manual.lock/x.json`）。保留文件同时按前缀保护，其下级路径
  一律拒绝；目录检查加入决定收据与其锁。
- NICE_TO_HAVE：防护分派去掉通配分支，逐一列出无调用方输出的 release 命令，新增命令必须显式归类。

验证：Windows 438 + 25 通过，clippy、fmt、phase-00 通过；Linux 回放 clippy 通过，连续两次 368 + 19 通过，phase-00 通过。

### 第 2 批代码审查第 5 轮（2026-10-14）

PASS / no blocker；第 4 轮其余修复成立（release 防护与写入方统一按工作目录解析、存储与 sidecar 拒绝链接、release
分派穷举）。1 个 SHOULD_FIX 已按审查的最小修法修复：`manual.lock/../diagnostic.json` 在检查时折叠为普通路径，写入时
`create_dir_all` 却会先把 `manual.lock` 建成目录。保留路径检查改为 `path_identity::reaches`：原始路径的每一级祖先在
别名解析后、`..` 折叠前与保留路径比较；回归测试覆盖存储文件尚不存在的情形。

最终验证：Windows `cargo test -p stickymd-smoke --locked` 439 + 25 通过，clippy `-D warnings`、fmt、phase-00 通过；WSL Linux
回放 clippy 通过，连续两次 369 + 19 通过，phase-00 通过。未覆盖：Windows 原生 junction/8.3 实测、真实并发 Promote、
readiness 写入失败时的输出捕获、性能与内存测量。

## 3-A 第 3 批实施记录（2026-10-14）

记录器、被测环境采集与被测实例落地，`acceptance manual run/guided` 恢复可用：

- **环境采集**（`manual_observation/environment`）：64 位注册表视图读取 `DisplayVersion`、`CurrentBuild`、`EditionID`
  （限定 `REG_SZ`）与 `UBR`（限定 `REG_DWORD`），固定缓冲，超长、类型不符、读取失败即拒绝；原生架构取
  `IsWow64Process2` 的 `pNativeMachine`，只接受 AMD64/ARM64。采集在工作线程上执行，10 s 截止。Windows 测试核对
  本机注册表构建号与 `ver` 一致（`ver` 只用于测试对照，不用于身份）。
- **注册表**：每项带 `phase-12.md` 的操作与预期原文（测试保证逐字相等，无仓库的记录器也显示同一说明）和被测实例规则：
  M34 为首次运行声明，M37/M41/M42 保留原进程，M01/M02 用空白 note，其余用渲染压力 note 与全部图片格式。规范文本
  升到 v2 并包含这些字段。
- **备注独立成记录**：日志 schema 2 去掉观察内的备注，备注作为引用 `(session, seq)` 的独立记录追加，每条观察至多一条，
  同文本重复为 no-op、不同文本拒绝。这样 FAIL 在询问备注之前即已落盘，又不违反"同 ID 不同内容拒绝"。重置带入时
  未解除 FAIL 的备注随之带入。日志校验按注册表规则检查实例：M34 不得有实例且必须带三项声明，其余必须有实例。
- **提交**（`store::commit`）：排他锁内重读 candidate 摘要、核对会话开始时的上下文 ID，并用 readiness 同一谓词复核
  角色，任一不符整批不写。
- **记录循环**（`manual/flow`）：对终端、机器与存储泛型，因此在两个平台都有测试。每个场景：准备 → 采集 → 角色判定
  （不询问）→ 按规则启动实例 → P/F/N → 再次采集并比较 → PASS 要求启动的进程仍在运行且是唯一 StickyMD → 提交 →
  可选备注。guided step 逐项逐场景回答；重测前列出该场景未解除的 FAIL 与备注；会话 `base` 为开始时的日志长度。
- **被测实例**（`manual/instance`，仅 Windows）：会话目录位于临时目录并持有会话锁，启动时清理失去锁的目录；校验 ZIP
  与解压出的 EXE，每个场景复制一份并放入 fixture，复制后的 EXE 重新计算摘要；进程加入 Job（`KILL_ON_JOB_CLOSE` 与
  `SILENT_BREAKAWAY_OK`），记录器无论如何退出都会结束它，StickyMD 自己打开的浏览器等不受影响。开始与每次启动前
  拒绝已有 StickyMD。
- **指纹分类**：人工验收代码（`manual*`、`guided.rs`）与 `tools/manual/` 原先落入 `ALL_HARNESS` 或 `GLOBAL`，每次改动
  都使全部功能模块失效。它们不被任何功能模块执行或读取，现在不使功能模块失效；workspace tests 身份仍覆盖它们
  （P14-A77）。本批改动 `fingerprint.rs`（全局输入），本身仍会使全部模块失效一次。

新增验收行 P14-A76（记录器与被测实例）、P14-A77（分类）；观察包导出与导入随第 4 批使用 P14-A78。

验证（审查前）：Windows `cargo test -p stickymd-smoke --locked` 457 + 25 通过，clippy `-D warnings`、fmt、phase-00 通过；
WSL Linux 回放 clippy 通过，连续两次 384 + 19 通过，phase-00 通过。

未覆盖（如实说明）：真实桌面上的一次完整记录会话（需要已晋升的 candidate 与已声明的目标环境）；Job 在 Ctrl+C 与
崩溃时结束实例的实测；会话目录清理在实例仍占用文件时的实测；M34 需要第 4 批的独立记录器才能在 Clean VM 中执行。

### 第 3 批代码审查第 1 轮（2026-10-14）

4 个 BLOCKER、5 个 SHOULD_FIX、1 个 NEEDS_VERIFICATION，核实后成立，处理如下：

- BLOCKER：会话目录清理有竞态，且会删除根目录下任何无锁目录。创建会话与清理现在都在根目录自己的锁下串行；只处理
  名称为 `<pid>-<nanos>` 的条目，持有其锁完成判定与删除；无法打开或判定的保留并报告；根目录是链接（含 junction）时
  拒绝（Windows 测试用 junction 实测）。
- BLOCKER：PASS 的进程检查早于提交，其间实例退出仍会写入 PASS。`store::commit` 增加在排他锁内、所有校验之后、写入
  之前执行的最后检查；记录循环在此复核进程，拒绝时不写任何字节（保留提前检查，便于观察者改答）。
- BLOCKER：进程创建与加入 Job 之间被强制终止会留下实例。在创建时加入 Job 需要 `PROC_THREAD_ATTRIBUTE_JOB_LIST`，
  而 `CommandExt::raw_attribute` 在固定工具链 1.97.1 上仍不稳定，唯一的原子做法是用 `CreateProcessW` 取代
  `std::process`。窗口是两个相邻调用，后果是一个可见、可从托盘退出的测试实例，不影响任何观察证据，因此不替换进程
  创建，而是如实收窄承诺：plan 11 改为"已加入 Job 的实例"随记录器退出终止，并写明遗留情形；下一次会话拒绝启动时
  指明该 PID 来自中断的记录器。本批第 3 批实施记录中"无论如何退出都会结束实例"的说法以此为准。
- BLOCKER：分类依据不成立——正式运行的证据写入（`exact_desktop::evidence::finish`、`smoke_scope`、资源观察）都经
  `validate_public_evidence_path` 调用人工存储的保留路径判定，修改它会改变正式运行结果而指纹不变。保留路径与人工存储
  锁移到新的 `qualification/reserved_paths.rs`（仍按工具代码分类，是全部功能模块的输入）；新增两个测试：真实指纹在
  修改人工代码时不变、修改保留路径时全部变化；源码守卫只允许列出的非人工文件引用人工模块（`decisions.rs` 的保留
  路径判定只用自身常量，campaign 在任何模块运行之前投影决定，校验失败只会中止 campaign）。
- SHOULD_FIX：字符串读取未拒绝 `REG_EXPAND_SZ`（会被展开并报告为 `REG_SZ`）→ 加 `RRF_NOEXPAND`；读取改为按键
  参数化，Windows 测试在 HKCU 临时键上覆盖展开字符串、超长、空值、类型不符、缺失与恰好 64 字符。
- SHOULD_FIX：Promote 不与人工提交串行 → Promote 在替换 staging、candidate 收据与下载收据时持有人工存储排他锁
  （无网络测试，见未覆盖）。
- SHOULD_FIX：解压与 `tasklist` 无界等待 → 改用已有的有界子进程执行（解压 120 s，`tasklist` 15 s），G3/G4/G5 一并受益。
- SHOULD_FIX：fixture note 经截断写入 → 改为原子写入（不改 G3–G5 共用的 `seed_note`）。
- SHOULD_FIX：清理失败被静默忽略 → 停止进程与删除目录失败时输出警告与残留路径，锁文件已被并发清理时不报。
- NEEDS_VERIFICATION：P14-A76/A77 超出测试范围 → 两行收窄到测试实际证明的内容；新增 Job 关闭结束进程、写入失败结束
  会话、提交前复核等测试。

验证：Windows `cargo test -p stickymd-smoke --locked` 465 + 25 通过，clippy `-D warnings`、fmt、phase-00 通过（原始输出
随第 2 轮审查材料提供）。仍未覆盖：真实桌面完整会话、记录器启动候选并记录 PID 的端到端运行（`Workshop` 无单元测试）、
Ctrl+C 与强制终止的实测、Promote 与提交并发的实测。

### 第 3 批代码审查第 2 轮（2026-10-14）

第 1 轮的 F2、F4、F5、F6、F8 判为已修；F3 的收窄处置被接受，不再列为阻断（理由按审查意见改为：Job 关联失败或此前
中断都到不了本场景的回答与提交，下一次启动先检查已有 StickyMD；而不是"只隔两个相邻调用"）。1 个 BLOCKER、5 个
SHOULD_FIX、1 个 NICE_TO_HAVE、1 个 NEEDS_VERIFICATION，核实后成立，处理如下：

- BLOCKER：候选已启动、但未在限时内生成 note 与配置时，记录器把它当作准备失败直接跳过，观察者无法登记 FAIL，旧
  PASS 继续有效。现在准备失败（复制、fixture、摘要不符、加入 Job 失败）与"已启动未初始化"分开：后者保留实例身份，
  说明原因，只接受 `F` 或 `N`，PASS 被拒绝（测试覆盖）。
- SHOULD_FIX：清理时目录枚举错误被 `flatten` 吞掉 → 逐项报告。
- SHOULD_FIX：指南仍无条件承诺实例都会关闭 → 写明残留窗口与按 PID 处理的方式。
- SHOULD_FIX：有界辅助进程只限制了等待退出，终止后的等待与读取输出无界、输出无上限 → 输出在独立线程中按 1 MiB
  上限并发读取，进程退出后管道仍被占用即报错；终止后的等待有上限；`ChildGuard` 的析构改为有界等待并在超时时警告
  （所有 harness 共用，终止后的进程正常在毫秒内退出）。新增超出输出上限的测试。
- SHOULD_FIX：加入 Job 失败的早退路径不报告停止失败 → 用有界停止并把失败并入错误信息。
- SHOULD_FIX：最后检查的测试未证明锁被持有、会话根锁测试未证明创建互斥 → 检查闭包内用独立句柄确认排他锁冲突；
  新测试在外部持有根锁时创建会话被阻塞、释放后完成。
- NICE_TO_HAVE：源码守卫可被别名导入绕过 → 改为跳过注释与字面量的词法扫描，识别 `use` 声明内（含别名、分组、跨行）
  与 `::` 两侧的人工模块名，并有自测。
- NEEDS_VERIFICATION：审查沙箱读不到放在仓库外的原始输出 → 第 3 轮的原始输出放在仓库内被忽略的 `target/` 下。

另：转交审查的代理在第 2 轮材料目录中删除了三个它自己创建的临时文件，违反其"不删除"约定；不涉及仓库与证据。

### 第 3 批代码审查第 3 轮（2026-10-14）

审查方这次读取了仓库内的原始输出；第 2 轮 BLOCKER 与 F1、F2、F4、F5、F6、F8、F9 判为已修，F3 的收窄处置维持接受。
1 个 BLOCKER、5 个 SHOULD_FIX、1 个 NICE_TO_HAVE、1 个 NEEDS_VERIFICATION，核实后成立，处理如下：

- BLOCKER：Performance/Resources 的隔离预检只认既有的 smoke 临时根目录，记录器会话根目录 `stickymd-manual-sessions`
  不在其中——记录中或中断遗留的实例不会使正式测量 fail closed。会话根目录名移到共享进程层（`managed_process`），
  并加入 smoke-owned 列表；分类测试与真实预检测试（不终止既有进程）覆盖该根目录。
- SHOULD_FIX：显式 `kill_and_wait` 仍无界，人工提交在存储锁内经候选校验运行仓库子进程。`kill_and_wait` 改为有界停止；
  锁内的 candidate 复核（提交、声明、重置）改为只读 candidate 收据摘要，完整的仓库校验在命令开始时完成。G3–G5 的
  `run_cargo_test` 无超时属于既有 harness，不在本批范围。
- SHOULD_FIX：辅助进程超时或退出后，持有输出管道的子孙进程与读取线程会遗留。辅助进程现在在包含子孙的 Job 中运行
  （Job 移到 `managed_process`，记录器的实例 Job 仍让 StickyMD 的子进程脱离）；辅助进程退出或超时后关闭该 Job，
  读取端随之结束；两路输出共用一个收集截止时间。真实测试：辅助进程退出时留下持有 stdout 的子孙，调用及时返回且
  子孙没有存活。
- SHOULD_FIX：会话目录正常删除时不取根目录锁 → 删除也在根目录锁下进行，取锁失败时保留并报告。
- SHOULD_FIX：根目录锁测试会把去掉锁的实现判为通过 → 改为带开始信号、结果保持存活的受控交错测试，分别证明创建与
  删除都在外部持有根锁期间不发生。
- SHOULD_FIX：Source Freeze 作废 candidate 时不取人工存储锁 → 写入 Source Freeze 收据与作废 candidate 在同一排他锁下。
- NEEDS_VERIFICATION：真实快速失败的候选未覆盖 → 候选在加入 Job 前退出时，按"已启动未初始化"处理而不是准备失败；
  等待初始化时一旦进程退出立即返回。真实测试用立即退出的程序代替候选，确认返回启动失败、带实际 PID 与实测摘要，
  且不等满超时。
- NICE_TO_HAVE：词法守卫可被宏参数与 `#[path]` 绕过 → 两个专有名称在代码中任何位置出现都报告（`mod` 声明除外），
  指向人工代码的 `#[path]` 同样报告；P14-A77 写明这是词法守卫，不展开宏、不追踪再导出。

### 第 3 批代码审查第 4 轮（2026-10-14）

审查方复算了工作树摘要与原始输出，与 `tree-digest.txt` 一致；R3 的根锁删除、Source Freeze 加锁判为已修，根锁测试与
立即退出测试判为有效但仍可加强。1 个 BLOCKER、3 个 SHOULD_FIX、1 个 NICE_TO_HAVE，核实后成立，处理如下：

- BLOCKER：隔离预检只认本进程 TEMP 下的 smoke 根目录，另一终端以不同 `TMP` 运行的记录器实例会被放过；无法读取映像
  路径的 `StickyMD.exe` 也被静默丢弃。现在按路径中任一级目录名识别 smoke 根目录（用户目录恰好同名只会让测量拒绝开始，
  不会终止任何进程）；映像路径不可读或进程枚举提前中断时隔离无法确认，fail closed；列出后已退出的进程不计入。真实
  预检测试新增"根目录不在本进程 TEMP 顶层"的子进程。这一缺陷早于本批（G3–G5 等根目录同样受影响），一并修正。
- SHOULD_FIX：辅助进程在加入 Job 前可能已创建逃逸的子孙。辅助进程与被测实例现在都以挂起状态创建、加入 Job 后再恢复
  运行（`managed_process::Job::spawn`），在 Job 之外不执行任何指令；第 1 轮 F3 的残留因此缩小为"创建与加入之间被强制
  终止时留下一个从未运行的挂起实例"，plan 11 与指南同步改写。
- SHOULD_FIX：子孙回收测试可在子孙根本没运行时通过 → 子孙先写入启动信号，辅助进程确认它已运行后才退出；测试检查辅助
  进程成功退出、子孙确已运行且没有存活。
- SHOULD_FIX：锁内回读的 candidate 收据无大小上限 → 必须是普通文件且不超过 64 KiB，读取时限量；超限与非普通文件的
  拒绝有测试。
- NICE_TO_HAVE：原始字符串形式的 `#[path]` 绕过守卫 → 原始字符串同样保留为 token 检查，自测覆盖。

维持的已知限制：G4 `run_cargo_test` 无超时（审查认可为本批范围外的既有问题，需单独跟踪）；根锁交错测试依赖 300 ms
调度窗口，是有效回归测试而非确定性证明；Source Freeze 与提交的交错没有专门测试。

### 第 3 批代码审查第 5 轮（2026-10-14）

PASS / no blocker；R4 的隔离预检、管道测试与原始字符串守卫判为已修，摘要复算一致。4 个 SHOULD_FIX/NEEDS_VERIFICATION
与 1 个 NICE_TO_HAVE，按最小修法处理：

- SHOULD_FIX：恢复挂起线程时只排除失败值，线程若被额外挂起仍会当作已恢复；线程枚举异常结束未检查。现在要求
  `ResumeThread` 返回的此前挂起计数恰为 1（否则停止该进程，不去恢复别人施加的挂起），并要求枚举以
  `ERROR_NO_MORE_FILES` 结束；返回值判定有单元测试，真实注入未做。
- SHOULD_FIX：`is_file()` 不排除非 name-surrogate 的重解析点（如云占位文件），锁内读取可能触发提供方取回。Windows 上
  candidate 收据带重解析点、脱机或需回调取回属性时拒绝；属性判定有测试，可创建符号链接时另测链接被拒。
- SHOULD_FIX：会话清理没有上限。每次清理最多列出 1024 个条目、删除 16 个过期会话、耗时 15 s、报告 16 条警告，其余
  留待以后并报告剩余数量（测试：20 个过期会话分两次清完）。单个会话目录只含一份解压候选及其副本，单次删除量小。
- NEEDS_VERIFICATION：8.3 与 junction 别名。预检现在对报告路径不匹配时再按最终路径（`canonicalize`，解析短名与
  junction）判断，解析失败则隔离无法确认、fail closed。真实测试：经名称不含 smoke 根的 junction 启动、经 8.3 短路径
  启动（本机卷生成短名，测试路径被缩短为 `STED09~1` 一类的形式），预检都识别出进程且不终止它。
- NICE_TO_HAVE：普通字符串转义（如 `\u{61}`）可绕过 `#[path]` 检查 → 按 Rust 规则解码后再比较；自测覆盖。

耗时说明的依据（本会话实测，未写入证据目录）：同一构建连续两次运行单元测试分别为 37.05 s 与 54.51 s；按模块过滤时
`managed_process` 1.27 s、`exact_desktop` 4.34 s、`manual::` 0.64 s、`manual_observation` 1.00 s，`module_ledger`
24.90 s（git 子进程密集）。

### 第 3 批代码审查第 6 轮（2026-10-14）

PASS / no blocker；F1 判为已修，F2–F5 部分修复，另有 5 项细化，处理如下：

- N1（SHOULD_FIX）：属性检查晚于首次打开——`symlink_metadata` 在 Windows 上先打开文件。现在 candidate 收据的元数据取自
  其目录列表（Windows 的目录项自带属性，不打开文件；Unix 上即 `lstat`），通过后才打开读取。外部在检查之后替换文件
  不在保证范围内。
- N2（SHOULD_FIX）：15 s 不约束单个目录的递归删除。清理改为逐项删除并计入删除预算（每次最多 20 000 次删除），每项之前
  检查截止时间；预算或时间用完时保留会话锁文件，下次继续；列出根目录时同样先查限额与时间；警告超过 16 条只计数
  （测试：60 个文件的目录在预算 10 时部分删除、再次调用完成）。
- N3（NEEDS_VERIFICATION）：8.3 分支可静默跳过、断言分不清"判为过期"与"无法确认"。测试现在要求错误以 stale 判定开头且
  PID 列表中有该 PID，8.3 分支要求被缩短的正是 smoke 目录本身并输出执行与否。修正过程中发现此前的 8.3 分支实际从未
  针对 smoke 目录执行——测试用 `Command::args` 给 `cmd` 传参，引号被按 C 运行库规则转义，`cmd` 解析错误，只有父目录被
  缩短；改用 `raw_arg` 后分支真正执行。实测记录：经 junction 启动时 Windows 报告解析后的目标路径；经 8.3 短路径启动时
  报告的是短路径（如 `...\ST41EA~1\STICKY~1\StickyMD.exe`），名称不含 smoke 根，只有按最终路径的判断能识别——第 5 轮
  的待核实项确实是缺口，现已由测试覆盖。
- N4（NEEDS_VERIFICATION）：恢复线程的原生失败（`ResumeThread` 失败、异常挂起计数、枚举异常结束）只测了判定函数，
  未做原生注入；记为已知限制。
- N5（NICE_TO_HAVE）：守卫的转义解码支持 Unicode 转义中的 `_`，续行只跳过 ASCII 空白，先规范化 CRLF，并识别
  `r#path` 原始标识符；自测覆盖。

### 第 3 批代码审查第 7 轮（2026-10-15）

PASS / no blocker（第 5–7 轮连续三轮无阻断）；审查方根据专项日志确认 8.3 分支确实经被缩短的 smoke 目录执行。剩余
4 个 SHOULD_FIX、1 个 NEEDS_VERIFICATION 按最小修法处理后即提交，未再送审：

- 清理预算：删除链接时退回 `remove_dir` 的第二次尝试、过期会话锁文件的删除都计入删除预算并检查截止时间；会话正常
  退出也改用同一有界删除，未删完时保留锁文件留待以后；警告在释放根目录锁之后输出。15 s 是操作之间检查的预算，
  不能中断已经阻塞的同步 I/O。
- 守卫词法：块注释按 Rust 规则嵌套；空白按 Rust 的 `Pattern_White_Space`（如 U+200E）而非 `char::is_whitespace`；
  新增嵌套注释隐藏与误报、U+200E、CRLF 续行与续行后非 ASCII 空白的正反例。
- 8.3 测试：短名查询失败使测试失败而不是跳过；设置 `STICKYMD_REQUIRE_SHORT_NAMES` 时跳过也算失败，证据运行使用它。
- N4 限制同步到 plan 11、指南与 P14-A76：恢复挂起线程的原生失败路径只验证了判定逻辑。
- candidate 收据的保证收窄：只保证不预先打开收据本身；目录自身需取回、目录列表中的属性已过期、检查后被外部替换，
  不在保证范围内（源码注释与 P14-A76 同步）。

## 证据写入超过 MAX_PATH 的修复（2026-10-15，06eefab）

第 3 批推送后 PR CI 在 Windows runner 上以 os error 3 失败：`MoveFileExW` 对普通路径受 260 字符限制，而 runner 的临时
目录较长，`std` 创建临时文件成功、发布时失败（问题自 U1 起潜伏，本地临时目录较短未触发）。本地以加深目录复现后修复：
普通路径经已存在父目录的规范形式（`\?\`）加文件名发布，文件名按 Win32 语义去掉末尾的点与空格；verbatim 路径原样
使用。

- 审查第 1 轮：2 个 BLOCKER——verbatim 路径中的 `/` 在拼接后会变成分隔符，写入位置可能与防护检查的不同；普通路径的
  DOS 设备名（`NUL`、`COM1` 等）在 Win32 下打开设备，扩展形式却是普通文件。两者都改为写入之前拒绝。
- 审查第 2 轮：PASS / no blocker；SHOULD_FIX 已修：`COM0`/`LPT0` 是普通文件；普通文件名中的 `:`（备用数据流或设备
  写法）拒绝；公共证据路径防护先调用同一拼写检查，被拒时不创建父目录；测试与临时目录的写法无关，并断言真实临时文件
  长度超过 260；防护错误带出路径。

验证：Windows 479 + 25、WSL Linux 393 + 19 通过，clippy、fmt、phase-00 通过。

## 3-A 第 4 批实施记录（2026-10-15）

观察任务导出、独立记录器与整包导入落地（plan 11 `#manual-observation-log`「其他机器」）：

- **格式**（`manual_observation/transfer`）：观察任务携带 candidate receipt 与目标收据的**原文**（日志头绑定的是文件
  字节的摘要，因此不重新渲染）、注册表摘要、`base`、选择与所选场景未解除的 FAIL（含备注）；解析时核对两份原文的
  摘要、目标收据指向任务的上下文与 candidate、未解除 FAIL 的场景已知且位于 `base` 之前。观察包是部分日志：日志头的
  四项摘要加一个会话的观察与备注；解析要求单一会话、序号从 1 连续、`base` 一致、不含携带项、备注只解释包内观察且
  每条至多一条。两者读取前检查 1 MiB 上限。观察与备注的编解码与日志共用。
- **导出**（`acceptance manual export --task=<path> [--select=…]`）：先做完整 candidate 校验，再在共享锁内读取 candidate
  收据原文（与开始时的摘要不同即拒绝）、上下文与日志快照；输出路径经公共证据路径防护；写入前用同一解析器回读。
- **独立记录器**（`acceptance manual record --task --zip --bundle`）：在查找仓库根之前分派，不需要仓库。工具内置注册表
  必须与任务的注册表摘要相同；有需要启动实例的场景时按任务的 candidate receipt 校验 ZIP 与 EXE（复用 `Workshop`），
  只有 M34 首次运行声明时仍校验 ZIP 摘要。观察包先以不得预先存在的方式创建，之后每个回答（含备注）都在下一个问题之前
  原子重写；会让包超过 1 MiB 的回答被拒绝并结束会话，`last_check` 在大小检查之后、写入之前运行。包路径位于某个仓库内
  时受该仓库的公共证据路径防护。
- **导入**（`acceptance manual import --bundle=<path>`）：先做完整 candidate 校验；`store::import` 在排他锁内复核 Promote，
  要求包头的 candidate receipt、上下文、目标收据与注册表摘要与日志头相同，再与本地会话共用同一追加步骤（按同一谓词
  重算角色、`base` 不得超出日志、同 ID 同内容跳过、整包一次原子替换）。同一个包或增长后的副本可重复导入。
- **记录循环**：`flow::run` 不再读取存储上下文，改为 `Start { target, base, unresolved }`；本地记录器从日志构造，独立
  记录器从任务构造。
- **工具依赖**：工具以 `+crt-static` 构建；编译出的工具（debug 与 release 一致）在产品允许的 Windows 自带 DLL 之外只导入
  `bcrypt.dll` 与 `WTSAPI32.dll`（dumpbin 核对），由集成测试用产品导入门对编译出的工具断言。单元测试二进制额外导入
  `USERENV.dll`（来自测试框架），因此不用它代替工具本身。解压与进程检查只用 Windows 自带的 PowerShell `Expand-Archive`
  与 `tasklist`。
- **plan 11 措辞**：原文"观察任务含注册表片段"改为"注册表摘要，工具内置注册表必须相同"。片段不增加校验能力：导入时
  日志头的注册表摘要本就必须与当前注册表相同，记录器也只能按自己编译进的注册表运行。属于已批准设计内的细化，不是
  骨架变化。

新增验收行 P14-A78；`cli.rs`、`main.rs`、`receipt.rs` 与 `qualification/mod.rs` 是全局输入，本批会使全部模块指纹失效一次。

验证（审查前）：Windows `cargo test -p stickymd-smoke --locked` 488 + 26 通过，clippy `-D warnings`、fmt、phase-00 通过；
WSL Linux 回放 clippy 通过、402 + 19 通过、phase-00 通过。

未覆盖（如实说明）：真实机器上的一次完整导出 → 独立记录 → 导入（需要已晋升的 candidate 与已声明的目标环境）；在没有
仓库、Rust、VS 与额外 VC runtime 的 Clean VM 中运行独立记录器；M34 的实际 Clean VM 流程。

### 第 4 批代码审查第 1 轮（2026-10-15）

1 个 BLOCKER、1 个 SHOULD_FIX、1 个 NICE_TO_HAVE、1 个 NEEDS_VERIFICATION，核实后成立，已修：

- BLOCKER：导入只校验观察中 `instance.exe_sha256` 的格式。把一条有效 PASS 的 EXE 摘要改动一个字符，导入、readiness 都
  照样接受，还能解除 FAIL。现在由共用的有效性判定（`reduce::Binding`：目标收据与 candidate 的 EXE 摘要）同时检查角色与
  实例的 EXE 摘要（M34 无实例，不受影响）；本地记录、导入、重置带入、readiness、`status` 与导出都用它，不符的观察使该
  `(项, 角色)` 无效且不能解除 FAIL。存储接口把 candidate 的两个摘要合成 `Promoted`。plan 11「被测实例」写明这一判定。
- SHOULD_FIX：包路径防护只看给出的路径，仓库外指向仓库 `dist/evidence` 的目录链接可以绕过。现在对给出的路径与其已存在
  目录经链接解析后的路径都做仓库识别与公共证据路径防护，目录无法解析即拒绝（Windows junction 与 Unix 符号链接测试）。
- NICE_TO_HAVE：空观察包的 `base` 不受日志长度约束。解析时 `base` 不得超过日志容量，导入时不得超过当前日志长度。
- NEEDS_VERIFICATION：真实往返与 Clean VM 运行仍未执行，见上节"未覆盖"。

验证：Windows `cargo test -p stickymd-smoke --locked` 490 + 26 通过，clippy `-D warnings`、fmt、phase-00 通过；WSL Linux
回放 clippy 通过，连续两次 404 + 19 通过，phase-00 通过。

### 第 4 批代码审查第 2 轮（2026-10-15）

PASS / no blocker；第 1 轮三项修复经源码核对成立（EXE 摘要经共用判定绑定 candidate，receipt 摘要与 EXE 摘要来自同一次
candidate 读取；链接解析后的目录同样受防护，未发现 `\?\` 写法引入的新绕过；空包的 `base` 受日志长度约束）。剩余
NEEDS_VERIFICATION：同一 promoted candidate 上的真实导出 → 独立记录 → 导入（含回答落盘后中断、重复与增长包导入、旧任务
不能解除新 FAIL），以及无仓库、Rust、VS 与额外 VC runtime 的 Clean VM 运行和 M34 顺序，仍未执行，留待 v0.1.3 候选资格化时
完成。

## 覆盖补全批设计（2026-10-15，经 11 轮审查 AGREE）

范围：把 Phase 8 人工义务与 plan 09 的相关断言映射进 P12 人工注册表（USER 2026-10-14 决定的"3-A 之后单独一批"）。gpt-6-astra
（effort max）第 11 轮 AGREE（0 BLOCKER、0 SHOULD_FIX），条件是下文四项 USER 决定获批并先更新相关 plan。工作稿与逐轮审查在被
忽略的 `target/review-evidence/coverage-r1..r11/`；以下是有效内容。

### 原则

- 客观、可由 exact candidate 机器判定的断言归 G3/G4 exact 自动化（扩展现有 case）；人工只保留视觉判断、真实 Explorer / 前台交互
  与托盘辨识。映射完整不等于验收通过。
- 只映射已有义务，不发明产品需求；需要的产品改动先进入 plan。

### 义务表与治理

- manual-only 模块 `manual_observation/coverage.rs` 声明义务 `(来源, 断言, 参数) → 目标`，目标为 `Manual{case, scenario}`、
  `Exact{case, check}`、`Automated{test, mode}`（`unit` / `local-preflight` / `runtime-module`）、`Unit{test, rationale}`、
  `Retired{decision, source, assertion, params}` 或 `Gap`；`KNOWN_GAPS` 由 `Gap` 推导，`Retired` 单独列出。
- 完整性：`phase-08.md` 整张矩阵（62 个 Manual 行：15 个 P08-Mxx + 47 个 P08-Dxxx）与 plan 09 的 Esc / 滚动条 / 透明度 / 退出条款
  按区段与逐条款绑定文本摘要和条数；每个 `(source, assertion_id)` 声明必需参数集合，覆盖只在同一断言内合并；反例测试覆盖追加
  条款、删减参数、跨断言掩盖、重复、未知 ID 与格式异常。退役决定只授权其列出的条目。C1 交付完整的子断言 × 参数展开表送审。
- exact 检查标签进入共享收据验证器 `module_evidence`（全局输入）：每个 G3/G4 case 声明必需检查集合，标签带参数身份；缺失、重复、
  错 case、非成功与未知 schema 均拒绝；记录由检查函数自身产生；按 plan 11 的模块指纹与 origin identity 复用。

### 映射要点

- 人工新增场景：M01/M02 加左 / 右 / 上 dock 下的候选窗视觉；M05 加 `second-launch-hidden/minimized/docked` 与 `exit-save-failure`；
  M11/M12 加 `undock-16dip`；M22 改为 40/70/96/100 四档；M23 加 `scrollbars`；M35 加 `primary-switch`、`secondary-right`（顶部对齐）、
  `secondary-below`（左侧对齐）；M38–M40 的 dock 拆为左 / 右 / 上并加 `scrollbars`。
- 新人工项：M45 两份便携副本（托盘辨识、各自显示 / 隐藏，Tier A）、M46 Windows 11 外壳视觉与操作（圆角阴影、置顶按钮、控件淡化、
  边角调整大小、各模式控件集成，Tier A）、M47 100% DPI（Tier B）。"accessibility" 从 P08-M12 退役。
- exact / automated 扩展：G4-06（Right/Top dock 组字不收起、96/100 透明度、composition 中 Esc 只取消 preedit）；G4-02（真实键盘 Esc
  分支 1/4/5/6、Floating、Shift+Esc、重复 key-down 逐次独立、收起按钮、透明度三种提交与滑块拖动中 Esc）；G4-01（退出保存失败、
  置顶双向同步与临时 sensor topmost 区分、Alt+F4）；G4-05（dock 收起状态下第二次启动）；G3-03（恢复待处理时 Esc 无变化的确定性
  fixture，`tmp.mtime > note.mtime` 显式设置并回读，含正向对照；资产写入与移入 trash 的确定性强杀点）；P08-A11 正式 Runtime 收据
  扩展（两份副本 note/config 双向不串写、退出 A 不影响 B）。
- `Unit` 覆盖（待 USER 决定）：资产对账进行中、退出进行中两个 Esc 守卫状态（exact 层不可控），以及 Ctrl+Esc（系统快捷键），
  经生产决策路径的单元测试，不证明原生投递。
- 人工场景：每角色 55 → 96，两个角色 110 → 192；人工项 24 → 27。

### 协议

- **退出保存失败**：G4-01 与人工场景都按"保存基线 → 设只读并回读 → 输入新文本 → 确认未落盘 → 托盘退出"进行，所有路径显式恢复属性，
  恢复失败不形成 PASS。因果由产品标题状态判定（待 USER 决定）：只由 `QuitCancelled(NoteSave)` 建立；`requires_note_barrier()`
  （`dirty || durability_required || note_save_in_flight`）在一次完整的主线程事件处理之后为假时清除；载入外部时解除被取代的重建
  义务；`update_window_title` 是唯一写入者，优先级为恢复 → 退出已取消：保存失败 → 冲突 → dirty → 公式 hover → 普通；固定字面值
  `StickyMD * — 退出已取消：保存失败`。
- **第二次启动**（严格激活，USER 决定）：记录器检查前置状态（隐藏 / `IsIconic` / 实际几何判定的 dock 收起），以带时间戳的采样记录到
  转换开始；连续性与"双击先于恢复"由观察者声明；PASS 需可见恢复、展开、只剩原 PID、键入进入原窗口；已证实的产品失败（第二进程
  超时存活、未恢复、原 PID 消失）必须可记 F；Windows 拒绝激活或证据不足记 N。
- **TwoCopies**（M45，待 USER 决定）：PASS 要求提交时运行的 StickyMD 恰好是注册表声明的实例集合；观察 `instances: Vec`（日志
  schema 3、观察包 schema 2），计数、PID 唯一与每个 EXE 摘要共用一个判定；不扩展到同目录第二次启动。
- **输入注入**：五个直接注入点（Rust 键盘与鼠标、helper 的 Esc / 右击 / 重试 Esc）全部迁入一个 `cfg(panic = "unwind")` 注入模块
  （abort 构建只有返回错误的桩），治理文本扫描禁止三种注入 API 出现在模块之外；先登记后注入，正常路径显式有界释放、`Drop` 兜底；
  清理以 `SendInput` 插入数为准（短计数即失败；完整计数只证明进入输入流），保留现有鼠标左键释放采样（采到 Down 为失败，0 不证明
  Up）；清理失败触发共享停止（后续注入输入的 G3/G4 case 不执行、不产生 PASS）；业务断言与清理分开判定。托盘菜单需核验从关闭到
  打开的变化。

### 需要 USER 决定（实施前）

1. 退出取消标题状态及 plan 09 退出合同的补充（含上述建立、清除与优先级）。
2. 上述 `Unit` 覆盖范围。
3. TwoCopies 对 plan 11:315 单实例规则的例外（只适用于 M45；同步 P14-A76）。
4. 双显示器右 / 下排列归 M35（Tier B）。

已决定（2026-10-15）：严格激活；accessibility 退役；M45 A、M46 A、M47 B；PR #3 先行合并。

### 分批与取证

C1 义务表、治理、注册表与新项、文档；C2 exact / automated 扩展与收据标签；C3 记录器（TwoCopies、前置检查、只读 fixture）与日志 /
观察包 schema。实施后取证：短计数与释放失败的共享停止、普通 unwind、托盘关闭到打开、实际 abort 二进制在有效环境下到达拒绝桩且
零注入（绕过收据复用）、TwoCopies 各失败路径、记录器中断、属性恢复失败、观察包往返、确定性强杀边界。

### Resolution（2026-10-15，覆盖补全批）

USER 批准全部四项：(1) 退出取消标题状态，先补 plan 09 退出合同后实施；(2) 资产对账进行中、退出进行中两个 Esc 守卫状态与
Ctrl+Esc 只由单元测试覆盖，验收中注明不证明原生投递；(3) TwoCopies 对 plan 11 单实例规则的例外，只适用于 M45，同步 P14-A76；
(4) 双显示器右 / 下排列归 M35（Tier B）。实施按 C1 → C2 → C3 分批进行，每批先更新 plan，再更新投影与代码。

## 覆盖补全 C1 实施记录（2026-10-15）

范围：plan 与投影先行（plan 09 退出取消标题状态与标题优先级、plan 11 `被测实例` 的 TwoCopies 例外与 `人工义务映射`），
然后是义务表、治理、注册表 v3 与新项。产品 runtime 不变；人工状态仍为 NOT TESTED。

### 落地内容

- **义务表**：`manual_observation/coverage.rs` 只含类型与查询（`gaps()`、`retired()`、`describe()`）；数据在
  `coverage/table.rs`（217 项：人工场景 141、G3/G4 exact 25、其他自动化 8（`Unit` 7、`RuntimeModule` 1）、具名缺口 42、退役 1）。
  需求表 `coverage/declared.rs`（`ASSERTIONS` 与 `RETIREMENTS`）只在测试中编译，runtime 只保留静态 `OBLIGATIONS`，
  readiness 与 `acceptance manual list` 对它做一次线性过滤。
- **治理测试**（`coverage/tests.rs`）：phase-08 的 62 个 Manual 行与 plan 09 Esc（6）、手工收起（1）、滚动条（7）、透明度（3）、
  退出（3）按分段摘要与条数、逐条 16 位摘要前缀绑定；每个 `(来源, 断言)` 的参数集合精确相等（`BTreeMap`/`BTreeSet`，
  O(n log n)），检查器对缩减、跨断言掩盖、重复与未知项的反例测试；所有者存在性一次解析后查找（注册表场景；G3/G4 源码中的
  `id` 字面量集合，以文本读取避免耦合；Automated 验收行 ID 集合或节标题；退役决定唯一授权且记录在 phase-08）。
- **注册表 v3**：27 项（Tier A 16、B 8、C 3），每角色 96 个场景；新增 P12-M45（`TwoCopies(Rendering)`）、P12-M46、P12-M47；
  `KNOWN_GAPS` 删除，由义务表的 `Gap` 推导；readiness JSON 增加 `retired`。
- **尚未有记录器程序的场景**：M05 的三个第二次启动场景与 `exit-save-failure`、M35 的 `secondary-right/below`，以及双副本
  用例，在任何提示、采集或启动之前拒绝记录（`manual/flow.rs` 的 `AWAITING_RECORDER`）；schema 2 日志拒绝双副本观察。
  它们在 C3 之前保持 NOT TESTED：M05 与 M45 作为 Tier A、M35 作为 Tier B 照常阻断，除非有绑定上下文的 USER waiver。
- **投影**：phase-12 行文本与注册表逐字一致；phase-08 P08-M12 去掉 accessibility 并注明 `RETIRE-P08-M12-ACCESSIBILITY`；
  guided G1/G2 步骤同步，新增 G2-07（M45）、G2-08（M46）；新增 P14-A79；coverage-matrix 映射。
- **治理 ID 规则**：Phase 12 人工行按 Tier 分节，M45/M46 位于 Tier A、M47 位于 Tier B，因此 P12-M ID 检查改为排序后与
  `1..=47` 精确相等（重复或缺号仍失败），P12-A 仍要求文档顺序。

### 与设计稿的差异（均为分批推迟，不改变设计）

- `Exact` 暂只绑定 case；逐检查标签与 `module_evidence` 校验随 C2 的 exact 扩展落地。
- USER 批准的仅单元覆盖项（两个 Esc 守卫状态、Ctrl+Esc）与标题优先级的单元测试尚不存在，目前是具名 `Gap`，C2 补测试后改为
  `Automated{mode: Unit}`；`local-preflight` 模式没有使用者，未声明。

### 验证（2026-10-15）

- Windows：`cargo fmt --all -- --check` 通过；`cargo clippy --workspace --all-targets --locked -- -D warnings` 通过；
  `cargo test -p stickymd-smoke --locked`：单元 498 passed / 17 ignored，集成测试全部通过（含 26 个 CLI 退出测试）；
  `stickymd-smoke phase 00` PASS。
- Linux（WSL Debian 重放，表拆分前）：fmt、`clippy -p stickymd-smoke --all-targets -D warnings` 通过；tests 412 passed / 3 ignored、
  19 passed。
- 指纹：phase-08 验收文档是共享输入，本批使全部模块失效一次；完整 `dev-check` 结果见下文审查记录。

### C1 代码审查第 1 轮（2026-10-15）

gpt-6-astra（effort max）：DISAGREE，1 BLOCKER、5 SHOULD_FIX；逐项核实后全部接受。

- **B1 P08-D068 归属错误**：G4-02 没有覆盖窗口，且以 `WM_MOUSEMOVE` 补强感应条进入，不能证明被遮挡的感应条可经真实鼠标到达。
  改为具名缺口（G4-02 扩展：Pin OFF、被覆盖、真实鼠标）。
- **S1 子断言遗漏**：plan 09 滚动条第 4 条拆出 `drag-exit`（松开 / 失焦 / 切换视图 / 尺寸变化）与 `drag-outside-window`；现有
  `scrollbar` 测试只检查几何与 clamp，不经过结束拖动的路径，均列为缺口。第 3 条拆为 `thumb-geometry` 与 `hover-drag-emphasis`；
  透明度第 3 条补 `no-stepwise-write`（P08-A07）；退出第 1 条拆为 `keeps-running`、`memory-text-kept`、`visible-error`，第 2 条拆为
  建立原因（`cause=note-save-failure / quit-guard / paste-unconverged`）、`cleared-after-event-when-barrier-false` 与
  `external-load-releases-rebuild`，均为缺口。
- **S2 人工场景文字不足**：M22 改为拖动滑块并观察拖动中的实时预览；M38–M40、M47 的 dock 明确为拖到边缘、失焦收起、另一窗口在前台时
  悬停 3 DIP 感应条展开，预期加"悬停展开不抢前台焦点""滚动条可抓取且不遮字"；逐项复核全部 141 个人工映射后，M05 另补"传感区悬停展开
  后点击"与"托盘图标与菜单可辨识"（P08-D070、P08-D131）。registry、phase-12 与 guided 同步。
- **S3 来源摘要可绕过**：分解移入只在测试中编译的 `coverage/sources.rs`，成为对文本的纯函数；plan 09 改为绑定 Dock、垂直滚动条、透明度、
  托盘生命周期四节的原文摘要与条款数，提取器对段落、下级标题、孤立续行、重复条款、无外侧竖线或残缺的 Manual 行报错；反例从修改原始
  Markdown 出发（Esc 引导句、追加退出条款、托盘新条款、滚动条子标题、透明度正文、无竖线与残缺 Manual 行），并先确认未改文档通过。
- **S4 自动化目标接受空值**：`Automated` 增加 `file`，按指定文档中的 Automated 行 ID 或完整节标题精确查找；证据文字须与 mode 相符
  （`Unit` 要求 `cargo test` 或 tests，`RuntimeModule` 要求 runtime），这是文字一致性检查，不证明测试内容；空引用、标题子串、错误文件、
  错误 mode 的反例均被拒绝。所有者检查一次解析为集合与映射。
- **S5 投影措辞**：phase-08 说明与 plan 09 验证节写明映射包含具名缺口、不等于覆盖完成。

映射现为 229 项：人工场景 141、exact 24、其他自动化 10（`Unit` 9、`RuntimeModule` 1）、具名缺口 53、退役 1。

### C1 代码审查第 2 轮（2026-10-15）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、4 SHOULD_FIX；第 1 轮 B1、S2、S5 判定已修复，S1、S3、S4 部分修复。逐项核实后全部接受。

- **R2-1 滚动条交互仍被绘制测试顶替**：plan 09 滚动条第 3 条再拆为 `thumb-geometry`、`emphasis-paint`、`drag-mapping`、`track-hit`
  （app 几何与绘制测试）、`ends-reach-document-ends`（render 测试），以及 `emphasis-trigger`（hover / drag）、`drag-live-update`、
  `track-page-scroll`；后三项需要经过 app runtime 处理路径的测试，列为缺口。
- **R2-2 M37 不能证明重连与回退主屏**：M37 改为断开 StickyMD 所在的非主显示器再重新连接，预期断开后恢复到主显示器且完全可见、重连后
  仍完全可见；新增场景 `reconnect-monitor`，P08-M10 `disconnect-reconnect` 按 `step=disconnect / reconnect` 分别归属。每角色场景 96 → 97。
- **R2-3 anchor 与表格格式绕过**：plan 09 区段改为只在三级及以上标题处结束，anchor 只允许作为区段末尾（之后再有内容即报错）；phase-08 改为
  绑定整个文档（行尾归一），矩阵行（首格为 P08 ID 的表格行）须为五格且 mode 恰为 `Automated` 或 `Manual`。新增反例：anchor 加新条款、
  强调的 mode、强调的 ID。
- **R2-4 mode 由自由文字推断**：`Mode` 改为结构化入口 `Entry`：`Tests { filter }` 须被证据以反引号逐字列出（或为其 `cargo test` 命令的
  过滤参数）且匹配工作区中至少一个 `#[test]` 函数；`Runtime { scenario }` 须被证据以 `-Runtime` 与场景名逐字列出且是 `RuntimeScenario`
  的变体。P08-A11 证据补写 copied-Release `-Runtime`（scenario `WindowShell`），与 P08-A10 的写法一致，状态不变。反例包括只写单元测试却
  提到 runtime 行为的行（P08-A08 形式）、未列出的过滤名、无匹配测试的过滤名与不存在的场景。该检查确认入口真实存在且被文档逐字点名，
  不证明测试内容。

映射现为 237 项：人工场景 142、exact 24、测试 12、Runtime 1、具名缺口 57、退役 1。

### C1 代码审查第 3 轮（2026-10-15）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、5 SHOULD_FIX；R2-1、R2-2 判定已修复，R2-3、R2-4 部分修复。逐项核实后全部接受，R3-2 的
harness 级部分推迟到 C2（理由见下）。

- **R3-1 plan 09 可被 HTML 注释整体隐藏**：plan 09 改为与 phase-08 一样绑定整个文档（行尾归一），四节原文摘要保留用于定位；新增 HTML 注释
  包住所绑定各节的反例。
- **R3-2 词法扫描不等于 harness 测试集合**：扫描范围收窄为根清单列出的工作区成员；它仍是防止过滤名指向空集合的词法检查，不求值 `cfg`、
  注释与模块可达性，P14-A79 如实写明。以已编译 harness 的测试清单核对，需要在单元测试中嵌套 cargo 构建（锁竞争、CI 时长），本批不做；
  C2 把 exact / automated 证据接入 `module_evidence` 时，以模块收据记录的已执行测试核对 `Tests` 入口。
- **R3-3 否定句可满足 Runtime 检查**：证据只取结构化字段：Automated 行的 Evidence 单元格，或节内的 `- Automated entry:` 条目（含其续行），
  正文其他段落不再计入；入口必须是字段开头：测试过滤名为首个反引号内容或其中 `cargo test` 命令的过滤参数，Runtime 场景须以
  copied-Release `-Runtime`（scenario `名称`）开头。新增否定句、非首位过滤名、只在正文出现的命令三个反例。
- **R3-4 滚动条映射仍以底层测试认领 app 行为**：app runtime 没有任何滚动条测试，凡需经过它的子断言均改为缺口。第 1 条：各视图的滚动条
  （source / preview / split）与分栏同步开关；第 2 条拆为 `gutter-geometry`（测试）、`short-note`（隐藏滑块为测试，保留槽宽为缺口）、
  `gutter-runtime`（命中与 Content Zoom，缺口）；第 3 条的到达文首/文末按视图拆分，Source 归 render 测试，Preview 为缺口；第 4 条
  `viewport-only` 按 text、generation、dirty、caret、selection、preedit 六项全部列为缺口；第 5 条补 `preview-document-height`（缺口）；
  第 6 条拆为 `stale-frame-no-thumb`（缺口）、`stale-receipt-identity`（worker 测试）与 `stale-receipt-guard`（缺口）。
- **R3-5 P08-A11 过度声明**：该 Runtime 检查只验证两个存活进程、不同 HWND 与预置的不同 note；Evidence 收窄为这些事实，并注明运行时
  note/config 隔离与退出隔离未驱动（P08-M09 的缺口），状态不变。

映射现为 253 项：人工场景 142、exact 24、测试 11、Runtime 1、具名缺口 74、退役 1。

### C1 代码审查第 4 轮（2026-10-15）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、5 SHOULD_FIX；R3-1、R3-4、R3-5 判定已修复，R3-2 的推迟可接受，R3-3 部分修复。

- **R4-1 / R4-2（自动化 owner 的证据仍可被否定句或 HTML 注释绕过）与批次调整**：`Automated` 目标的检查连续三轮（S4 → R2-4 → R3-3 →
  R4-1/R4-2）都是在自由 Markdown 上做词法判断，每轮修补又出现新的绕过。它只服务 12 个映射、4 处证据字段，而按已批准的分批，exact /
  automated 扩展与收据标签本来就属于 C2，R3-2 的 harness 级核对也已推迟到 C2。因此 C1 不再保留 `Automated` 目标：12 项改为具名缺口，缺口
  文字写明现有入口（phase-03 / phase-05 滚动条维护节的 `cargo test … scrollbar`、P08-A07 的 `phase8_`、P08-A11 的 `-Runtime` WindowShell），
  并删除证据解析、测试名扫描与 Runtime 场景读取。C2 以模块收据记录的已执行测试绑定这些 owner。这是推迟，不是取消；R3-5 对 P08-A11 证据的
  收窄是独立的文档修正，保留。
- **R4-3 `no-stepwise-write`**：reducer 测试只证明 Preview intent 不持久化，不证明拖动只产生 Preview intent；改为单独的缺口（经事件路径的
  单元测试）。
- **R4-4 mixed-DPI 的公式与图片清晰度**：M36 改为跨屏移动后检查公式与图片，预期按新显示器 scale 重新绘制、清晰（投影 plan 09 §DPI 的
  "跨显示器移动重算 scale；公式 cache key 含 DPI；图片按设备像素绘制"）；registry 与 phase-12 同步，场景数不变。
- **R4-5 无首竖线的追踪行被忽略**：治理的矩阵行读取（各阶段共用）对首格为追踪 ID、却不以 `| ` 开头的表格行报错；注册表对照测试同样要求
  P12-M 行使用规范写法。phase 00 对全部阶段文档通过；新增反例单元测试。

映射现为 253 项：人工场景 142、exact 24、具名缺口 86（其中 11 项待 C2 绑定现有自动化的模块收据）、退役 1。

### C1 代码审查第 5 轮（2026-10-15）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、2 SHOULD_FIX、1 NEEDS_VERIFICATION；R4-1..R4-4 判定已修复，接受把 `Automated` 目标推迟到
C2（与 R3-2 同一依据），R4-5 部分修复。逐项核实后全部接受。

- **R5-1 格式化的行 ID 仍可绕过**：治理的矩阵行读取改为按表结构判定：首格为 `ID` 的表头开始一张矩阵表，其分隔行之后到无竖线的行为止，
  每个数据行都必须以竖线加空格开头且首格是不带格式的行 ID（字母数字段以连字符相连、末段为可选字母加数字，覆盖 `P08-D001`、`P11B-A06`、
  `UX10-01`、`REL-CLI-01`）；强调、代码跨度、HTML 包裹或无外侧竖线的 ID 都报错，与内联写法无关。注册表对照测试改为复用该读取。phase 00
  对全部阶段文档通过。
- **R5-2 G3-03 可空通过**：四个强杀点都接受旧 note，且不确认编辑与保存已开始，保存晚于强杀即可通过；`crash-reconciliation`
  `point=note-save` 改为具名缺口（C2：确认编辑与持久化阶段后在确定性中断点强杀，并以保存未开始的负对照排除空通过）。
- **R5-3 composition 守卫无法与焦点守卫区分（待验证）**：G4-06 在聚焦时组字，焦点本身就阻止自动收起，所以只能证明"真实 IME、聚焦、
  左 dock 时不收起"。该映射保留为对行为的观察，另为 P08-D055 增加 `composition-guard-wiring` 缺口（经 app runtime 的守卫接线与失焦取消
  preedit；目前只有 P08-A05 的 reducer 覆盖）。

映射现为 254 项：人工场景 142、exact 23、具名缺口 88、退役 1。

### C1 代码审查第 6 轮（2026-10-15）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、2 SHOULD_FIX、2 NICE_TO_HAVE；R5-2、R5-3 判定已修复，R5-1 部分修复。逐项核实后全部接受。

- **R6-1 / R6-2 结构检查与各消费者对"哪些行存在"的判断不一致**：根因是检查与读取分离——结构检查用修剪后的首格，冻结 ID、mode/status
  与注册表对照各自按原始前缀读取，双空格、制表符、表内第二个 `ID` 表头或紧随表格的文本行都能让某一方跳过该行。改为单一读取
  `trace_rows`：表格须以空行结束（GFM 会把紧随的文本行当作新的一行）；`ID` 表头须以 `| ID |` 开头并紧跟分隔行；矩阵表数据行须为
  `| <行 ID> | …`（无缩进、竖线后恰一个空格、首格为不带格式的行 ID），表内不能重新进入表头；表外首格为行 ID 的行同样须为该写法。
  `frozen_trace_ids`、`matrix_rows` 与注册表对照（`governance::manual_rows`）都消费它的结果，一行要么被全部检查看到，要么整份文档被拒绝。
  反例单元测试对每个变体同时断言三个消费者都失败。phase 00 对全部阶段文档通过。
- **R6-3 转义竖线**：受限写法不支持转义竖线，遇到即明确报错，而不是误算列数。
- **R6-4 plan 11 人工项范围**：`P12-M01..P12-M44` 同步为 `P12-M47`。

### C1 代码审查第 7 轮（2026-10-15）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、3 SHOULD_FIX、2 NICE_TO_HAVE；R6-3 判定已修复，R6-1、R6-2、R6-4 部分修复。逐项核实后全部接受。

表格读取已连续四轮（R4-5 → R5-1 → R6 → R7）出现新的 GFM 写法绕过，与此前 `Automated` 证据解析的情形相同。本轮采取两条并行措施：读取器只做
代价低、结构性的收紧，并明确声明它是刻意收窄的写法；C1 依赖其完整性的文档另以整文摘要绑定，与已被接受的 phase-08 / plan 09 做法一致。

- **R7-1 非 P 行绕过 mode/status/evidence**：此前只有 `P…` 行进入校验（沿袭旧实现），Phase 10 对 `UX10-*` 的守卫实际不可达。现在矩阵表的
  每个数据行都校验（含 `UX10-*`、`REL-CLI-*`），mode 增加现有文档使用的 `Automated local`；新增修改 UX10 行证据与状态的反例。
- **R7-2 HTML 修饰的表头使整张表退为普通表**：识别表头与行 ID 前先去掉 HTML 标签与 `*`、`_`、反引号；被修饰的 `ID` 表头必须写成规范的
  `| ID |`，任何表中被修饰的行 ID 都报错。注册表对照测试另以 phase-12 整个文档的 SHA-256 绑定，文档任何改动都须复核后更新。
- **R7-3 表头宽度与分隔行未关联**：记录表头列数，分隔行与每个数据行都须同宽；表头之后遇到空行、文本或文档结束都报错；单元格只去掉
  一个外侧竖线，末尾的 `||` 不再被吞掉。
- **R7-4 / R7-5**：phase-14 的人工入口范围同步为 M01..M47，P14-G2 补列 M45、M46；P14-A79 引用的测试名更正，并收窄为上述受限写法加
  整文摘要的表述。

phase 00 对全部阶段文档通过。

### C1 代码审查第 8 轮（2026-10-15）

gpt-6-astra（effort max）：**AGREE**（0 BLOCKER、0 SHOULD_FIX），2 NICE_TO_HAVE、1 NEEDS_VERIFICATION。R7-1..R7-5 及此前各轮判定已修复（R3-2
按批准范围留给 C2）；在已审查摘要常量不变的前提下，未找到能让 C1 人工义务或 P12 人工项静默遗漏而摘要与计数仍全绿的反例。

- **N1 摘要措辞**：摘要按行归一，LF 与 CRLF、末尾单个换行等价；注释与 P14-A79 改为"行内容的任何改动"。
- **N2 修饰识别的范围**：读取器只识别 HTML 标签与 `*`、`_`、反引号修饰；链接、实体等其他行内写法不识别，由整文摘要兜底。注释与 P14-A79
  如实限定。
- **V1 Rust 基线**：审查只读、未运行构建；作者在本 worktree 重跑的结果见下方取证。

### C1 Resolution（2026-10-15）

经 8 轮审查 AGREE。C1 交付：plan 09 退出取消状态与标题优先级、plan 11 被测实例 TwoCopies 例外与人工义务映射；义务表（254 项：人工场景 142、
exact 23（case 级）、具名缺口 88、退役 1）与只在测试中编译的分解、来源摘要和所有者检查；注册表 v3（27 项，每角色 97 个场景）与 M45 / M46 / M47；
记录器对尚无程序的场景前置拒绝；治理的受限矩阵读取。相对设计的调整：`Automated` 目标与 exact 逐检查标签推迟到 C2，届时绑定已执行测试的
模块收据（这 11 项现为写明现有入口的缺口）；多项原归自动化或 exact 的义务经审查改为缺口（D068、note-save 强杀、滚动条 app 路径、
composition 守卫接线等）。人工状态仍为 NOT TESTED；映射完整不等于验收通过。

### C1 取证（2026-10-15，最终工作树）

- Windows：`stickymd-smoke dev-check` 9/9 PASS（governance、fmt、workspace strict clippy、cargo deny、Phase 1 两组、workspace tests、
  Release 构建、native-runtime gate；phase-08 是共享输入，全部模块重跑一次）；`cargo test -p stickymd-smoke --locked` 单元 501 passed /
  17 ignored、CLI 26 passed；`phase 00` PASS。
- Linux（WSL Debian 重放同一工作树）：fmt、`clippy -p stickymd-smoke --all-targets -D warnings` 通过；tests 415 passed / 3 ignored、19 passed。
- 未运行：真实桌面、人工记录与 G3/G4 exact 资格化（本批不改产品 runtime）。

## 覆盖补全 C2 实施记录（2026-10-16）

### 分批

- **C2a**：产品侧的最小纯函数接缝与单元测试，不需要桌面。C2a-1 是 plan 09 驱动的产品改动（标题单一写入者与退出已取消状态、Esc 判定、
  载入外部解除重建义务）；C2a-2 是透明度指针、守卫与失焦、滚动条手势的接缝。
- **C2b**：exact 收据的逐检查标签（plan 11 先行）；以及以已执行测试为证据的 owner 绑定能否成立——模块收据目前不记录测试名，若不能低成本
  补上，相关义务继续作为缺口并写明原因。
- **C2c**：输入注入模块（需要真实桌面回归）。
- **C2d**：G3/G4 exact 扩展与 P08-A11 Runtime 扩展（与 C2c 共用一次独占桌面资格化）。

### C2a-1

- **窗口标题**：`app/title.rs` 的 `window_title` 是唯一把状态投影成标题文字的地方，优先级为恢复 → 退出已取消：保存失败 → 冲突 → dirty →
  公式 hover → 普通；`update_window_title` 是唯一写入者，并缓存上次写入的文字，只在变化时调用原生接口（此前每次指针移动都重写标题）。
  公式 hover 改为状态输入，离开窗口时清除；行为变化：hover 不再盖过恢复、冲突或未保存的 `*`（按 plan 09 优先级）。
- **退出已取消**：`QuitCancelNotice` 只由 `QuitCancelled(NoteSave)` 建立，`QuitCancelled(Paste)` 与 `QuitBlocked` 既不建立也不清除；
  在 `about_to_wait`（一次完整事件处理结束）以 `requires_note_barrier()`（`dirty || durability_required || note_save_in_flight`）为假清除。
- **载入外部解除重建义务（实现漂移修正）**：冲突中的"载入外部"此前只解除冲突，没有解除缺失文件重建义务（干净 reload 路径有），导致
  退出屏障和新标题状态可能永不解除；两条路径统一为 `PersistenceCoordinator::accept_external`，并加单元测试。
- **Esc 判定**：`app/escape.rs` 的 `escape_action` 以 plan 09 顺序一次判定，`handle_key` 对每个 Esc key-down 先经它路由；原分散在透明度
  弹层、编辑区组字、Search、Preview 与 Source 的五处 Esc 分支删除，顺序与行为不变（Search 组字时仍由 IME 自行取消 preedit）。
  "修饰键不改变 Esc、按住时每次 key-down 各自判定"是 3-A 第 1b 批经审查确立的行为，此前只在代码注释中，现写入 plan 09 的 Esc 引导句。
- **plan 09**：Esc 引导句补上述规则；为 Dock 与托盘生命周期两节增加锚点 `dock-auto-hide`、`tray-lifecycle`，供新模块的 `plan_ref` 使用。
  覆盖分解的 plan 09 与 phase-08 整文摘要相应更新（条款摘要不变）。
- **覆盖映射**：相关缺口文字改为指向新的单元测试；它们仍是缺口，C2b 决定以何种已执行证据绑定。
- 验收投影：phase-08 增加 2026-10-16 维护节；coverage-matrix 映射。

#### C2a-1 代码审查第 1 轮（2026-10-16）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、2 SHOULD_FIX；Esc 重构判定行为等价，`accept_external` 判定正确。两项核实后接受。

- **结算时机早于本轮调度**：`settle` 位于 `about_to_wait` 开头，而同一轮随后的 `tick_autosave` 可能再提交保存，使"本轮结束时仍在途"
  的状态被提前判为可清除。改为在 `about_to_wait` 内所有调度（autosave、外部检查、预览与窗口 tick）之后调用 `finish_event_pass`，
  重新读取 guards 后结算并更新标题。该时序需要真实事件循环，单元测试只覆盖结算规则本身，接线仍属缺口。
- **公式 hover 缓存会过期**：缓存的 hover 文字不随帧、布局、模式或文档变化失效。改为不缓存：标题投影时按当前指针与当前 Preview 帧
  派生（指针在窗口内且 Preview 布局为当前时才命中），帧或模式一变即不再显示旧错误。

#### C2a-1 代码审查第 2 轮（2026-10-16）

gpt-6-astra（effort max）：**AGREE**（0 BLOCKER、0 SHOULD_FIX），两项上轮发现判定已修复；2 NICE_TO_HAVE、1 NEEDS_VERIFICATION，前两项已处理：

- 公式 hover 命中排除透明度弹层覆盖的区域（弹层不透明地画在 Preview 之上）。
- 文档区分标题文字的生成函数 `window_title` 与唯一原生写入者 `update_window_title`；新增源码守卫测试，产品源码中只允许一处 `set_title`。
- 待验证项：调度顺序与"不移动鼠标完成 reload / 切模式 / 滚动 / resize"需要真实事件循环，留给 exact 批次，相关义务继续是缺口。
- 取证（2026-10-16，最终工作树）：Windows `dev-check` 9/9 PASS（含 workspace tests 与 Release 构建）；Linux 重放 fmt、clippy、
  stickymd-smoke 415 + 19 passed。

### C2a-2

把运行时里仍混在 `StickyApp` 方法中的判定抽成最小纯函数，行为逐项不变，接线留在原处：

- **滚动条手势**（`app/scrollbar.rs`）：`ScrollbarInteraction` 判定加粗（hover 或拖动中）、按下（拖动开始 / 轨道翻页 90% 窗格高度）、
  指针移动（hover 变化、拖动实时目标、拖动栏消失即结束）与结束拖动；`pane_bars` 与 `preview_scroll` 决定可操作的栏（不接受编辑时一条也没有，
  过期 Preview 帧没有滑块）。`scroll_runtime` 与 `presentation` 只执行这些结果。
- **Preview 回执路由**（`app/preview_runtime.rs::completion_route`）：托盘隐藏时释放、旧文档重建、无视口丢弃、视口不符重排（成功时复用语义树）、
  只有当前文档与视口的回执进入准入规则。
- **透明度指针**（`app/controls.rs`）：`press_slider` / `drag_slider` / `release_slider` 返回偏好 intent；测试经真实的偏好 reducer 确认拖动
  每一步都是不持久化的 `PreviewOpacity`、有效松开至多产生一个 `CommitOpacity`（值变化时 reducer 要求持久化）、Esc 之后松开不产生提交；
  运行时的配置提交与落盘不在测试范围内。
- **窗口守卫**（`app/guards.rs`）：`window_guards` 由 `guard_snapshot` 投影，每个事实只抬起它所属的守卫；编辑区或 Search 字段的组字都阻止自动收起。
- 观察（未改动）：`opacity_dragging` 只由松开、Esc 取消与隐藏清除；输入禁用、失焦、resize 与切换视图都不清除。具体序列：按下滑块 →
  Close/Quit 等待保存并禁用输入 → 左键松开因 `shell_input_enabled` 为假被丢弃 → 保存失败恢复输入，此后不按键的移动仍会预览，下一次被接收的
  左键松开才提交。属于既有行为，记为后续条目；新测试假定松开能到达判定函数。
- 覆盖映射：相关缺口文字改为指向这些判定测试并写明 winit 接线仍未覆盖；它们仍是缺口。验收投影：phase-05、phase-08 的 2026-10-16 补充与
  coverage-matrix。

#### C2a-2 代码审查第 1 轮（2026-10-16）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、2 SHOULD_FIX、2 NICE_TO_HAVE；各抽取判定与基线逐项等价，守卫 15 个事实映射无误。全部核实后接受：

- **验收过滤器漏跑回执测试**：测试名不含 `completion_route`；改名为 `completion_route_admits_only_the_current_build`，以 `--list` 确认过滤器选中。
- **"松开写一次配置"超出证据**：测试只经偏好 reducer 收集 `persist` 标记，不经运行时的配置提交，且拖回原值松开时合法地不持久化；
  注释、报告、phase-08 与缺口文字统一改为"拖动只产生不持久化的预览，有效松开至多一个提交，值变化时要求持久化"，并写明配置提交不在范围内。
- **短路求值边界**：恢复原有的提前退出——透明度布局只在拖动中读取（`drag_slider` 改为延迟取值）、滚动条几何只在接受编辑时读取、Preview
  视口只对可见且当前文档的回执读取。
- **隐藏优先级的竞争用例**：托盘隐藏与旧文档、缺失或不符的视口、失败结果同时成立时都返回 `ReleaseHidden`。
- 透明度残留拖动的既有缺陷补记完整事件序列。

另：USER 2026-10-16 为 C2b 选定方案 A——CI 与 dev-check 的 headless 测试把实际通过的测试名写入绑定提交的执行清单收据，随候选晋升，
readiness 逐个核对单元测试 owner 确实执行并通过。

#### C2a-2 代码审查第 2 轮（2026-10-16）

gpt-6-astra（effort max）：**AGREE**（0 BLOCKER、0 SHOULD_FIX），上轮四项判定三项已修复、一项部分修复；2 NICE_TO_HAVE 已处理：轨道按下只在确认翻页后
读取窗格高度（`PressOutcome::Page` 只带方向，像素由纯函数 `page_pixels` 计算），与基线的读取时机一致；回执路由测试补上"可见、当前文档、视口相符、
构建成功 → `Admit`"的正例。
- 取证（2026-10-16，最终工作树）：Windows `dev-check` 9/9 PASS；Linux 重放 fmt、clippy、stickymd-smoke 415 + 19 passed。

### C2b 设计（2026-10-16，待审查）

USER 2026-10-16 选定方案 A（执行清单收据）。C2b 分两批：C2b-1 exact 逐检查标签，C2b-2 headless 执行清单与单元测试 owner。两批都先改 plan 11。

#### C2b-1：exact 逐检查标签

- **单一来源**：新的平台无关模块 `qualification/exact_checks.rs` 声明 G3/G4 每个 case 的 ID 与必需检查标签集合（静态表）。`g3.rs` / `g4.rs` 的
  case 表、`module_evidence` 的 case 列表、`cli.rs` 的 `--case` 选项与证据 fixtures 都从它派生，消除目前四处重复的 case ID。
- **标签**：`名称` 或 `名称:k=v,k=v`（参数按键排序，规范写法由单元测试校验）；只为每次 PASS 都必然经过的事实设标签，条件分支（如 G3-03 的
  `.tmp` 与恢复标题、G4-06 的 preedit 泄漏分支）不设必需标签。初始标签集合取自本节附带的盘点（G3-01..05、G4-01..06 约 90 个）。
- **记录**：`CaseEvidence` 增加 `checks`；检查函数在断言成立后记录标签（G3 直接在 case 函数内，G4 经 `&mut Checks` 参数，G4-06 经
  `RealImeSession` 字段）。失败的 case 不产生标签。
- **收据 schema 2**：每个结果带 `checks` 数组。`render_receipt` 与 `module_evidence` 都要求记录集合与声明集合完全相等：缺失、多余、重复、
  属于其他 case 的标签与非 PASSED 结果都使收据无效。schema 1 收据不再被接受，现有 G3/G4 账本记录因此失效、需要重跑（预期行为，不是回归）。
- **覆盖映射**：`Target::Exact { case, checks: &[label] }`；覆盖测试直接引用 `exact_checks` 静态表校验每个标签都已声明（不再从源码文本提取
  `id:` 字面量）。未带 profile / producer 参数的义务要求全部变体的标签。
- **plan 11 拟增文字**（"Source freeze、remote artifact promotion 与 exact evidence"一节 G3/G4/G5 成功记录条目之后）：
  "G3/G4 exact 收据为 schema 2：每个 case 结果列出该 case 声明的全部检查标签（由检查函数在断言成立后记录，标签带参数身份），必需集合与
  case 表同源声明；记录与声明不完全相等即无效。schema 1 收据不再有效。"

#### C2b-2：headless 执行清单

- **采集**：`run_captured` 已分别捕获 stdout 与 stderr。cargo 在 stderr 依次打印每个测试二进制的 `Running …` / `Doc-tests …`，二进制在 stdout
  依次打印 `running N tests` 块；按顺序配对第 k 个块与第 k 个二进制，块数不等即失败。每块内收集 `test <路径> ... ok` 行，并核对条数等于该块
  `test result` 行的 passed 数，不符即失败（fail closed）。二进制身份取自 `Running` 行的文件名去掉目录、扩展名与 `-<hash>`（如 `stickymd_win`、
  `cli_exit`），文档测试为 `doc:<crate>`。
- **收据**：headless CI 收据升为 schema 3，cargo test 任务结果带 `tests` 数组（`<二进制>::<测试全路径>`）；readiness 要求合并的 workspace tests 任务
  有非空清单。清单约 1.5k 条，内存与收据体积可忽略。
- **owner 绑定**：覆盖表新增 `Target::Test { binary, path }`；单元测试只校验写法。readiness 在 headless 收据有效并绑定 Source Freeze 时，把未出现在
  清单中的 Test owner 列为缺口（"owner 测试未执行"），清单内的才算有 owner；`acceptance manual list` 单独列出这些 owner 待执行证据绑定。
- **plan 11 拟增文字**（同一节 headless 收据条目）："本地 headless CI 收据（schema 3）的每个 cargo test 任务还记录实际执行并通过的测试全名清单；
  人工义务映射中由单元测试持有的义务，只有其测试出现在当前 Source Freeze 绑定的清单中才算有 owner。"

#### 风险与取舍

- 检查标签由检查函数自行记录，记录调用与断言之间可能漂移；以"记录紧随断言、失败不记录"与审查约束，不另做静态证明。
- 执行清单依赖 cargo / libtest 的人类可读输出格式；格式变化会让采集失败（fail closed），而不是静默少报。稳定的 JSON 输出需要 nightly，不采用。
- 两批都会使相关资格化记录失效一次（schema 变化），预期在 v0.1.3 重建时一并重跑。

#### C2b 设计审查第 1 轮（2026-10-16）与设计第 2 版

gpt-6-astra（effort max）：DISAGREE，2 BLOCKER、6 SHOULD_FIX、1 NEEDS_VERIFICATION；全部核实后接受。第 1 版的执行清单靠解析 libtest 的人类可读输出：
`--nocapture` 下测试正文可以在名字与结果之间打印伪造的 `... ok` 行，计数相等也挡不住（B1）；去掉路径与 hash 的二进制名会在不同 package 间撞名（B2）；
现有性能测试与 Rust 2024 合并 doctest 的正常输出也不满足配对假设（S3）。这与 C1 的自由文本问题同属一类，因此不再修补解析，改换机制。

**C2b-2 第 2 版：单元测试 owner 核验**

- **owner 身份**：`Target::Test { package, target, path }`，`target` 为 `lib` 或 `bin:<名称>` 或 `test:<名称>`（只允许使用 libtest 默认 harness 的目标；
  工作区唯一的 `harness = false` 是 stickymd-core 的 bench，不在 `cargo test` 构建范围内），`path` 为测试全路径。
- **核验任务**：`all --ci`（与 dev-check）增加一个任务，按覆盖表中的 owner 集合执行：
  1. 执行前记录 HEAD 与工作树是否干净；`cargo test --workspace --locked --no-run --message-format=json`，从结构化的 `compiler-artifact`（`profile.test`、
     `package_id`、`target.kind/name`、`executable`）建立目标到测试二进制的映射；缺失或歧义的目标即失败。
  2. 对每个含 owner 的二进制运行 `--list` 与 `--list --ignored`：每个 owner 必须在前者中恰好出现一次、不在后者中。`--list` 不执行测试代码。
  3. 以 `--exact <owner…>` 运行该二进制一次，要求退出码为 0：上一步已确认每个 owner 存在且未被忽略，libtest 会执行全部匹配项，任一失败即非零。
  4. 执行后复核 HEAD 与干净状态不变（复用现有身份采集），否则失败。
- **收据**：任务结果在 headless CI 收据（schema 3，source-only，已由 readiness 绑定 Source Freeze）中列出已核验的 owner 身份；收据由本地 `all --ci` 在冻结源码上产生，
  与现有 headless 门同一路径。相对 USER 选项描述的"随候选晋升"：本机制把证据绑定到 Source Freeze 提交（候选由同一提交构建），不经 release workflow 与 promotion，
  因而不改发布资产；dev-check 只运行核验、不写收据（S4）。
- **readiness 与列表（S6）**：覆盖状态统一计算：Test owner 只有在有效、绑定当前 Source Freeze 的 headless 收据中且已核验集合恰为覆盖表的 owner 集合时才算有 owner；
  收据缺失、无效、陈旧或集合不符时，所有 Test owner 都列为"待执行证明"。`acceptance manual list` 静态列出它们待证明，readiness 普通输出与 JSON 共用同一结果。
- **不再采集全量测试名**，因此 S3 的格式问题、`Command::output` 的内存问题都不再适用；核验的进程输出只用于失败诊断。

**C2b-1 修正**

- `window-flow-unit-tests` 从 G4-02 的标签中删除（S7）：它只检查 `cargo test` 退出码，过滤为零也会通过；该单元测试归 headless。
- `ime-focus-loss-keeps-preedit` 收窄为 `ime-focus-loss-keeps-text`（S8）：现有断言只证明失焦 / 复焦不改变 canonical text。
- schema 2 用于全部 exact 组（G3/G4/G5；G5 声明空检查集合），渲染照常写出失败结果与诊断；"非 PASSED 或标签不符"只使收据不能作为成功证据，不阻止序列化（N9）。
  plan 11 与 `module_evidence` 是全局输入，两批合并后统一重建证据。

#### C2b 设计审查第 2 轮（2026-10-16）与设计第 3 版

gpt-6-astra（effort max）：DISAGREE，1 BLOCKER、4 SHOULD_FIX、1 NICE_TO_HAVE、1 NEEDS_VERIFICATION；R1 的撞名、输出配对、G4-02 零测试标签、preedit 夸大与
schema 问题判定已解决，Source Freeze 绑定判定在现行合同下可接受（须记为对原方案措辞的取舍）。本轮核实后：

- **B1 提前 `process::exit(0)`**：同一进程内一个测试提前以 0 退出会让其余 owner 未执行而退出码仍为 0。改为**每个 owner 单独一个进程**（`--exact <owner>`），
  一个测试的提前退出不再掩盖其他 owner。并写明**威胁模型**：owner 核验针对意外情形——测试被删除、改名、忽略、cfg 排除、过滤失配、失败或未执行；
  它不防御同权限测试代码的蓄意伪造（在自身测试体内提前 `exit(0)`、伪造 `--list` 输出、篡改结果文件），后者属于代码审查的范围。不采用已 deprecated 的
  libtest `--logfile`：它会随工具链移除，也不防同权限篡改。
- **S2 harness 前提**：owner 只允许在 `lib` 与 `bin` 目标（单元测试）中；治理检查要求工作区清单中 `harness` 键只出现在 `[[bench]]`，否则失败，并附自定义 harness 负例。
- **S3 身份贯通**：`all --ci` 在开始时采集一次源码身份（HEAD 与干净状态），owner 核验任务前后复核，收据序列化使用该身份并在写出前再复核一次；
  不再在输出时重新读取 HEAD 为结果署名。
- **S4 范围**：owner 核验只在完整 `all --ci` 中作为 tests 分片任务产生正式结果；定向运行按所选 package 投影 owner，没有适用 owner 时跳过，只作诊断；
  非 Windows 平台不规划该任务（owner 所在的 stickymd-win 只在 Windows 编译）。开发期身份采集允许未冻结或 dirty 的工作树，但其结果不能作为提交资格证明。
- **S5 进程与内存**：所有核验命令经现有 `run_bounded_output`（超时、输出上限、子进程回收）；`--list` 与 cargo JSON 的解析有行数与字节上限；去掉"内存问题不再适用"的说法，
  耗时与峰值在实施后测量报告。
- **N6 列表与状态**：`acceptance manual list` 把 Test owner 列为"证明要求（由 readiness 对照 headless 收据核对）"，不显示为当前执行状态；状态只由 readiness 计算。
- **V7 标签语义**：检查函数内部的记录为幂等集合（同一标签多次成立只记一次），收据仍拒绝重复项；规范参数拒绝重复键与含分隔符的值。

#### C2b 设计审查第 3 轮（2026-10-16）与设计第 4 版

gpt-6-astra（effort max）：无 BLOCKER，3 SHOULD_FIX、1 NEEDS_VERIFICATION、1 NICE_TO_HAVE；逐 owner 进程判定解决了跨 owner 提前退出。全部接受：

- **期间不变性**：开始、任务前后与写出前的复核只能发现漂移，不能证明运行期间源码未被改动后又恢复。正式收据的前提写明：在冻结提交的独占 checkout
  与构建目录上运行，运行期间不得有其他源码或产物写者（与 Source Freeze 的现行前提一致）；复核保留为漂移检查，不宣称期间不变。
- **发现阶段的成功条件**：cargo `--no-run` 构建、`--list`、`--list --ignored` 与每个 owner 的执行都必须成功退出；发现输出须完整（未截断）并解析成功后才做集合判断。
  非零退出、超时、截断或解析失败一律使核验失败，不能贡献已核验 owner；附"ignored 列表失败而执行阶段零退出"的负例。
- **构建范围**：完整运行用 `--workspace`；定向运行只对含适用 owner 的已选 package 使用 `-p`（及 Cargo 自动带入的依赖）；没有适用 owner 时连发现构建一起跳过。
- **输出上限**：owner 执行加 `--nocapture`，让测试输出进入父进程已有的上限控制，而不是积累在子进程的捕获缓冲中；实施后以大量输出与超时用例测量父、子进程峰值与回收。
- **威胁模型按可观察行为划界**：owner 测试必须把控制权正常交回 libtest；测试体自身提前成功退出（无论是否蓄意，包括经辅助函数）、断言遗漏、断言与标签漂移都由代码审查承担。
  exact 收据的"记录集合等于声明集合"证明声明完整，不独立证明断言语义。

#### C2b 设计审查第 4 轮（2026-10-16）

gpt-6-astra（effort max）：**AGREE**，0 BLOCKER、0 SHOULD_FIX。剩余两项：

- NEEDS_VERIFICATION：owner 核验的输出上限、超时与持有管道的子孙进程回收须在 Windows 上实测（父、子进程峰值、返回耗时、存活进程，失败不贡献
  已核验 owner）；属于 C2b-2 实施后的测量，设计不扩大。
- NICE_TO_HAVE：向 plan 11 转录时不得照抄第 1 版拟增文字（它仍写"每个 cargo test 任务记录全部测试名"、只点名 G3/G4）。处理：每批只转录该批的
  最终条款——C2b-1 写 exact schema 2（含 G5），owner 核验条款随 C2b-2 写入。

### C2b-1 实施记录（2026-10-16）

- **plan 11**（`release-artifact-authority`）：新增 exact 收据 schema 2 条款——标签规范写法、一处静态声明、断言成立后幂等记录、`PASSED` 且记录集合
  等于声明集合才算成功、G5 空集合且截图要求不变、失败结果照常写出作诊断、schema 1 无效不迁移、标签相等只证明声明的检查都已到达。
- **单一声明**（新 `qualification/exact_checks.rs`，平台无关）：G3/G4/G5 的 case ID 与检查标签，共 131 个标签（G3-01 13、G3-02 4、G3-03 8、G3-04 11、
  G3-05 4、G4-01 10、G4-02 34、G4-03 8、G4-04 4、G4-05 5、G4-06 30；G5 空）。每个 case 的标签按字节升序书写，与记录集合（`BTreeSet`）的迭代顺序相同，
  因此"记录等于声明"是一次 O(n) 切片比较；不等时才用集合差列出排序后的缺失与多余项（确定性诊断）。`label` 对参数键排序，同一断言的参数顺序不影响
  标签。测试校验规范写法（字母表、键严格升序）、逐 case 严格升序、ID 连续与总数。
- **runner**：`ExactGroup` 由声明与操作数组配对（`[CaseOperation; G.len()]` 编译期等长，运行时再拒绝数量不符）；新的 `judge` 决定结果——返回错误的
  case 不带标签，标签不符的 case 为 FAILED 并保留所记录的标签。`g3.rs` / `g4.rs` / `g5.rs` 不再重复 case ID；`module_evidence` 与证据 fixtures 读取
  同一声明；`--case` 选项保留类型化枚举，由测试确认其恰好接受声明的 case（Windows 上 `as_str` 与声明 ID 一致）；G5 最少截图表由测试对照声明。
- **记录点**：G3 在 case 函数内，`assert_user_asset` 带阶段参数自行记录；G4 经 `&mut Checks`（`cases.rs` 的 `checked` 统一包装）；
  `dock_timing_cycle` 带 Pin 参数并在每个计时断言后记录；G4-06 由 `RealImeSession` 为本输入法记录（自动加 `ime=` 参数），整个 profile 连同模式、
  进程与输入法恢复都成功后才并入 case；`begin_preedit` 只在两个成功出口记录。`run_cargo_test(flow::window)` 调用不变，但不再设标签（设计第 2 版）。
- **收据与校验**：`render_receipt` 写 schema 2，每个结果带 `checks`；`module_evidence::exact` 要求 schema 2 且每个 PASSED 结果的 `checks` 与声明逐项相等
  （缺失、多余、重复、乱序、其他 case 的标签、缺字段、非字符串、schema 1 与带标签的 G5 结果均有负例）。
- **覆盖映射**：23 个 exact 义务改为 `Target::Exact { case, checks }`，列出证明该义务的标签；不带输入法、producer 或 Pin 参数的义务取全部变体——
  M13 为整个 G4-06，M15 剪贴板为整个 G3-01，M15 / D132 导出为整个 G3-02，D117 / D118 为该输入法的全部 15 个标签，Right dock（M02、D040）含 Pin 开
  与关两轮；覆盖测试直接查声明（case 存在、标签已声明、非空、不重复），不再扫描源码中的 `id:` 字面量，并另有测试固定上述"全部变体"的义务。
- **验收投影**：P14-A80（治理范围改为 1..=80），P14-A79 中 exact 目标的描述改指 P14-A80；coverage-matrix 追加映射。
- **失效（预期）**：plan 11 与 `module_evidence` 是全局输入，所有模块的 last-success 记录随之 stale；既有 G3/G4/G5 schema 1 收据无效，需在 exact
  候选上重跑，计划在 v0.1.3 重建时一并处理。
- **未运行**：G3/G4/G5 桌面 exact 用例本身（需要独占桌面）。本批的证据是声明、记录器、runner 判定、收据渲染与校验、覆盖映射的单元测试，以及记录点
  紧随断言的代码审查；记录调用与断言的对应仍由审查承担。
- **验证**（2026-10-16，审查前）：`cargo fmt --all -- --check` 通过；`cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings` 0 警告；
  `cargo test -p stickymd-smoke --locked` 510 passed、17 ignored，另一个测试二进制 26 passed；`tools/smoke/phase-00.ps1` PASS；`dev-check` 9/9 PASSED
  （workspace tests 136.9 s，Release 构建 90.1 s）；Linux 复跑（WSL Debian，3795335 上叠加此后改动的 65 个文件）clippy 退出 0、测试 423 passed /
  3 ignored 与 19 passed。

#### C2b-1 代码审查第 1 轮（2026-10-16）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、3 SHOULD_FIX；131 个标签与 23 个 exact 映射静态展开核对无误。三项都对照源码核实后接受：

- **读取失败被当作空文本**（既有缺陷，新标签依赖它）：`exact_desktop::wait_note` 以 `unwrap_or_default()` 把缺失或非 UTF-8 的 `note.md` 读成空串，
  等待"笔记已清空"的检查（G4-03 剪切 / Undo、G3-01 Undo 等）会在落盘失败时记录成功标签。改为读取失败从不满足等待、在期限内重试，超时报告最后一次
  读取错误；`accepted_note` 带缺失文件、无效 UTF-8、空文件与不符文本的单元测试。
- **空替换也能得到 `ime-replace-commit`**：原判定只要求"文本变化、仍含另一处 term、无 ASCII 字母"，`\n分隔\n中国` 也满足。改为 `replacement_fragment`：
  恰好一处匹配被替换、其余不变，替换片段为单行、非空、不含 term 的 CJK 提交；单元测试含空替换、两处都替换、未替换、残留拼音、保留原匹配后追加等反例。
- **托盘 Hide 未执行却计入 M05**：G4-01 只经 Close 隐藏，托盘菜单项"隐藏"只核对存在。M05 `tray-menu` 按菜单项分解为 `item=show` / `item=hide` /
  `item=quit`：Show 与 Quit 绑定各自的 G4-01 标签，Hide 列为具名缺口（"窗口可见时经托盘菜单 Hide 隐藏"），留待 exact 扩展（C2d）。
- 验证：fmt、clippy 0 警告；`cargo test -p stickymd-smoke --locked` 512 passed、17 ignored，另一个二进制 26 passed。

#### C2b-1 代码审查第 2 轮（2026-10-16）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、4 SHOULD_FIX、1 NICE_TO_HAVE；第 1 轮三项判定已解决、未引入回归；256 个义务参数恰好覆盖一次，M05 拆分后
exact 义务为 24 个。四项都是"标签名称强于其断言"，属于设计中由代码审查承担的那一层；对照源码核实后全部接受，并加强断言而不是收窄标签：

- **重启检查可能早于启动整理**（G3-04 `user-asset-intact:stage=restart`、G3-05 `fake-asset-intact:stage=restart`）：重启时 note/config 已存在，
  `wait_for_layout` 立即返回，探针可能在新进程执行 `main.rs` 的启动资产整理之前就检查并杀进程。新增 `exact_desktop::start_until_editor_ready`：
  复用产品已有的诊断就绪事件（只设 `STICKYMD_DIAGNOSTIC_READY_EVENT`，不写 trace、不退出），等待该次启动的首次 present（整理在编辑器创建之前）再检查。
- **`shift-delete-cuts` 区分不了剪切与删除**：Ctrl+Insert 已把 `TEXT` 放入剪贴板。现在先把剪贴板换成哨兵文本，Shift+Delete 后同时要求文档为空、剪贴板为
  `TEXT`。
- **`redo-restores-asset` 只比较数量**：粘贴后要求 Markdown 引用的正是唯一发布的资产，并记下其路径与 SHA-256；Redo 后要求受管资产恰为该路径且字节不变。
- **`export-leaves-working-note` 只读旧文件**：导出后在 Source 末尾粘贴标记，要求 `note/note.md` 保存为原文加标记、导出副本不变，证明工作文档与保存目标
  未切换。
- NICE_TO_HAVE：G5 截图转移反例因 schema 2 多出 `checks` 而退化为删除反例；匹配串改为新结果形状，并断言转移前后截图总数不变。
- 这些加强的探针只能在独占桌面的 exact 运行中执行，本批未运行；它们只会让既有路径更严格（误判只会表现为 FAILED，不会产生假 PASS）。验证：fmt、clippy 0 警告；
  `cargo test -p stickymd-smoke --locked` 512 passed、17 ignored，另一个二进制 26 passed。

#### C2b-1 代码审查第 3 轮（2026-10-16）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、3 SHOULD_FIX；第 2 轮各项已落实，就绪事件确在启动资产整理之后。全部核实后接受：

- **Preview 的非法剪切与粘贴可以互相抵消**：原探针连续执行全选、Shift+Delete、Shift+Insert 后才比较正文。Preview 的命令映射把两者都映射为无操作
  （`preview_input::preview_command`），现在分别验证：剪切后正文不变且剪贴板仍是哨兵，再换一个哨兵后粘贴，正文仍不变。
- **导出正文未绑定实际复制的图片**：现在先确认恰好复制了一个与原图字节相同的文件，再用它的文件名构造此固定 fixture 的完整预期 Markdown（本地图片指向
  `g3-export-assets/<该文件>`，远程引用与其余正文不变）逐字比较。
- **重启探针可能继承其他诊断开关**（本批新代码）：子进程继承父环境，残留的 `STICKYMD_DIAGNOSTIC_EXIT_AFTER_READY=1` 会在设置就绪事件后生效，使候选首帧后
  自行退出。启动前显式移除退出开关、trace 与细节路径。
- 验证：fmt、clippy 0 警告；`cargo test -p stickymd-smoke --locked` 512 passed、17 ignored，另一个二进制 26 passed；桌面探针未运行（同第 2 轮）。

#### C2b-1 代码审查第 4 轮（2026-10-16，逐标签审计）

为收敛，本轮要求审查者逐个审计全部 131 个标签，一次列出所有剩余缺口。gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、6 SHOULD_FIX、1 NEEDS_VERIFICATION；
其余标签逐项判定为断言与名称相符，第 3 轮三项已解决、未发现回归。六项核实后处理如下：

- **S1 资产枚举把读取错误变成"没有文件"**（G3-01 数量 / 移除 / 恢复、G3-02 复制数量）：`regular_files` 只把目录不存在视为空，打开列表、读取目录项或其类型
  失败都返回错误；附缺失目录、文件与子目录混合、对文件路径列目录三种用例。
- **S2 失焦检查可能读到旧落盘文本**（`ime-focus-loss-keeps-text`）：失焦 / 复焦后先等过自动保存 debounce（650 ms）的观察窗口，要求标题不为 dirty 且
  `note.md` 仍为基线，再进入下一轮组字，避免后续恢复掩盖前一阶段的提交。
- **S3 七个 `snap-*` 标签缺少输入与终态几何证明**：拖动允许光标误差，终态几何未回读。探针照常运行（配置不符仍使 case 失败），但不再声明标签；
  标签总数改为 124（G4-02 为 27）。核验实际输入几何与终态几何后再设标签，列入 exact 扩展（C2d）。这七个标签此前没有被覆盖映射引用。
- **S4 图片快捷键标签只证明引用前缀**：要求正文中恰有一个受管图片引用且文件已发布；file-drop 的发布文件与原 PNG 逐字节相同（PNG 原样保存），DIB 的发布
  文件是 2×2 PNG（产品把 DIB 转为 PNG）；附引用解析与 PNG 头检查的单元测试。
- **S5 `junction-no-durable-mutation` 只观察 note/config**：标签改名为 `junction-keeps-note-and-config`，只声明实际观察的两个文件；plan 11 的第二实例零
  durable write（资产、trash、恢复文件）不由此标签证明。M08 未引用该标签。
- **S6 Preview 剪切检查没有确认选区存在**：先在有界等待中执行全选与复制，回读到全文后才设置剪切哨兵，证明 Preview 已有画面且选区非空。
- NEEDS_VERIFICATION：加强后的桌面探针尚无实跑证据，需在独占桌面上运行适用的 G3/G4 后核验收据（与 v0.1.3 重建一并进行）。
- 验证：fmt、clippy 0 警告；`cargo test -p stickymd-smoke --locked` 515 passed、17 ignored，另一个二进制 26 passed。P14-A80 的标签数同步为 124。

#### C2b-1 代码审查第 5 轮（2026-10-16）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、2 SHOULD_FIX、1 NEEDS_VERIFICATION；S1、S2、S3、S5、S6 在源码层面成立，124 个标签与 24 项 exact 映射与声明一致。
两项都在 S4 的修复里，核实后处理：

- **引用解析接受不成立的 Markdown**（缺右括号、代码跨度、额外正文）：产品单图粘贴插入的正文恰为 `![](images/<name>)`（`assets::transaction`），fixture
  从空文档粘贴，因此改为要求全文恰好等于该写法、文件名只含字母数字与 `-` `.`；负例含缺右括号、代码跨度、尾随换行、额外正文、两张图与路径穿越。
- **PNG 头检查接受损坏文件**：smoke 工具只用标准库，不引入解码器；DIB→PNG 转换由 `stickymd-render` 的图片测试负责。两个标签按 G3-01 资产标签（第 4 轮判为
  相符）的口径改名，只声明快捷键走到了图片粘贴路径并发布了被引用的资产：`shift-insert-dib-publishes-asset`（全文恰为该引用、文件存在、PNG 头为 2×2，
  头部只是健全性检查）与 `shift-insert-file-drop-publishes-asset`（全文恰为该引用、发布文件与原 PNG 逐字节相同）。这两个标签不被覆盖映射引用。
- 验证：fmt、clippy 0 警告；`cargo test -p stickymd-smoke --locked` 515 passed、17 ignored，另一个二进制 26 passed。

#### C2b-1 代码审查第 6 轮（2026-10-16）与结论

gpt-6-astra（effort max）：**AGREE**，0 BLOCKER、0 SHOULD_FIX；第 5 轮两项修复成立，全量核对未发现声明、记录调用、schema 2 校验与覆盖映射之间的新冲突
（124 个标签、24 项 exact 映射）。仍待：加强后的 G3/G4 桌面探针在独占桌面上的实跑（NEEDS_VERIFICATION，随 v0.1.3 重建一并进行）。

最终验证（提交前，同一工作树）：`cargo fmt --all -- --check` 通过；`cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings` 0 警告；
`cargo test -p stickymd-smoke --locked` 515 passed、17 ignored，另一个二进制 26 passed；`dev-check` 9/9 PASSED（首次运行的 dependency policy 因拉取 RustSec
advisory-db 时 TLS 握手失败而失败，网络恢复后重跑通过；workspace tests 122.7 s，Release 构建 95.8 s）；Linux 复跑（WSL Debian，3795335 上叠加 65 个文件）
clippy 退出 0、测试 423 passed / 3 ignored 与 19 passed。

### C2b-2 实施记录（2026-10-16）

本批实现 owner 核验机制，并绑定第一批三项义务；其余 C2a 缺口在下一批逐义务映射后绑定。

- **plan 11**（`release-artifact-authority`，headless 收据条目之后）：一条写 smoke 证据 schema 3 与源码身份；一条写单元测试 owner 的声明、核验、
  readiness 判定、执行前提与证明边界（按第 4 轮审查 NICE_TO_HAVE，转录最终合并条款，不是第 1 版草案）。
- **`unit_owners`**（新）：`Owner { package, target (lib | bin:<名称>), path }`，记录为 `package/target/path`。`artifacts` 解析 cargo
  `--message-format=json`：只取 `profile.test` 且 `executable` 非空的 `compiler-artifact`，包名按 Package ID Specification 取（本工具链的
  `path+file:///…/stickymd-win#0.1.3` 省略了与目录同名的包名），只认 `["lib"]` 与 `["bin"]` 目标，流须以成功的 `build-finished` 结束且其后无消息。
  `listing` 严格解析 libtest `--list`：逐行 `name: test|bench`、有条目时一行空行、最后一行摘要（单复数与规范数字），计数须与条目一致。
  `verify`（Windows）：无 owner 时连构建也跳过；构建、两次列出与每个 owner 的 `--exact <path> --nocapture` 都经有界执行器并要求成功退出；
  任一失败即整个任务失败、不返回 owner；前后复核源码身份。执行调用可注入，单元测试覆盖已列出 / 忽略 / 缺失 / 重复 / 截断 / 执行失败，以及
  "ignored 列表失败而 owner 本会通过"（第 4 轮设计要求的负例）。
- **有界执行器**：`run_bounded_output` 从 `exact_desktop` 移到 `managed_process::bounded`，增加显式上限参数（cargo JSON 16 MiB；列表与 owner
  1 MiB），行为不变，`exact_desktop` 保留原签名包装，既有持管道子孙、超量输出与超时测试照常运行。
- **证据 schema 3**：每个结果带 `owners`（只有 owner 任务可非空）。`SourceIdentity` 在 `runner::execute` 开始时采集，owner 任务前后与写出前复核，
  漂移即追加 FAILED `source identity` 结果、照常写出并使命令失败；中间进度与单步收据仍在写出时读取身份。读取者一并升级：`automated_readiness`、
  `module_evidence::smoke`、`resource_plan::coverage`、资源诊断 payload；`timing_summary` 只读测量值，接受 2 与 3。
- **runner**：`Task::UnitTestOwners` 只在 Windows 的 `Selection::All` 中、workspace tests 之后规划（owner 所在包只在 Windows 构建）；
  `modules run` 与 dev-check 的投影与 Cargo 任务同规则——全部 workspace 包被选中时不变，否则只保留被选中且持有 owner 的包，一个都没有时整项跳过；
  这些运行不写收据。
- **readiness**：`check_document` 要求 schema 3，owner 只从 owner 任务结果读取且须与映射的 owner 集合完全相等，其他结果不得列 owner，映射有 owner
  而计划没有 owner 任务即无效；同一结论既决定阻断，也决定 `unit_test_owned.verified`（普通输出与 JSON 同源）；`acceptance manual list` 把这些义务
  列为证明要求。
- **治理**：工作区与已登记清单的 `harness` 键只允许在 `[[bench]]`（区分键与值、注释与更长名称），附负例。
- **指纹分类**：`qualification/mod.rs` 读取 `manual_observation::coverage` 的 owner 集合列入例外：它只被 headless owner 任务（收据绑定 Source
  Freeze 提交而非模块指纹，且没有功能模块运行规划它）与 readiness 使用。
- **首批绑定**：plan 09 `esc-2` transient-guards `state=asset-sync` / `state=quitting` → `app::escape::tests::each_guard_alone_ignores_esc`
  （分别对应 `asset_reconcile_pending` 与退出期间输入关闭后的 `accepts_editor_mutation = false`），`esc-6` collapse `modifier=ctrl` →
  `app::escape::tests::modifiers_never_change_esc`（USER 2026-10-15 批准这些 Esc 行仅单元覆盖）。
- **端到端**（未冻结的 dirty 工作树，仅作开发证据）：`modules run windows --mode=tests` 通过（owner 任务投影为 `-p stickymd-win`）；
  `all --ci --ci-shard=tests --json` 产生 schema 3 证据，owner 任务 PASSED、列出两个 owner，耗时 4.2 s（workspace tests 209.1 s）；把一个 owner
  临时改成不存在的测试后 `modules run windows` 以 "owner … is not listed" 失败（已恢复）。另一次运行中我在运行期间改了源码，owner 构建需要重编译
  正在运行的 `stickymd-smoke.exe`，Windows 拒绝覆盖而失败（fail closed）；这也说明 HEAD / dirty 身份看不到已 dirty 工作树内的内容改动，正式收据
  依赖独占干净 checkout 的前提。
- **测量**：父进程对每条流最多缓冲上限加 1 字节（owner 与列表 1 MiB，cargo 16 MiB），子孙进程随作业结束；子进程自身的内存不受此限，属于被测代码。
- **验证**（审查前）：`cargo fmt --all -- --check` 通过；`cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings` 0 警告；
  `cargo test -p stickymd-smoke --locked` 528 passed、17 ignored，另一个二进制 26 passed；`tools/smoke/phase-00.ps1` PASS；`dev-check` 10/10 PASSED
  （新增的 unit-test owner verification 3.3 s，workspace tests 206.1 s，Release 构建 147.9 s）；Linux 复跑（WSL Debian，3795335 上叠加 95 个文件）
  首次发现非 Windows 非测试构建中解析器是死代码、一个夹具测试按路径分量比较 Windows 路径，改为 `cfg(any(windows, test))` 与整串比较后 clippy 退出 0、
  测试 433 passed / 3 ignored 与 19 passed。

#### C2b-2 代码审查第 1 轮（2026-10-16）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、3 SHOULD_FIX、1 NEEDS_VERIFICATION、1 NICE_TO_HAVE；三项 Esc 绑定未发现语义错绑。全部核实后处理：

- **`harness` 治理漏检与误报**：行级启发式不区分引号键（`"harness"`、`'harness'` 与转义写法）、字符串值与注释。改为
  `governance/manifest_harness.rs`：按 TOML 词法跟踪基本 / 字面字符串（含多行与转义）、注释、表头与内联表 / 数组，找出每个键（裸键或引号键）及其
  所属表；只有 `[[bench]]` 表或根级 `bench` 内联值里的 `harness` 被允许；无法跟踪的输入（未闭合字符串、表头或括号，顶层逗号）fail closed。测试含
  引号键、转义键、字符串与多行字符串中的文字、注释、数组中的字符串、多行内联数组、`[package]` 下的 `bench`，以及工作区五个真实清单。
- **失效的 Source Freeze 仍能得到 `verified: true`**：readiness 中 Freeze 读取与仓库校验抽为 `read_source`，返回 Freeze 与"仍对当前仓库成立"两个值；
  owner 证明改为 `headless_ok && source_current`。附回归测试：Freeze 与仓库不符时仍返回该 Freeze 但不成立。
- **非 headless 读取者没有落实 schema 3 的 `owners`**：新增 `evidence::require_no_owners`，`module_evidence::smoke`（每个结果）、
  `resource_plan::coverage::validate_group_result` 与资源诊断缓存 payload 都要求字段存在且为空数组；测试覆盖 Runtime、Performance 与资源收据中
  缺字段、类型错误与非空数组。
- NICE_TO_HAVE：readiness 期望的 owner 字符串改为按字符串排序，与核验写出的顺序一致（同一包同时有 lib 与 bin owner 时不再误判）。
- NEEDS_VERIFICATION（故障路径测量）：单独运行有界执行器的三个故障测试并采样父进程峰值工作集——子进程写出超过 1 MiB 上限：0.41 s、峰值 11.6 MiB；
  超时：1.20 s、10.2 MiB；子孙进程持有管道：4.53 s、10.2 MiB；三次运行后都没有残留的测试进程。owner 任务失败时收据中的 owners 见下一条记录。
- owner 任务失败时的收据：把一个 owner 临时改成不存在的测试后运行 `all --ci --ci-shard=tests --json`，schema 3 证据中 owner 任务为 FAILED
  （"owner … is not listed"，3.5 s）、`owners` 为空，命令以 1 退出；之后已恢复映射。
- 验证：fmt、clippy 0 警告；`cargo test -p stickymd-smoke --locked` 532 passed、17 ignored，另一个二进制 26 passed。

#### C2b-2 代码审查第 2 轮（2026-10-16）

第一次运行因 WSL 服务错误与 Codex 中途退出没有产生结论（环境故障，不是审查结论），重跑后 gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、
2 SHOULD_FIX、1 NICE_TO_HAVE；第 1 轮的 Source Freeze 门控、非 owner 结果空数组校验与 owner 排序已确认。两项都在新扫描器的字符串处理，核实后接受：

- **多行字符串提前结束**：TOML 允许结束定界符前有一至两个内容引号（`"""hello""""`）。原实现在第一个三引号处结束，剩余引号与注释里的引号重新组成字符串，
  能把其后的 `[[bin]]` 与 `harness = false` 吞进字符串而漏检；去掉注释时又误拒合法写法。改为：第一段三个及以上的连续引号结束字符串，其中最多两个是
  内容，超过五个即无效；基本与字面多行字符串同样处理。
- **行尾反斜杠后的空白被拒绝**：TOML 允许反斜杠与换行之间有空格或 Tab。`escape` 改为只在多行基本字符串中接受"反斜杠、可选空白、换行"并修剪其后
  所有空白与换行；单行字符串中的续行与"反斜杠加空格加非换行"仍判为无效。
- NICE_TO_HAVE：P14-A81 的测试入口改为 `governance::manifest_harness::tests`。
- **差分测试**（为不再一轮轮挖词法边界）：Python 生成器随机组合表头（含引号与空格写法）、裸键 / 引号键 / 转义键、各类字符串（含结尾引号段、续行、
  内含 `harness = false` / 注释 / 括号的内容）、数组与内联表与注释，用 `tomllib` 保留合法清单并按"`harness` 只能是根级 bench 目标的直接字段"
  计算期望，再用临时的忽略测试让扫描器读同一语料。种子 20261016 的 20 000 例（9 613 例违规）与种子 777 的 100 000 例（50 816 例违规、51 315 例含多行
  基本字符串、约 9 000 例含续行）均 0 不一致；第一次比较的 529 例差异来自我的预言过宽（把 bench 目标内更深的嵌套 `harness` 也当允许），按规则收窄后
  消失。临时测试已删除，不进入提交。
- 验证：fmt、clippy 0 警告；`cargo test -p stickymd-smoke --locked` 533 passed、17 ignored，另一个二进制 26 passed。此前一次完整运行中
  `a_descendant_holding_the_output_is_ended_with_the_helper`（54.7 s）与 `a_candidate_that_exits_at_once_is_a_startup_failure`（等满超时）失败，
  当时 E: 盘只剩约 6 GB、WSL 中其他项目的构建正在大量写盘；腾出空间后两项都通过，判定为环境压力而非回归。

#### C2b-2 代码审查第 3 轮（2026-10-16）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、2 SHOULD_FIX；第 2 轮两项字符串修复符合 TOML 1.0，owner 核验、schema 3 消费链、readiness、
投影与三项 Esc 绑定未发现其他应修问题。两项核实后接受：

- **bench 目标内的嵌套 `harness` 被当作直接字段放行**：判断只看外层上下文是否为 `bench`，丢失了点分键前缀与内联嵌套层级——`[[bench]]` 下的
  `meta.harness = false`、`bench = [{ meta = { harness = false } }]` 被放行，而语义等价的 `[bench.meta]` 下的 `harness` 被拒。改为按"形状"跟踪
  完整路径：键与数组元素是不同的部分（`[[bench]]` 后的 `harness` 是 `bench[].harness`，其下的 `meta.harness` 是 `bench[].meta.harness`），
  只允许 `bench[].harness` 与 `bench.harness`；表头的最后一个键同样受此规则约束（`[harness]`、`[[test.harness]]` 被拒）；没有键的括号只允许作为
  数组元素，其他位置 fail closed。
- **缺少 TOML 1.1 的 `\xHH` 转义**：Cargo 自 1.94 接受 TOML 1.1，项目锁定 Rust 1.97.1；`description = "\x53tickyMD"` 这样的合法清单会被误拒。
  增加两位十六进制转义（单行与多行基本字符串、键与值）。
- 差分测试补充：本机 Python 3.15 的 `tomllib` 已支持 TOML 1.1（`\xHH`、内联表换行与尾随逗号），生成器加入 `\xHH`、点分键（含 bench 内）、
  `[harness]` 等表头、根级 `bench.harness`、跨行与带尾随逗号的内联表，并去掉此前排除这些形状的过滤。种子 31337 的 100 000 例（75 641 例违规、
  21 566 例含 `\x`、19 884 例 bench 内点分键、12 733 例 `[harness]`）与此前两个语料（20 000 与 100 000 例）均 0 不一致；临时测试已删除。

#### C2b-2 代码审查第 4 轮（2026-10-16）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、1 SHOULD_FIX；第 3 轮的 bench 嵌套路径与 `\xHH` 修复未发现新问题。核实后接受：

- **点分键与表头中的父键 `harness` 漏检**：只检查路径的最后一个键，`[package.metadata]` 下的 `harness.enabled = false`、
  `[package.metadata.harness.settings]` 被放行，而等价的 `harness = { enabled = false }` 被拒。TOML 的点分键与表头为路径上的每一级命名表，所以改为
  `misplaces_harness`：路径上每个名为 `harness` 的键，都以截至该键的前缀检查是否为 bench 目标的直接字段；表头与赋值键共用。负例补点分键、内联点分键、
  子表头与数组表头。
- 差分：生成器加入父键位置的 `harness`（`harness.enabled`、`meta.harness.x`、`[package.metadata.harness…]`、`[[harness.x]]`、
  `[bench.harness.deep]` 等）。种子 4242 的 100 000 例（81 988 例违规、55 592 例含父键位置的 `harness`、18 012 例允许）与此前三个语料共 320 000 例
  均 0 不一致；临时测试已删除。
- 期间一次 dev-check（CPU 100%）中 stickymd-win 的 `preview::worker::tests::document_projection_release_forces_the_next_visible_request_to_rebuild`
  因工作线程接收超时失败；本批未改 `apps/` 与 `crates/`，单独重跑 5 次均通过，判定为负载造成的既有时序敏感。

#### C2b-2 代码审查第 5 轮（2026-10-16，逐构造审计）

本轮要求审查者逐类列举 Cargo 1.97 接受的 TOML 1.1 构造并判定扫描器能否正确定位每个 `harness` 键。gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、
1 SHOULD_FIX；裸 / 引号 / 点分键、表头与隐式父表、数组表、内联表（空、嵌套、多行、注释、尾随逗号）、全部字符串形式与转义、注释、数组、数值与日期、
空白与 CRLF 均判为正确，owner 核验、证据与 readiness、计划与分类、三项绑定未发现新问题。唯一一项核实后接受：

- **文件开头的 UTF-8 BOM 使合法清单被误拒**：Cargo 1.97 的 TOML 词法器（`toml_parser` 1.1.2）跳过文件开头的一个 BOM，扫描器把它当作非法键字符。
  改为扫描前只去掉开头的一个 BOM；测试含 BOM 加合规清单通过、BOM 加非 bench 的 `harness` 仍被拒、两个 BOM 与字符串内的 U+FEFF 照常处理。
- 本轮期间两次完整运行中各有一个既有时序敏感测试失败：stickymd-win 的两个 Preview worker 测试（工作线程接收超时）与 smoke 的
  `managed_process::tests::stale_smoke_preflight_fails_without_killing_the_existing_process`（结束子进程后立即删除夹具目录失败）；本批未改 `apps/`、
  `crates/` 与这些测试，stickymd-win 测试单独连跑 3 次（287 passed）与该 smoke 测试单独连跑 3 次均通过，判定为机器负载下的既有不稳定。

#### C2b-2 代码审查第 6 轮（2026-10-16）与结论

gpt-6-astra（effort max）：**AGREE**，0 BLOCKER、0 SHOULD_FIX；BOM 修复只去掉开头一个 U+FEFF，与上游 lexer 一致；owner 核验、schema 3 消费链、身份漂移、
readiness、任务投影与三项 Esc 绑定未发现失败后仍贡献 owner、误收旧证据或错绑路径。

最终验证（提交前，同一工作树）：`cargo fmt --all -- --check` 通过；`cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings` 0 警告；
`cargo test --workspace --locked --no-fail-fast` 中 stickymd-smoke 534 passed / 17 ignored 与 26 passed、core 与 render 及 phase-1 实验全部通过；
stickymd-win 有 2 个 Preview worker 测试在当时 CPU 96% 的负载下因接收超时失败（本批未改 `apps/` 与 `crates/`，stickymd-win 不依赖 stickymd-smoke，
单独运行 stickymd-win 测试连续 3 次 287 passed）；Linux 复跑（WSL Debian，3795335 上叠加 96 个文件）clippy 退出 0、测试 439 passed / 3 ignored 与 19 passed。
本批之初的 dev-check 曾 10/10 通过；此后的 dev-check 均止于上述负载下的既有时序敏感测试，没有完整通过的最终 dev-check。

### C2b-3 实施记录（2026-10-17）

本批逐义务审计 C2a 留给单元测试的 42 项缺口（`UNIT_SCROLL_RUNTIME`、`C2_SCROLL_SHELL`、`C2_SCROLL_RENDER`、`C2_OPACITY_REDUCER`、
`UNIT_OPACITY_DRAG`、`UNIT_QUIT_STATE` 与两项内联缺口），只绑定能被现有或补强后的单元测试完整证明的义务。

- **方法**：每项义务先由一个映射者读 plan 条款、声明的断言与参数和 `--list` 出的测试名，给出唯一 owner 或"无"；提出 owner 的再由三个独立反驳者
  各自尝试证伪（默认倾向证伪），多数未证伪才保留。结果：17 项提出 owner、25 项无候选；17 项中 9 项保留、8 项被多数证伪。保留的 9 项里有 6 项各被
  一位反驳者证伪，逐条核实其理由后补强（见下），不是按票数直接接受。
- **plan 11**（`release-artifact-authority` 的 owner 条目）：owner 声明改为"一个或多个 owner，全部核验才算有 owner"。这是 C2b-2 已批准合同的
  细化（集合而非单值），不改变核验、收据与 readiness 判定；`Target::Test(&'static [Owner])`，`test_owners` 取并集，覆盖测试要求非空且不重复。
- **补强与新增接缝**（针对单票证伪的理由）：
  - emphasis 绘制：原测试只断言加粗后像素多于静止时，静止宽度为 0 也能通过；补 `thin_count > 0`（每个 DPI）。
  - hover 触发：原测试把"光标在哪条 gutter"作为参数直接传入，命中判定在 `StickyApp` 方法里没有测试；抽为纯函数 `app::scrollbar::gutter_at`
    （`scroll_runtime.rs` 只负责取几何与 DPI 后调用），新测试 `the_gutter_is_the_band_right_of_each_pane` 覆盖两侧边界、上下边界、
    缺失窗格与 150% DPI；该义务由 gutter 判定与 emphasis 判定两个 owner 共同持有。
  - 轨道命中：原测试只按 Source 一条栏；补 Split 中按 Preview 轨道上下两侧得到 `Page { pane: Preview, direction: ±1 }` 且不开始拖动。
  - Source 两端可达：`caret_rect(text.len()).is_some()` 对被裁掉一半的末行也成立；改为断言末行完整位于 220 px 视口内。
  - 标题优先级：plan 条款含"唯一函数"，只测优先级不够；与 `the_title_has_one_native_writer` 一起作为两个 owner。
  - 载入外部解除重建义务：反驳者认为测试未覆盖"外部删除"语境与运行时调用点。核实：测试的义务正是由 `RecreateMissing` 写入失败产生（外部删除后的
    重建义务），再经冲突载入 `accept_external` 解除；`IgnoreKnown`（重新出现的已知内容）走 `confirm_durable_present`，不是"载入外部"；运行时的两处
    载入（`ReloadClean` 与冲突载入）都调用 `accept_external`。接缝与其他绑定一致（单元决策，接线另计），不补强，直接绑定。
- **绑定的 9 项**（plan 09）：`scroll-3` emphasis-trigger `trigger=hover`（2 个 owner）/ `trigger=drag`、emphasis-paint、drag-mapping、
  track-hit、ends-reach-document-ends `view=source`（stickymd-render lib owner，首个 `lib` 目标）；`quit-2` established-only-by-save-failure
  `cause=paste-unconverged`、external-load-releases-rebuild；`quit-3` title-priority（2 个 owner）。`unit_owners` 中 `Lib` 的 dead_code 预期随之删除。
- **被多数证伪的 8 项**（保留缺口）：thumb-geometry（只测 dpi 1 的最小高度，未测按比例与按 DPI）、drag-live-update、track-page-scroll、
  viewport-only `state=preedit`、stale-frame-no-thumb、stale-receipt-identity（只测偏移相同的视口不等，未测只差偏移的旧回执被丢弃）、
  stale-receipt-guard、no-stepwise-write（只断言 reducer 的持久化请求，不读 config，而运行时按 config dirty 写盘）。
- **无候选的 25 项**：视图模式到窗格栏的组合、`StickyApp` 的 winit 接线（hover、拖动、翻页、结束拖动、回执应用）、对文档状态不变的断言、
  gutter 几何避让、短笔记隐藏、Source 映射缓存、Preview 文档高度、透明度范围与输入、守卫阻止的退出、屏障判定与每次事件处理后的清除、组字守卫接线。
- **缺口文案**：各 gap 常量与内联文案改为"C2b-3 审计：……"，写明现有测试未证明的部分与预期的补强或 exact 层，替换此前"C2 / C2b 绑定模块收据"的
  计划文字（那些文字假设整组测试能持有义务，审计后不成立）。
- **回归**：首次 `all --ci --ci-shard=tests` 中 `runner::headless::tests::requesting_several_modules_deduplicates_shared_commands` 失败——它假设
  `core,render` 只规划两个任务，render 有了 owner 后多出投影到 `stickymd-render` 的 owner 任务（行为正确）。测试改为把 owner 任务分开：至多一个，
  其余仍是去重后的两个任务；是否规划 owner 任务由映射决定，投影本身由 `owner_verification_projects_onto_the_selected_owning_packages` 覆盖。
- **验证**（审查前，未冻结的 dirty 工作树，仅作开发证据）：`cargo fmt --all -- --check` 通过；`cargo clippy -p stickymd-smoke -p stickymd-win
  -p stickymd-render --all-targets --locked -- -D warnings` 0 警告；`cargo test -p stickymd-win -- app::scrollbar` 8 passed、`cargo test -p
  stickymd-render --lib -- source::scrollbar` 4 passed；`modules run windows,render --mode=tests` PASS（stickymd-win 288 passed）；
  `all --ci --ci-shard=tests --json` 全部 PASSED（workspace tests 207.4 s），schema 3 证据中 owner 任务 PASSED、8.3 s、列出 12 个 owner（11 个
  `stickymd-win/bin:stickymd-win`，1 个 `stickymd-render/lib`）；Linux 复跑（WSL Debian，3795335 上叠加 97 个文件）stickymd-smoke 与
  stickymd-render clippy 退出 0、测试 439 passed / 3 ignored、19 passed 与 render 165 passed / 13 ignored 等全部通过。

#### C2b-3 代码审查第 1 轮（2026-10-17）

gpt-6-astra（effort max）：DISAGREE，3 BLOCKER、4 SHOULD_FIX、1 NICE_TO_HAVE；多 owner 机制（并集、`describe`、readiness 严格相等、按包投影）与
`gutter_at` 抽取未发现问题，9 项绑定中 track-hit 与 `cause=paste-unconverged` 判为成立。全部核实后接受，并对每项补强做了变异检查（临时改生产代码、
只跑该义务的 owner、确认失败后恢复）：

- **hover / drag 只测 Source**（BLOCKER）：`emphasized()` 限定 Source 或拖动时两栏一起强调都能通过。`hovering_or_dragging_a_bar_emphasizes_only_that_bar`
  改为 Split 两栏，对每个窗格依次 hover、离开、拖动、结束拖动，并断言另一栏不强调。两个变异均被杀死。
- **标题期望值引用生产常量、只测顺次清除**（BLOCKER）：`each_state_has_its_title_and_higher_states_win` 改为五个事实的全部 32 种组合，期望标题在测试中
  逐字写出（退出已取消的字面量即 plan 09 原文）。"取消标题要求 dirty" 与 "字面量漂移" 两个变异均被杀死；唯一原生写入者守卫保留为第二个 owner。
- **Source 两端只用短行**（BLOCKER）：plan 09 要求覆盖单行折行。`scrollbar_handles_a_single_wrapped_line_and_resized_short_notes` 补首尾断言（末行与首行
  完整位于视口内、`fraction` 为 1、回到开头 `line` 与 `vertical` 为 0）并加入为第二个 owner。变异"折行行高按一行计"被该测试杀死，而只看短行的原 owner
  不会失败——这正是需要两个 owner 的原因。
- **emphasis 绘制只比像素数**（SHOULD_FIX）：改为比较着色像素的列与行范围：强调后两侧都更宽、行范围不变、且仍在轨道内。变异"强调时等宽增高"被杀死。
- **拖动映射只测原位与越界**（SHOULD_FIX）：补轨道内部位置：两种抓取偏移、四个非整齐位移下，fraction 等于"滑块顶端跟随指针位移"除以可移动长度。
  变异"fraction 量化为 0.2 步长"被杀死。
- **载入外部只测有冲突的路径**（SHOULD_FIX）：测试对无冲突（`ReloadClean`）与有冲突两种状态都验证解除。变异"仅在存在冲突时解除"被杀死（首次运行时
  rustc 因会话中断崩溃，单独重跑后确认）。
- **gap 文案与现有断言矛盾**（SHOULD_FIX）：例如绘制测试其实断言了避开 resize 边与同步按钮、默认 96 有 config 断言、非数字字符被拒有断言，"任何其它事件都会
  提交"过强。所有剩余缺口按义务（或同族义务）重写：先写已有的单元断言，再写缺失部分；`UNIT_SCROLL_RUNTIME` 拆为视图与栏、窄槽运行时、拖动运行时、
  只改视口、Preview 运行时五组，其余七项改为内联文案。
- **缺 owner 集合的负例**（NICE_TO_HAVE）：owner 检查器的正反例加入 `Target::Test`（合法单 / 双 owner；空、重复、未知包、空路径、含空白路径）；
  并集抽为 `owners_of` 并以合成义务测试第二个 owner 与跨义务共享 owner。
- 文档：P14-A81、phase-05 的 2026-10-17 补充与 coverage-matrix 按补强后的断言重写。
- 验证：fmt、clippy（smoke、win、render）0 警告；`cargo test -p stickymd-smoke --locked` 534 passed / 1 failed / 17 ignored，失败的是既有负载敏感的
  `qualification::manual::instance::tests::a_candidate_that_exits_at_once_is_a_startup_failure`（本批未改），单独连跑 3 次均通过。
- 第 1 轮修复后 `all --ci --ci-shard=tests --json` 首次运行因机器提交内存耗尽（rustc os error 1455，另一项目的构建与 Lean / WSL 同时运行）在编译阶段
  失败，属环境故障；以 `CARGO_BUILD_JOBS=4` 重跑全部 PASSED（workspace tests 383.9 s），owner 任务 12.2 s、列出 13 个 owner（含两个
  `stickymd-render/lib`）；Linux 复跑（97 个文件）clippy 退出 0，smoke 440 passed / 3 ignored、19 passed，render 165 passed / 13 ignored。

#### C2b-3 代码审查第 2 轮（2026-10-17）

gpt-6-astra（effort max）：DISAGREE，1 BLOCKER、1 SHOULD_FIX；第 1 轮的标题、Source 两端、绘制、拖动映射、载入外部、gap 文案与 owner 负例判为已关闭，
emphasis-paint、drag-mapping、Source 两端、`cause=paste-unconverged`、external-load 与 title-priority 六项绑定判为成立（只认单元接缝，调用接线另计）。
两项核实后接受：

- **hover 与拖动同时成立未测**（BLOCKER）：测试先清除 hover 再拖动，`emphasized()` 改成 `hovered.or(drag)` 或 `drag.or(hovered)` 都能通过，而拖动
  一栏时 hover 另一栏是 `pointer_moved` 的合法状态。测试在每栏拖动中再 hover 另一栏，断言两栏都强调，结束拖动后只剩被 hover 的一栏。两个变异均被杀死。
- **轨道命中没有纵向越界的反例**（SHOULD_FIX）：唯一反例在正文区，`Scrollbar::contains` 只查横向也能通过。测试对 Split 两栏轨道上下左右紧邻的外侧各按一次，
  断言不命中且不开始拖动。"忽略高度"与"左右放宽两像素"两个变异均被杀死。
- 文档：P14-A81 与 phase-05 补充写入这两项断言。
- 验证：fmt、clippy（stickymd-win）0 警告；`modules run windows,render --mode=tests` PASS（stickymd-win 288 passed、render 168 passed，owner 任务通过）。
  本轮只改 stickymd-win 的测试与文档，owner 集合不变。

#### C2b-3 代码审查第 3 轮（2026-10-17）

gpt-6-astra（effort max）：DISAGREE，1 BLOCKER、1 SHOULD_FIX；第 2 轮两项判为已关闭，六项绑定仍判为成立。两项都是相邻边界，核实后接受；为不再逐例补，
改为在小状态空间上穷举：

- **同一栏同时 hover 与拖动未测**（BLOCKER）：互斥（XOR）的 `emphasized()` 能通过，而"在窄槽内按下滑块后沿原窄槽拖动"正是该状态。测试保留原有转换序列，
  另穷举 hover（无 / Source / Preview）× 拖动（无 / Source / Preview）九种状态，断言每栏强调当且仅当它被 hover 或被拖动。XOR 与 `Option::or` 变异均被杀死。
- **轨道左 / 上边界未作为命中测试**（SHOULD_FIX）：轨道是半开矩形 `[left, right) x [top, bottom)`。测试对两栏每条边各按内侧末像素（须翻向对应方向）与
  外侧首像素（须不命中），并先断言左右边的按点在滑块之上。严格左界、严格上界、含右界、含下界、右界少一像素五个变异均被杀死。
- **主动补齐同类边界**：hover 的第二个 owner `the_gutter_is_the_band_right_of_each_pane` 原只测部分边界（上 / 下边内侧未作为命中）。改为对两个窗格、两个 DPI
  的四条边各测内侧与外侧半像素；严格上界、含下界、严格左界、不按 DPI 缩放、忽略高度五个变异均被杀死。
- 文档：P14-A81 与 phase-05 补充改写为穷举后的断言。
- 验证：fmt、clippy（stickymd-win）0 警告；`modules run windows --mode=tests` PASS（stickymd-win 288 passed，owner 任务通过）；owner 集合不变。

#### C2b-3 代码审查第 4 轮（2026-10-17）

gpt-6-astra（effort max）：DISAGREE，1 BLOCKER、1 NICE_TO_HAVE；第 3 轮两项与主动补齐的 gutter 边界判为已关闭，hover 与 drag 两项绑定改判为成立，
九项中八项成立。核实后接受：

- **轨道命中没有测滑块与空白轨道的分界**（BLOCKER）：`Scrollbar::press` 中 `y < thumb_top` 误写为 `<=` 时，track owner 的 21 个按点结果全部不变，而按在滑块
  首行会变成翻页。测试对两栏补滑块上方半像素（翻页向上）、`thumb_top`（开始拖动）、滑块末行内侧半像素（开始拖动）与 `thumb_top + thumb_height`（翻页向下），
  每次按下后结束拖动以免串扰。"首行算轨道""末端算滑块""滑块短一像素""滑块晚一像素开始"四个变异均被杀死。
- **报告措辞**（NICE_TO_HAVE）：第 3 轮记录中 gutter 的"四条边各测内侧与外侧半像素"不准确。更正：每条边取相距 0.5 px 的一内一外两点，其中一点就在边界上——
  左、上边的边界点是命中点，右、下边的边界点是不命中点。测试取点不变。
- 文档：P14-A81 与 phase-05 补充写入滑块分界。
- 验证：fmt、clippy（stickymd-win）0 警告；`modules run windows --mode=tests` PASS（stickymd-win 288 passed，owner 任务通过）；owner 集合不变。

#### C2b-3 代码审查第 5 轮（2026-10-18）

gpt-6-astra（effort max）：DISAGREE，1 BLOCKER；第 4 轮两项判为已关闭，九项中八项成立。核实后接受：

- **滑块非整数末端的最后整数行未取样**（BLOCKER）：Preview 滑块末端约为 438.396，"末端内侧半像素"取到 437.896，而整数行 438 仍属滑块；末端判定改为向下取整时
  owner 的 29 个按点结果全部不变。逐点补样已连续三轮留下新的间隙，故改为扫描：对 Split 两栏和四条滑块起止落在像素之间或之上的栏（fraction 0、0.3137、0.6421、1），
  沿轨道中线自轨道上方两像素到下方两像素、并在三个高度上横跨轨道左右各两像素，每四分之一像素按下一次，期望由半开区间 `[left, right) x [top, bottom)` 与
  `[thumb_top, thumb_end)` 独立给出，并断言至少一条栏的滑块起点和终点不在整数上。末端向下 / 向上取整、起点向下 / 向上取整、指针纵坐标取整、命中判定取整与
  "首行算轨道"七个变异均被杀死（其中一次因机器提交内存耗尽而未编译，以 `CARGO_BUILD_JOBS=2` 重跑后确认）。
- 文档：P14-A81 与 phase-05 补充写入扫描。
- 验证：fmt、clippy（stickymd-win）0 警告；`modules run windows --mode=tests` PASS（stickymd-win 288 passed，owner 任务通过）；owner 集合不变。

#### C2b-3 代码审查第 6 轮（2026-10-18）

gpt-6-astra（effort max）：DISAGREE，1 BLOCKER；第 5 轮的扫描判为已关闭，九项中八项成立。核实后接受：

- **末端比较的浮点改写未被排除**（BLOCKER）：把 `y >= thumb_top + thumb_height` 改写为代数等价的 `y - thumb_top >= thumb_height`，两者在部分几何下相差一个
  ULP。审查者给出合法输入 `preview_scroll(…, 780.0, 4.0)`：滑块末端恰为 567.0 而起点与高度都不是整数，改写后整数行 567 会开始拖动而不是翻页；网格扫描的
  16 197 个按点都碰不到这种差异。修复：把该 Preview 栏加入扫描（断言末端为 567.0、起点非整数）；另在 3 个 DPI × 4 种可见比例 × 41 个位置上，按滑块起点与
  末端本身及其前一个浮点值（`next_down`）各按一次，期望由存储的 `[thumb_top, thumb_top + thumb_height)` 给出——舍入方式不同的分类会在其中一部分几何上
  分歧。"末端向下取整"变异被杀死；末端改写变异的结果见下方验证条目。
- **探针发现的实现细节**：滑块起点或末端与轨道端点重合时，轨道端点旁的浮点值会被 `Scrollbar::contains` 按 `f32` 读成轨道内（`press` 按 `f64` 分类），
  例如 y = 41.99999999999999 被当作轨道顶 42 内的点而向上翻页。指针坐标是整像素（`f32` 精确），所以没有用户可见的影响，也不改产品代码；探针在这两种
  重合处跳过，那里的轨道边缘由整像素与四分之一像素扫描覆盖。
- **等价变异**：把起点比较改写为相对轨道顶（`y - track.y < thumb_top - track.y`）未被杀死。轨道顶是不超过 2¹⁰ 的整数，减去它在这些量级上不引入舍入
  （结果所需位数不超过原值），两种写法对所有可达输入一致，判为等价变异，不作为缺口。
- 文档：P14-A81 与 phase-05 补充写入边界探针。
- 验证：fmt、clippy（stickymd-win）0 警告；`modules run windows --mode=tests` PASS（stickymd-win 288 passed，owner 任务通过）；owner 集合不变。期间 E: 盘
  剩余低于 10 GB 触发 Rust 磁盘闸，经 USER 同意删除了可重建的 `target/release`（3.5 GB）。

#### C2b-3 代码审查第 7 轮（2026-10-18）

gpt-6-astra（effort max，58 min）：DISAGREE，1 BLOCKER、1 SHOULD_FIX；第 6 轮的末端改写反例判为已关闭，起点相对改写判为等价变异（与本报告判断一致），
九项中八项成立。本轮按要求区分了整像素影响。两项核实后接受：

- **高 DPI 下的轨道宽度未被扫描**（BLOCKER，整像素可见）：七条扫描栏都是 100% DPI，DPI 网格只按轨道中线；把命中宽度固定为 10 像素（遗漏 DPI 换算）时
  owner 的全部按点不变，而 150% DPI 下宽 15 像素的轨道右侧 5 像素点击会失效。测试重构为：所有按点经同一个只知半开区间的 `classify` 判定，且每次按下后核对
  `end_drag` 是否恰报告开始了拖动（第 6 轮的探针此前未核对）；DPI 网格扩为 100% / 125% / 150% / 200% × 有无 Split 控件留白 × 4 种可见比例 × 41 个位置，
  每条栏以四分之一像素横扫轨道左右各两像素（经过滑块），并纵扫轨道两端与滑块两端各两像素，再加滑块边界与其前一个浮点值的探针。"固定 10 像素宽度"与此前
  全部 14 个轨道 / 滑块变异（忽略高度、四条边的严格 / 包含错误、指针取整、起止取整、末端相对改写等）共 15 个均被杀死。
- **投影文档未写排除范围**（SHOULD_FIX）：P14-A81 与 phase-05 补充改写为实际覆盖：哪些按点在哪些 DPI 上、滑块边界与轨道端点重合时跳过哪一个浮点值及原因、
  轨道端点由哪些按点承担。
- 验证：fmt、clippy（stickymd-win）0 警告；`modules run windows --mode=tests` PASS（stickymd-win 288 passed，owner 任务通过）；owner 集合不变。

#### C2b-3 代码审查第 8 轮（2026-10-18）与结论

gpt-6-astra（effort max）：**AGREE**，0 BLOCKER；第 7 轮两项判为已关闭，九项绑定在声明的单元接缝内全部成立。审查者逐 DPI 复算了杀死"固定 10 像素宽度"的
整像素按点（125% `(515,59)`、150% `(517,64)`、200% `(523,74)`），并把起点相对改写、整像素输入下的指针取整等判为限于可达输入域的等价变异；在 track 命中、翻页
方向、滑块分类与是否开始拖动的范围内没有找到仍能通过 owner 的整像素回归。另两项：

- NICE_TO_HAVE（勘误）：第 7 轮记录中"所有按点经同一个 `classify` 判定，且每次按下后核对 `end_drag`"过宽——该测试前段的 29 次按下使用直接写出的期望，其中前
  21 次不逐次核对 `end_drag`；统一判定与逐次核对只适用于尾部扫描与边界探针（代码注释 "From here" 的范围）。审查者统计尾部与探针共 211 644 次按下。
- NEEDS_VERIFICATION：当前工作树的 Windows 执行——见下方最终验证。

C2b-3 最终结果：42 项 C2a 单元缺口中 9 项由 13 个单元测试 owner 持有（其中 hover、Source 两端与标题优先级各两个 owner），33 项保留为按义务写明已有断言与
缺失部分的 gap；产品代码唯一改动是保持原判定的 `gutter_at` 抽取；`Scrollbar::contains` 按 `f32`、`press` 按 `f64` 读取指针的差异对整像素输入无影响，记录
不改。

最终验证（提交前，同一工作树，未冻结的 dirty 树，仅作开发证据）：`cargo fmt --all -- --check` 通过；clippy（stickymd-smoke、stickymd-win、stickymd-render）
0 警告；`all --ci --ci-shard=tests --json`（`CARGO_BUILD_JOBS=2`，机器内存紧张）全部 PASSED，workspace tests 266.2 s，owner 任务 13.7 s、列出 13 个
owner；Linux 复跑（WSL Debian，3795335 上叠加 97 个文件）clippy 退出 0，smoke 440 passed / 3 ignored、19 passed，render 165 passed / 13 ignored。
报告中各轮的变异检查为临时改生产代码、只跑对应 owner、确认失败后恢复（脚本不进仓库），每次恢复后 `git diff --stat` 与改前一致。

## 覆盖补全 C2c / C2d 准备（2026-10-18）

### 第二实例唤醒收起 dock 的产品缺陷

为 C2d 的 G4-05（dock 收起状态下第二次启动）取证时读代码发现：窗口在 dock 边缘处于稳定收起状态（收起按钮、Esc 或 700 ms 失焦自动收起）时，
同目录第二次启动只聚焦窗口——`show()` 对非隐藏状态只发出 `FocusWindow`，而获得焦点只展开仍在向收起动画中的窗口，于是窗口停留为获得焦点但拒绝编辑
输入的 3 DIP 条。plan 09 `#single-instance` 要求"通知第一实例显示并激活"，并把第二实例列为四条恢复路径之一；没有规则让手动收起阻止唤醒，因此判为
实现偏离而非 plan 问题，不改 plan。

- 复现：新单元测试 `flow::window::reducer::phase8_window_tests::a_second_instance_expands_a_collapsed_dock`（Left / Right / Top，手动收起与
  初始收起）在修复前失败（`left: Presented(DockedCollapsed(Left))`）。
- 修复：`show(now_ms, …)` 在非隐藏分支中，若稳定目标为 `DockedCollapsed` 则 `start_expand(now_ms, false, …)` 后再聚焦；`ShowRequested` 与托盘切换
  传入 `now_ms`；`start_expand` 改为 `pub(super)` 供 lifecycle 使用。隐藏到托盘后的显示路径不变。
- 验证：fmt、clippy（stickymd-win）0 警告；`cargo test -p stickymd-win --locked` 289 passed / 8 ignored（含 `flow::window` 38 passed）。
- 投影：phase-08 增补 2026-10-18 节。真实的第二次启动（P08-M08 `identity-wake state=docked`）仍归 G4-05 exact。
- 审查：gpt-6-astra（effort max）AGREE，逐状态追踪了 `ShowRequested` 与托盘切换（稳定收起仍属可见窗口，托盘切换仍为隐藏）、hover 展开、Pin ON 与
  退出中的行为。NICE_TO_HAVE：三个用例实际都是手动收起（初始收起也置 `manually_hidden`），且未断言 frame、临时置顶与 140 ms。测试改为三条边 × 三种
  收起方式（700 ms 失焦自动收起、聚焦时手动收起、初始收起），断言唤醒后临时置顶结束、展开恰从唤醒起 140 ms、frame 为 dock 的展开 frame、无剩余期限，
  且唤醒请求的焦点使其保持展开。"不展开""从 0 时刻展开""只恢复手动收起""只改 visibility 不移 frame""保留临时置顶"五个变异均被杀死。
  NEEDS_VERIFICATION（运行证据）：补强后在本机 Windows（HEAD 2e7e7cb 上的同一工作树）重跑——fmt 通过，`cargo clippy -p stickymd-win --all-targets --locked
  -- -D warnings` 退出 0，`cargo test -p stickymd-win --locked` 289 passed / 8 ignored（`flow::window` 38 passed / 1 ignored）；修复前的失败输出见上方复现条目。
  `flow` 模块只在 Windows 编译，Linux 复跑不覆盖它。

### C2c / C2d 实施计划（2026-10-18，待 USER 决定 D1–D4）

依据：覆盖补全批设计（2026-10-15，11 轮 AGREE 与 USER Resolution）中的"输入注入"协议与 exact / automated 扩展清单；本节只补实施顺序、取证方式与
设计未定的机制。现存 77 项缺口按可关闭的层分组：G4 真实输入扩展约 37 项、G3-03 强杀点与恢复待处理 Esc 4 项、P08-A11 双副本 3 项、滚动条运行时
接线 23 项、可由补强单元测试关闭 10 项。

**C2c 输入注入模块**（不改产品；改 `tools/stickymd-smoke`）
1. plan 11 先行：在 `#phase-verification-harness` 写入注入合同——注入只经一个模块；该模块只在 `cfg(panic = "unwind")` 中真正注入，abort 构建只有
   返回错误的桩；先登记后注入，正常路径显式有界释放、`Drop` 兜底；清理以 `SendInput` 插入数判定（短计数即失败，完整计数只证明进入输入流），保留
   鼠标左键释放采样（采到 Down 为失败）；清理失败触发共享停止，之后注入输入的 G3/G4 case 不执行、不产生 PASS；业务断言与清理分开判定；注入前目标
   HWND 须为前台、激活且有焦点。
2. 实现：键盘改用 `SendInput`（`INPUT_KEYBOARD`，替换 `keybd_event`），鼠标沿用 `SendInput`；helper 的右击与两处 Esc 迁回 Rust（helper 只做 UIA
   定位与菜单核验）；治理文本扫描禁止 `SendInput` / `keybd_event` / `mouse_event` 出现在模块之外（含 helper）。托盘菜单核验从关闭到打开的变化。
3. 取证（无需独占桌面的部分先做）：短计数、释放失败的共享停止、普通 unwind 由注入调用可替换的单元测试覆盖；真实 abort 二进制到达拒绝桩且零注入、
   托盘关闭到打开需在独占桌面上运行。
4. 影响：`window_control*` 属于全部 harness 的共享输入，本批使 G3 / G4 / G5、Runtime、Performance、Resources 的模块证据全部失效一次。

**C2d exact 扩展**（按 2026-10-15 清单；每项先在 `exact_checks.rs` 声明标签）
- G4-06：Right / Top dock 组字不收起，96 / 100 透明度下提交（先 40 再 96 再 100，避免默认值空通过），编辑区与 Search 中组字时 Esc 只取消 preedit
  （以下一次输入落点、标题与 dock 几何判定）。
- G4-02：真实键盘 Esc 各分支（弹出层关闭并回退、Search 关闭、Preview 有 / 无选区、Docked / Floating 收起、Shift+Esc、重复 key-down 逐次独立）、
  物理点击收起按钮、透明度三种提交与滑块拖动中 Esc、被覆盖的感应条经真实鼠标悬停展开（自建覆盖窗口，不加消息补强）。
- G4-01：托盘"隐藏"、退出保存失败（只读并回读、键入、确认未落盘、托盘退出、保持运行、标题为固定字面值、所有路径恢复属性）、置顶在托盘与窗口间
  双向同步且不受临时 sensor topmost 影响、真实 Alt+F4（见 D4）。
- G4-05：dock 收起状态下第二次启动——上一节的产品缺陷修复后才可能 PASS。
- G3-03 与 P08-A11：见 D1、D2。
- 滚动条：见 D3。

**待 USER 决定**
- D1 G3-03 的确定性强杀点。产品当前没有可在保存中途停住的钩子，且 G3 已批准的边界是"产品 runtime 不变"。
  (a) 推荐，只改 harness：资产写入与移入 trash 以文件状态为里程碑（最终 `images/stickymd-*` 出现、`.trash/` 出现条目）后立即强杀；note 保存以只读
  note 使发布失败（产品测试已证明只读会使保存失败），在临时文件写出而发布失败后强杀，重启后按恢复规则判定，并加"编辑后未触发保存即强杀"的负对照；
  恢复待处理 Esc 以显式设置并回读 `tmp.mtime > note.mtime` 的 fixture 构造，配正向对照。它证明的是"发布失败后的崩溃"，不是系统调用中途的崩溃。
  (b) 在 Release 产品中加运行时开启的测试里程碑 / 暂停点：能在发布 API 前后精确停住，但改产品 runtime 与 plan，需要新的 USER 决定。
- D2 P08-A11 的"退出一份不影响另一份"。两份副本的托盘图标同名，现有 helper 取第一个命中且只按 PID 过滤菜单。
  (a) 推荐：helper 列出全部 StickyMD 托盘图标，逐个打开菜单并只接受菜单属于目标 PID 的那个，其余以 Esc 关闭；有界、无匹配即失败。
  (b) 只证明 note / config 双向不串写，退出隔离留给 M45 人工。
- D3 滚动条运行时接线 23 项（视图到栏、窄槽运行时、拖动应用与结束、只改视口、Preview 运行时）。
  (a) 推荐：把 `StickyApp` 中的滚动应用（视图模式 → 栏、拖动 / 翻页 → 视口、结束拖动的各触发、回执应用）再抽一层纯函数并单元持有决策，真实接线只由
  G4 持有其中可观察的几项（拖出窗口完成拖动、松开 / 失焦结束拖动），以点击视口固定点后读取光标所在行作为滚动位置的观察。
  (b) 全部走 G4 exact，需要为滚动位置建立截图或剪贴板观察，用时与脆弱性都更高。
- D4 真实 Alt+F4。若注入前的焦点核验与注入之间焦点被抢，它会关闭当时的前台窗口（例如运行 harness 的终端）。
  (a) 推荐：只在独占桌面的 G4 中注入，注入前立即核验目标为前台 / 激活 / 有焦点，失败即不注入；接受该残余竞态。
  (b) 不注入真实 Alt+F4，该义务改为人工（M05 或 M46）。

**顺序**：先提交第二实例唤醒修复；C2c plan 与模块（可无桌面开发与单元取证）；C2d 各 case 扩展；最后一次独占桌面运行同时完成 C2c 的桌面取证与 C2d 的
G3 / G4 资格化（运行前向 USER 确认桌面可独占）。10 项可单元关闭的缺口另列 C2b-4，可与 C2c 并行。

### C2c / C2d 决定（2026-10-19）

USER 选择：D2 helper 逐个试托盘图标；D3 纯函数 + 少量 G4；D4 独占桌面下注入真实 Alt+F4。D1 USER 问"长远来看哪种更好"，回答如下，按 (a) 实施：

- 被 `TerminateProcess` 结束的进程在磁盘上留下的正是它最后一次完成的系统调用之后的状态。重启后要证明的性质——从每个中间磁盘状态都能正确协调——无论这个
  状态是由产品钩子在精确时刻强杀得到，还是由 harness 构造得到，都是同一个。产品钩子不提供额外证据。
- 产品钩子却要把运行时开关、暂停点及其自身的防护与测试永久留在发布的 EXE 中。这违背 plan 11 `#qualification-process-isolation` 的"验证工具不进入产品
  runtime 与 ZIP"，也违背 G3 已批准的"产品 runtime 不变"。
- (a) 的 note 保存点是忠实的：`persist_note` 在发布失败时返回错误而不删除临时文件，所以只读 note 使发布失败后留下的是"已写出并 flush 的完整临时文件 +
  旧 note"，与"发布前一刻崩溃"的磁盘状态相同。资产写入与移入 trash 以文件出现为里程碑后强杀，是"在该点或之后"的确定性点。
- 两者都不覆盖操作系统崩溃或掉电，那依赖 `FlushFileBuffers` 与 `ReplaceFileW` 的系统保证。若将来需要逐阶段的磁盘状态，可用已有的进程内观察接缝
  （`persist_note_with_before_publish`、`atomic_publish_with_observer`）在单元层构造每个阶段并跑启动协调，同样不改产品 runtime；本批不做。

## 覆盖补全 C2c 实施记录（2026-10-19，输入注入模块）

按 2026-10-15 设计的"输入注入"协议与本报告 C2c / C2d 实施计划。plan 11 先行：新增 `#physical-input-injection` 一节（只经一个模块以 `SendInput`
注入、`cfg(panic = "unwind")` 才注入、先登记后注入与逆序显式有界释放、插入数判定、左键释放采样、共享停止、业务断言与清理分别判定、注入前核验
目标、托盘菜单核验从关闭到打开、自动化与独占桌面取证要求）。

- **核心** `tools/stickymd-smoke/src/input_injection.rs`：`Injection<B: Backend>` 在一个互斥量中持有后端、已登记的按下与停止原因。`press`
  先登记再发送按下，按下短计数时照常释放并报错（不停止）；`release` 发送抬起，左键另在 500 ms 内采样，短计数或仍为按下即设置共享停止并报错；
  `chord` 按序按下修饰键与键、再逆序全部释放；左键以 `LeftButton` 守卫持有，`release()` 显式释放、`Drop` 兜底（失败只能经共享停止报告）。停止后
  一切注入（含移动、滚轮与左键恢复释放）都返回带停止原因的错误。`live()` 只在 `cfg(all(windows, panic = "unwind"))` 中给出进程唯一的注入器；
  其余构建的后端是不可构造的 `Refused`，`SendInput` 后端（`input_injection/win32.rs`）根本不编译。
- **调用方**：键盘的四个发送函数（单键、Ctrl、Shift、Ctrl+Shift 组合）改为 `send_chord`，统一先核验目标窗口为前台、激活且有焦点（原 Ctrl+Shift 组合
  只激活不核验）再经模块注入；鼠标的左键、移动与滚轮经模块注入；拖动与点击由 `drop(button)` 改为 `button.release()?`，正常路径的释放失败不再被
  `Drop` 吞掉。
- **托盘**：helper 去掉 `mouse_event` 与 `keybd_event`，动作改为 `tray-target`（先报告菜单是否已打开，未打开才定位图标并把光标移到其上）、
  `tray-menu-items`、`tray-menu-closed` 与 `tray-invoke`；`qualification/exact_desktop/tray.rs` 负责编排：要求菜单先关闭，核验光标仍在图标上且其下
  根窗口为 Explorer 托盘后经模块右击，读到菜单打开才继续，最多两次；Esc 只在前台窗口属于 StickyMD 进程或为托盘时注入。G4-01 的菜单、显示与退出，
  以及 G3-04 / G3-05 的托盘退出改用它。
- **runner**：`ExactGroup` 增加 `injects_input`（G3、G4 为真，G5 为假）。注入停止后的 G3/G4 case 记为 FAILED"not run"；case 运行期间发生的释放失败
  使该 case FAILED，细节同时列出断言结果与清理失败。
- **治理**：`governance/input_injection.rs` 扫描 `tools/stickymd-smoke/src` 的 Rust 源码与 `tools/` 下全部 PowerShell 脚本（含注释），三种注入 API
  只允许出现在 `input_injection` 模块；helper 标记改为要求托盘读取标记，Rust 驱动须保留 `TRAY_MENU_OPEN_ATTEMPTS = 2`、菜单已打开的处理、右击与
  Esc 的核验入口。
- **未做（需独占桌面）**：实际 abort 构建到达拒绝桩且零注入、托盘菜单从关闭到打开、以及迁移后的全部 G3/G4 case，随 C2d 的独占桌面运行取证。
- **静态取证（无需桌面）**：解析 PE 导入表——`cargo build -p stickymd-smoke --release`（工作区 release profile 为 `panic = "abort"`）得到的二进制
  不导入 `SendInput`、`GetAsyncKeyState`、`keybd_event` 与 `mouse_event`；debug（unwind）二进制只导入 `SendInput` 与 `GetAsyncKeyState`，已不再导入
  `keybd_event`。release 二进制中出现的 `SendInput` / `keybd_event` 字符串是治理扫描的待查名单（拆分的 `concat` 被优化器折叠为常量），不是导入。
  （第 1 轮审查指出此结论过宽：当时光标放置与合成指针消息仍绕过模块，见下方第 1 轮记录的更正。）
- **验证**（审查前）：fmt 通过；`cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings` 0 警告；`cargo check -p stickymd-smoke --release`
  0 警告；`cargo test -p stickymd-smoke --locked` 551 passed / 17 ignored 与 26 passed；`phase 00` PASS；Linux 复跑（3795335 上叠加 108 个文件，含新文件）
  clippy 退出 0、测试 452 passed / 3 ignored 与 19 passed（Linux 测试构建编译注入核心但不编译 `SendInput` 后端）。
- 期间发现：上一提交（第二实例唤醒修复）只跑了 stickymd-win 测试，phase-08 增补使覆盖来源的整文摘要变化，PR CI 的 Linux 与 Windows 测试失败；
  核对 62 个矩阵行与全部条款摘要不变后单独提交摘要更新（`23d548d`）。今后提交前跑受影响的全部包测试。

#### C2c 代码审查第 1 轮（2026-10-19）

gpt-6-astra（effort max）：DISAGREE，3 BLOCKER、6 SHOULD_FIX、1 NEEDS_VERIFICATION、1 NICE_TO_HAVE；INPUT 布局、键 / 鼠标标志、cfg 划分、五处迁移与
runner / 治理入口判为正确。逐项核实后处理：

- **B1 unwind 与锁毒化**（接受）：后端在按下中途 panic 会跳过释放；释放中 panic 时登记已删而停止未设；毒化的锁照常放行。改为：按下登记到发起它的操作；
  每个操作持有一个 unwind 守卫，unwind 时释放该操作的按下并设置共享停止；释放的登记保留到判定之后，unwind 仍能找到；锁毒化即视为停止。单元测试覆盖
  后端在按下、释放与采样时 panic 三种情形，以及毒化后的全面拒绝。
- **B2 绕过模块的输入路径**（接受）：Rust 的光标放置、环境探针的写回与 helper 的托盘光标移动都直接调用 `SetCursorPos`，合成指针消息不受停止约束。
  改为：后端增加光标放置，三处都经模块（helper 只报告图标坐标，由 Rust 放置并读回核验）；`SetCursorPos` 加入治理禁止名单；合成指针消息
  （`send_window_message`）先检查共享停止，plan 11 写明它们只送达目标窗口、不改变桌面输入状态、不是物理输入也不证明物理投递。重新解析导入表：
  release（abort）二进制只剩 `PostMessageW`（`WM_CLOSE` 与输入法切换请求），不再导入 `SetCursorPos` 与 `SendMessageW`（合成消息的停止检查在 abort
  构建中恒拒绝，调用被消除）；debug 二进制导入 `SendInput`、`SetCursorPos`、`GetAsyncKeyState`、`SendMessageW` 与 `PostMessageW`。
- **B3 左键采样**（澄清合同而非改实现）：审查者按 plan 11 新文"采到按下即失败"指出 `[Down, Up]` 被判成功。2026-10-15 设计要求的是"保留现有鼠标左键
  释放采样"，而现有采样（迁移前的 `wait_for_left_button_state`）是在 500 ms 内等待读到抬起、到时限仍为按下才失败——释放后短暂仍读到按下是异步输入
  处理的正常现象，按"首次读到按下即失败"会制造误判。plan 11 改为准确描述现有采样："有界时间内等待读到相应状态，到时限仍未读到即失败；读到未按下也
  不证明目标收到释放（无法读取状态时同样读到未按下）"；单元测试覆盖"释放后读到三次按下再抬起"为成功、"到时限仍按下"为停止。
- **S1 跨操作释放**（接受）：chord 的清理会释放别的操作按住的左键。改为每个操作只释放自己的登记；左键守卫按自己的操作身份释放、不重复；停止后仍可释放
  已登记的按下，新注入被拒。测试覆盖"按住左键时 tap 与 chord 不松开它"和"停止后守卫仍释放"。
- **S2 托盘失败路径**（接受）：右击后读取菜单失败直接返回、重试前忽略关闭失败、诊断缺失。改为每次尝试记录图标几何、读回的光标与清理结果；右击后的任一
  失败先尝试清理再返回；清理未确认关闭即不再点击。
- **S3 观察失败被当作关闭**（接受）：helper 的菜单枚举区分"观察到为空"与"无法观察"（报告 `UIA_TRAY_MENU_UNOBSERVABLE`），关闭只认观察到的空；
  Rust 严格解析 `tray-target`、`tray-menu-items`、`tray-menu-closed` 与 `tray-invoke` 的报告，拒绝缺失、重复与矛盾。
- **S4 两份副本的图标选择**（不在本批）：exact 资格化要求唯一的 StickyMD 进程，第一个可用图标即被测图标；D2 的逐个试图标属于 C2d 的 P08-A11 扩展。
  `tray.rs` 文档写明这一点，不声称已满足 D2。
- **S5 托盘 Esc 的目标核验**（接受）：改为只在前台线程处于弹出菜单模式（`GUITHREADINFO.flags` 的 `GUI_POPUPMENUMODE`）且菜单所有者属于被测进程时注入，
  不再以前台类名为 Explorer 托盘为条件。这依赖托盘菜单由 StickyMD 的隐藏窗口经 `TrackPopupMenu` 拥有，需桌面运行确认。
- **S6 P14-A82 状态过宽**（接受）：拆为 P14-A82（仓库内可重复的单元与治理检查，AUTOMATED PASS）与 P14-A83（桌面取证：abort 构建零注入、托盘关闭到打开、
  迁移后的 G3/G4，NOT TESTED）。
- **N2**：治理模块注释写明它是对所扫文件的词法回归检查，不跟踪构建脚本、其他 crate 或运行时拼接的名称。
- **N1**：桌面取证仍待独占桌面运行（P14-A83）。
- 验证：fmt、clippy（smoke）0 警告；`cargo test -p stickymd-smoke --locked` 554 passed / 17 ignored 与 26 passed；`phase 00` PASS；release 构建 0 警告。

#### C2c 代码审查第 2 轮（2026-10-19）

gpt-6-astra（effort max）：DISAGREE，2 BLOCKER、5 SHOULD_FIX、1 NEEDS_VERIFICATION、1 NICE_TO_HAVE；第 1 轮的 B2、S5（静态）、S6、N2 判为已修复，
S4（D2）延期到 C2d 可接受。逐项核实后处理：

- **R2-B1 左键释放的 panic 路径**（接受）：显式释放先置 `released` 再释放，释放 panic 时登记残留且无人兜底；外层 case unwind 时左键守卫的 `Drop`
  若再次 panic 会逃出析构而使进程 abort，`Guarded` 也在可能 panic 的清理之后才停止。改为：左键的显式释放与非 unwind 的 `Drop` 都在该操作的
  `Guarded` 下进行，释放 panic 时由守卫停止注入并重试一次；unwind 中的清理（`Guarded` 与左键守卫的 `Drop`）先停止，每个释放用 `catch_unwind`
  隔离、各尝试一次，再次 panic 即注销该登记并继续释放其余按下，不让 panic 逃出析构。已在本工具链（1.97.1，MSVC）实测 unwind 中析构内
  `catch_unwind` 能接住第二次 panic、进程不 abort。测试覆盖：左键释放的发送与采样 panic 由守卫重试；chord 在 unwind 中释放再次 panic 时其余
  修饰键仍释放；外层 unwind 时左键释放 panic 不 abort、不留登记、停止注入；非 unwind 的 `Drop` 释放 panic 时守卫重试并停止。
- **R2-B2 采样语义**（USER 决定）：审查者指出改写 plan 不能代替对"采到按下即失败"条款修订的批准。核实属实：2026-10-15 设计的控制性文本与迁移前
  实现冲突。USER 2026-10-19 决定：等待读到抬起——有界时间内等待读到相应状态，到时限仍读到按下即失败（释放后 Windows 处理输入期间仍可能读到
  按下）。plan 11 记入该决定，`wait_left_button` 文档同步。
- **R2-S1 同一按钮的重叠持有**（接受）：`a = press_left(); b = press_left(); b.release()` 与 `a = press_left(); release_left()` 都会提前结束 A 的按下。
  改为同一键或按钮同时只由一个操作持有：已持有时拒绝再次按下与恢复性释放，原持有不受影响；plan 11 写明原因（Windows 对每个键只记一个状态）。
- **R2-S2 托盘失败出口**（接受）：每次打开尝试记录图标几何、读回的光标与结果；右击失败、菜单读取失败、未观察到菜单、`tray-invoke` 报告不合法与
  所需项缺失都先清理再返回，清理结果（含因共享停止而未执行）写入错误；任一尝试的已有观察不再丢失。
- **R2-S3 无法观察被当作无菜单**（接受）：helper 的菜单等待器到时限返回最新快照（含其可观察性），不再返回旧的空快照；`GetGUIThreadInfo` 查询失败
  传播为错误，与"查询成功但无 StickyMD 弹出菜单"分开。
- **R2-S4 严格解析**（接受）：每种 action 一个解析器，按 `=` 或 `;` 前的键与含分隔符的余部逐行精确匹配：空报告、未知行、缺失、重复、矛盾、
  空几何、非大写四位十六进制的项名与重复项名一律拒绝；`tray-menu-closed` 只认唯一的 `UIA_TRAY_MENU_CLOSED`，`close` 与 cleanup 复用同一解析器；
  `tray-invoke` 只认唯一且等于所请求项的 `UIA_TRAY_INVOKED`。反例测试覆盖审查列出的各情形。
- **R2-S5 激活点击的核验点**（接受）：物理点击前以读回的光标位置为落点，核验该点下的根窗口；托盘图标另要求读回位置与目标点一致。plan 11 同步。
- **R2-N2 采样时限**（接受）：恢复单调时钟 deadline，同时保留采样次数上限，任一耗尽即失败。
- **R2-N1**：桌面取证仍待独占桌面运行（P14-A83）；D2 双副本与 D4 真实 Alt+F4 仍属 C2d。

#### C2c 代码审查第 3 轮（2026-10-19）

gpt-6-astra（effort max）：DISAGREE，1 BLOCKER、4 SHOULD_FIX、2 NEEDS_VERIFICATION、1 NICE_TO_HAVE；R2-B2（按 USER 决定）、R2-S1、R2-S3、R2-S4、
R2-S5 判为已修复。逐项核实后处理：

- **R3-B1 左键守卫在外层 unwind 时未先停止**（接受，并更正第 2 轮记录）：第 2 轮记录称两个 `Drop` 路径都先停止，实际左键守卫在 case unwind 时只做隔离
  释放、不停止，现有测试还断言了不停止。case 持有左键时 unwind，其结果未知，按保守原则改为与操作 unwind 相同：先停止（原因"a case unwound while
  it held the left button"），再逐项隔离释放；测试改为要求停止且之后全部入口被拒。
- **R3-S1 被接住的 panic payload 在隔离范围外析构**（接受）：`catch_unwind` 的 `Err` 被丢弃时会析构 payload，析构若 panic 即逃出析构。改为接住后
  `mem::forget` payload（每个释放至多一次）。新增测试以析构会 panic 的 payload 触发；变异为 `drop(payload)` 时测试进程以 0xC0000409 异常终止，证实了
  审查者的静态推断。
- **R3-S2 时限后的读数被当作成功**（接受）：采样改为按后端时钟——`Backend` 增加 `now()`（真实后端为 `Instant::now`，测试后端为只由 sleep 与慢读推进的
  虚拟时钟），每次读数之后检查已用时，超过时限的读数无论读到什么都不计；sleep 不超过剩余时间；采样次数上限只防时钟不前进。新增测试：首次释放采样
  耗时 600 ms 后读到抬起为失败并停止，400 ms 为成功；卡住的按钮恰好睡满 500 ms。
- **R3-S3 托盘已有观察丢失**（接受）：光标放置与右击拆成两步，放置后先记录读回位置，右击前再读回一次并核验；打开成功时保留各次尝试，之后读取、关闭、
  缺项与 invoke 的失败都附上打开历史与清理结果；`tray-target` 失败注明"无需清理，未点击"。
- **R3-S4 菜单所有者查询失败被当作已关闭**（接受）：弹出菜单模式下所有者 HWND 为 0 或 `GetWindowThreadProcessId` 失败改为错误；cleanup 中未发送 Esc 不再
  等于已关闭，两种情形都要 helper 观察到关闭才算已知关闭。
- **R3-N3 核验与按下之间的等待**（接受）：五处左键按下统一经 `press_left_on`，在按下前立即读回光标并核验其下根窗口为目标窗口（拖动与缩放的 75 ms
  等待、激活的 25 ms 等待都在核验之前）；核验与按下之间的残余竞态写入 plan 11。
- **R3-N1**：A83 仍为 NOT TESTED。工具链行为现有执行证据：本批的隔离测试在 1.97.1 MSVC 的测试二进制中实际运行并通过（见下方验证），变异实验证明去掉
  隔离会使进程异常终止。abort 构建"到达拒绝桩"仍待桌面运行区分于更早的环境预检失败。
- **R3-N2 headless 首跑失败**：`cli_exit::diagnostic_commands_preserve_internal_success_files` 在第 2 轮修复后的首次全量运行中失败 1 次，失败详情因输出
  过滤未保留；随后单独运行 1 次、全量 headless 8 次、强制重新链接后全量 3 次均通过。该测试文件本批未改，预期错误发生在治理执行与注入之前；
  两个测试共用仅以 PID 与时间戳命名的临时目录，存在既有的碰撞可能，但未证实为根因。结论：根因未知，不以复跑通过关闭；后续全量运行保留完整输出。
- 验证（第 3 轮修复后，rustc 1.97.1）：fmt 与 `cargo clippy -p stickymd-smoke --all-targets --locked -- -D warnings` 退出 0；`cargo test -p stickymd-smoke --locked`
  单元 559 passed / 17 ignored、headless 26 passed（完整日志保留）；`phase 00` PASS；release（abort）构建退出 0，其输入相关导入只有 `PostMessageW`，debug 导入
  `GetAsyncKeyState`、`PostMessageW`、`SendInput`、`SendMessageW`、`SetCursorPos`；Linux 复跑（3795335 上叠加 110 个文件）clippy 退出 0、测试 459 passed /
  3 ignored 与 19 passed。变异检验：左键守卫 unwind 不停止、析构 payload、接受时限后的读数、sleep 越过时限四个变异均被对应测试杀死。

#### C2c 代码审查第 4 轮（2026-10-19）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、2 SHOULD_FIX、2 NEEDS_VERIFICATION；R3-B1、R3-S1、R3-S2、R3-S4、R3-N3 判为已修复。逐项核实后处理：

- **R4-S1 时限当刻读到按下后仍继续采样**（接受）：已用时恰等于时限时剩余为零，读数不符仍会 `sleep(0)` 后再读并可能成功。改为时限当刻的读数计入、
  读到不符即失败。测试：释放首次采样恰在 500 ms 读到按下即失败并停止（即使下一次读数会是抬起）；499 ms 读到按下只睡剩余 1 ms、500 ms 读到抬起为成功。
  去掉该判断的变异被测试杀死。
- **R4-S2 仍遗漏已取得的观察**（接受）：托盘光标放置把每次读回写入该次尝试的记录，后续放置或读取失败时不再丢失；helper 在菜单无法观察时附带此前已读到的
  部分菜单项（`UIA_TRAY_MENU_UNOBSERVABLE; partial_items=…`），Rust 解析器只把它们写入错误、从不当作菜单。plan 11 同步。第 3 轮记录"任一尝试的已有观察
  不再丢失"的说法当时过宽，以本条为准。
- **R3-N1 延续**：本轮验证改为留存绑定源码身份的证据（见下方验证）；A83 仍为 NOT TESTED。
- **R3-N2 延续**：根因仍未知，记录不变。
- 验证（第 4 轮修复后）：证据留存于 ignored 的 `target/review-evidence/c2c-code/verification-r4/`（`identity.txt` 记 HEAD `23d548d`、范围补丁
  sha256 `65050626…6693`（`tools/stickymd-smoke`、plan 11、phase-14、coverage-matrix；不含记录本次运行的报告）、六个新文件的 sha256、`rustc -Vv`
  1.97.1 x86_64-pc-windows-msvc；`summary.txt` 记各命令与退出码、测试二进制与 release/debug 可执行文件的 sha256、导入表，以及运行后范围补丁摘要不变）。
  结果：fmt、clippy（`-D warnings`）、`cargo test -p stickymd-smoke --locked`（单元 559 passed / 17 ignored、headless 26 passed）、`phase 00`、release 与 debug
  构建均退出 0；release（abort）输入相关导入只有 `PostMessageW`。Linux 复跑（3795335 上叠加 110 个文件，归档 sha256 `d9e9cc4e…d373`，rustc 1.97.1）clippy
  退出 0、测试 459 passed / 3 ignored 与 19 passed。

#### C2c 代码审查第 5 轮（2026-10-19）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、1 SHOULD_FIX、2 NEEDS_VERIFICATION、1 NICE_TO_HAVE；R4-S1 判为已修复；审查者独立复算了
`verification-r4` 的范围补丁、六个新文件、两个测试二进制与 release/debug 可执行文件的摘要，均与当前树一致。逐项核实后处理：

- **R5-S1 等待中较早的部分观察被覆盖**（接受）：helper 的菜单等待每轮覆盖快照，较早一次无法观察的轮询已读到的项在后续空轮询后丢失。改为等待中保留
  最近一次非空的部分项（有界，只存一份），等待结束仍由最新快照决定结果；`UIA_TRAY_MENU_UNOBSERVABLE` 与 `UIA_TRAY_MENU_NONE` 都可附带
  `; partial_items=…`，Rust 解析器只把它写入该次尝试的结果。新增 Windows 测试从 helper 源码载入其函数、以脚本化序列替换 UIA 枚举（首轮无法观察且读到
  一项，之后为空），分别断言两种结局的报告与解析；去掉保留语句的变异被杀死。测试的等待时限取 1 s，因为在并行测试负载下 PowerShell 首次调用函数的编译
  耗时可超过 150 ms（首次以 150 ms 运行时只轮询了一次）。
- **NICE 扩展名大小写**（接受）：治理的文件收集改为不区分 ASCII 大小写匹配扩展名（Windows 文件名不区分大小写），新增 `tools/probe.PS1` 被扫描的测试，
  变异被杀死。该函数也供其他治理扫描使用，只会多收集文件。
- **R3-N1、R3-N2 延续**：不变——A83 仍为 NOT TESTED（环境探针把拒绝原因折叠为布尔值，需桌面运行区分），headless 首次失败根因仍未知。
- 验证（第 5 轮修复后）：证据在 `target/review-evidence/c2c-code/verification-r5/`（范围补丁 sha256 `a87ff41e…35ee`，运行后不变）。fmt、clippy（`-D warnings`）、
  `cargo test -p stickymd-smoke --locked`（单元 561 passed / 17 ignored、headless 26 passed）、`phase 00`、release 与 debug 构建均退出 0；release（abort）输入相关
  导入只有 `PostMessageW`。Linux 复跑（110 个文件，归档 sha256 `6a3afd69…eff8`）clippy 退出 0、测试 460 passed / 3 ignored 与 19 passed（新 helper 测试只在
  Windows 编译）。

#### C2c 代码审查第 6 轮（2026-10-19）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、1 SHOULD_FIX、2 NEEDS_VERIFICATION（不阻断）、1 NICE_TO_HAVE；R5-S1 与扩展名大小写判为已修复；
审查者复算 `verification-r5` 的全部摘要，与当前树一致。逐项核实后处理：

- **R6-S1 菜单随后打开时较早的部分观察丢失**（接受）：等待中某次无法观察的轮询读到部分项、随后观察到完整菜单时，helper 只报告当前项，后续关闭或 invoke
  失败的错误无从得知较早的部分项。改为成功快照同样携带部分项，helper 在菜单项之后至多附一行 `UIA_TRAY_MENU_PARTIAL; partial_items=…`；Rust 解析器只接受它
  作为菜单项之后的唯一末行，记入该次尝试（"opened; partial_items=…"），从而进入之后各失败出口的打开历史；菜单是否打开仍只由完整当前观察决定。helper 测试
  增加"部分观察 → 完整打开"结局；去掉 helper 的附加行、成功快照不带部分项、解析器丢弃部分项三个变异均被杀死。
- **R6-N1 helper 测试依赖调度时间且无进程时限**（接受）：测试先以立即成功的序列预热一次（编译这些函数），再计数正式运行的轮询，不再与首次编译竞争；子进程经
  `managed_process::run_bounded_output` 运行，60 s 总时限。剩余风险：进程在等待中被挂起超过 1 s 仍会使"至少两次轮询"失败，这是测试的已知前提。
- **R3-N1、R3-N2 延续**：不变。
- 验证（第 6 轮修复后）：证据在 `target/review-evidence/c2c-code/verification-r6/`（范围补丁 sha256 `356baa8d…3b01`，运行后不变）。fmt、clippy（`-D warnings`）、
  `cargo test -p stickymd-smoke --locked`（单元 561 passed / 17 ignored、headless 26 passed）、`phase 00`、release 与 debug 构建均退出 0；release（abort）输入相关
  导入只有 `PostMessageW`。Linux 复跑（归档 sha256 `049776df…965c`）clippy 退出 0、测试 460 passed / 3 ignored 与 19 passed。

#### C2c 代码审查第 7 轮（2026-10-19）

gpt-6-astra（effort max）：**AGREE**，0 BLOCKER、0 SHOULD_FIX；R6-S1、R5-S1、R4-S2 与扩展名大小写判为已修复；审查者复算 `verification-r6` 的范围补丁、六个新文件
与四个可执行文件的摘要，均与当前树一致，并独立读取 PE 导入表得到相同的输入 API 集合。保留项：

- **R3-N1（NEEDS_VERIFICATION）**：A83 仍为 NOT TESTED；环境探针把拒绝原因折叠为布尔值，需在独占桌面运行中区分"到达 abort 拒绝桩"与更早的环境预检失败。
  随 C2d 的独占桌面运行取证。
- **R3-N2（NEEDS_VERIFICATION）**：headless 首次失败的根因仍未知；再次失败时保留完整断言、退出码、stdout/stderr 与 fixture 路径后定向复现。
- **R6-N1（NICE_TO_HAVE）**：helper 测试"1 s 内至少两次轮询"仍依赖调度（正式首轮被挂起超过 1 s 会假失败）；60 s 是运行期限，其后另有有界回收时间。
  可控时钟或等待接缝留作后续改进。
- **R7-N1（NICE_TO_HAVE）**：打开历史中的部分项从 `MenuRead.partial` 到 `Opened::failed` 的最后一段没有回归断言（把 `format!("opened{}", menu.partial)`
  改回 `"opened"` 不会被现有测试发现）。下一批抽出成功结果的组装步骤并补断言。
- 范围：D2 逐图标尝试与 D4 真实 Alt+F4 属 C2d；`apps/stickymd-win`、`crates/stickymd-render` 的未提交改动（C2b-4）不在本批。

## 覆盖补全 C2b-4 实施记录（2026-10-19，无需桌面的单元绑定）

范围：按 D3（纯函数 + 少量 G4）把 `StickyApp` 剩余的滚动决策抽为纯函数并以单元测试持有，补强透明度、退出取消状态与 Preview 回执的测试，再逐义务审计后绑定。
产品行为不变：抽取保持原有的早退与输入，只把计算移出 runtime。

### 落地内容

- **纯函数**（`apps/stickymd-win/src/app/scrollbar.rs`）：`view_bars(dpi, geometry, source, preview)` 给出视图显示的窗格的栏（Source 由投影的滚动度量，
  Preview 由 `PreviewFacts`：帧缓存的 document height、偏移与是否当前）；`preview_offset_at`（滑块比例到 Preview 偏移）与 `preview_offset_by`（文档内翻页）。
  `scroll_runtime.rs` 改为调用它们：`scrollbar_geometry` 在窗口拒绝编辑时仍先返回无栏，只为视图显示的窗格测量。
- **退出已取消状态的唯一入口**（`app/title.rs`）：`QuitCancelNotice::on_window_effect` 接收每个窗口效果，只有因 canonical note 保存失败而取消的退出建立该
  状态；`window_runtime.rs::apply_window_effect` 在处理每个效果前先交给它，原先只在 `QuitCancelled` 分支调用的私有入口不再被外部引用。
- **测试**：`app::scrollbar` 三项（逐视图的栏、Preview 位置与翻页、滑块几何）、`app::window_interaction` 窄槽几何、`app::preview_runtime` 只差偏移的旧回执、
  `app::controls` 透明度范围（并补强拖动测试逐步断言 config 不脏）、`app::title` 三项（只有保存失败改变状态、唯一入口的词法守卫、`finish_event_pass` 只在
  `about_to_wait` 的窗口 tick 之后调用一次的词法守卫）、`app::guards` 屏障组成（8 种组合 × 其余事实全开 / 全关）、render `source::scrollbar` 两项（折行长文的
  映射与末端边界只测量一次、Source 翻页）。render 的 `SourceProjection` 增加只在测试中编译的 `extent_measures` 计数，用于断言边界缓存被复用。

### 绑定（17 项，`coverage/table.rs`）

| 义务 | owner | 依据 |
| --- | --- | --- |
| scroll-1 per-pane（source / preview / split） | `view_bars` 测试 | 三种视图、两种 DPI 下恰好显示的窗格有栏且在自身内容矩形上；`scrollbar_geometry` 直接以 `view_geometry` 调用 `view_bars` |
| scroll-2 short-note hide-thumb | `view_bars` 测试 | Source 全可见与不高于窗格的 Preview 文档在各窗格单独都无滑块 |
| scroll-2 short-note keep-gutter | 窄槽几何 + `view_bars` 测试 | `geometry()` 在每个视图保留窄槽，且其输入只有视图模式、窗口尺寸与 DPI，与正文长度无关 |
| scroll-2 gutter-geometry | 窄槽几何测试 | 四种 DPI × 四种窗口尺寸 × 三种视图：窄槽宽恰为缩放后的 `GUTTER_DIP`，轨道在窄槽内、真实 resize 命中为空、与 Split 同步按钮不相交 |
| scroll-2 gutter-runtime content-zoom | 窄槽几何测试 | `view_geometry` 只以视图模式、窗口尺寸与窗口 DPI 调用 `geometry()`，Content Zoom 不是输入 |
| scroll-3 thumb-geometry | 滑块几何测试 | 1000 种可见比例 × 4 种 DPI × 有无 Split 留白：长度为可见比例且不低于 24 DIP × DPI、不等于整条轨道，位置线性且在轨道内 |
| scroll-3 ends-reach-document-ends（preview） | Preview 位置测试 | 比例 0 / 1 分别为文首与最后一屏 |
| scroll-3 track-page-scroll | 轨道命中 + Preview 位置 + Source 翻页测试 | 轨道按下按窗格 90% 翻页；Preview 与 Source 都按像素翻页并停在两端 |
| scroll-5 source-mapping | Source 映射测试 | 每行都折行的 2000 行长文：末端只测量一次（排版少于 40 行）并复用到几何变化；跳转只排版目标附近并读回比例 |
| scroll-5 preview-document-height | Preview 位置 + `view_bars` 测试 | 滑块比例、可见比例与偏移都由给定的 document height 计算；runtime 传入 `frame.document_height()`（缓存帧的值，不排版） |
| scroll-6 stale-receipt-identity | 旧回执测试 | 只差偏移的回执布局相同但身份不同，成功与失败都只重排、从不显示 |
| opacity-2 range-and-input | 透明度范围测试 | 四种 DPI 下滑块以步长 1 到达 40–100 每个整数；输入只收至多三位数字；提交 clamp 到范围、空输入不提交；reducer 对预览与提交再 clamp |
| opacity-3 no-stepwise-write | 拖动提交测试 | 每个拖动步骤后 config 不脏、`begin_persist` 为空，松开恰写一次 |
| quit-2 cause=quit-guard | 只有保存失败改变状态 + 唯一入口测试 | 全部窗口效果（含 `QuitBlocked`）从两种初态出发都不改变状态，除保存失败取消的退出；产品源码只有一个入口 |
| quit-2 cleared-after-event-when-barrier-false | 屏障组成 + 事件结束 + 清除测试 | 屏障恰为 dirty / 欠持久化 / 保存中之一；`finish_event_pass` 只在窗口 tick 之后调用一次；屏障为假时才清除 |

两个词法守卫（唯一入口、调用位置）只检查源码文本，不求值 `cfg` 或宏展开。

### 仍为缺口（改写后的文本见 `table.rs`）

Split 同步开关是否带动另一窗格、窄槽内按下不作为正文点击、指针移动逐次应用到视口（实时定位）、松开 / 失焦 / 切换视图 / resize 结束拖动、拖出窗口完成拖动、
滚动只改视口（六项状态）、`preview_layout_is_current` 的当前判定（过期帧无滑块、旧回执守卫）。D3 下松开 / 失焦与拖出窗口由 G4 持有；其余待新的单元接缝或 exact 层。

### 验证（2026-10-19，审查前）

- 变异检验（临时改动、恢复，apps/crates 改动的摘要前后一致）：24 个变异全部被对应 owner 测试杀死——守卫阻止的退出建立状态、屏障漏掉欠持久化或保存中、
  状态在窗口 tick 之前结算、滑块止于 99、输入接受四位、提交 clamp 到 99、拖动预览弄脏 config、滑块下限不随 DPI、滑块比例不 clamp、旧偏移回执被显示、
  窄槽不随 DPI、resize 边伸进轨道、边界缓存从不读取、跳转忽略行内偏移、Preview 栏忽略当前标志、Preview 视图出现 Source 栏、忽略可见比例、最后一屏被截短、
  翻页越过末端、reducer clamp 到 99、`view_bars` 忽略 Split 留白、Source 翻页只走一半、另一处源码提及私有入口。
- 绑定源码身份的证据在 ignored 的 `target/review-evidence/c2b4-code/verification-r0/`（HEAD `5c65b46`，范围补丁 sha256 `b17395bd…bcd7`，运行后不变）：
  `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings` 与 `stickymd-smoke all --ci --ci-shard=tests`（governance、
  Phase 1 两组、workspace tests、unit-test owner verification 五项）均退出 0。
- Linux 复跑（5c65b46 上叠加本批 15 个文件）：`clippy -p stickymd-smoke -p stickymd-render` 退出 0，测试全部通过（render 167 passed / 13 ignored 等，smoke 460 passed /
  3 ignored 与 19 passed）。`stickymd-win` 只在 Windows 构建。

#### C2b-4 代码审查第 1 轮（2026-10-19）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、6 SHOULD_FIX、1 NEEDS_VERIFICATION、1 NICE_TO_HAVE；建议 17 项绑定中 9 项退回 gap。审查者确认重构保持行为
（拒绝编辑时的早退、只为可见窗格测量、Preview 高度为缓存字段读取；退出已取消状态的更新移到效果分派之前，标题事实不读取 diagnostic，无可见差异），并复算了
`verification-r0` 的范围补丁摘要。逐项核实后处理（上文绑定表中的六项以本条为准）：

- **F1 两个词法守卫不证明调用约束**（接受）：唯一入口守卫只禁止其他文件出现私有入口名，删掉或后移 `on_window_effect` 调用、在 `QuitBlocked` 分支直接改状态都能通过；
  结算守卫只比较文本位置，"先构造 Tick、再结算、后 dispatch"或循环中调用两次都能通过，结算函数体改为 `settle(false)` 也不被三个 owner 发现。`cause=quit-guard`
  与 `cleared-after-event-when-barrier-false` 退回 gap，文本写明判定与词法守卫已测、效果处理与事件结算的运行时组合未测。两个守卫保留为回归检查，不再作为证明。
- **F2 依靠未被 owner 执行的调用关系**（接受）：几何测试直接调用 `geometry()`、Preview 测试手填高度，`view_geometry` 改用内容缩放、按短文回收槽宽或把
  `PreviewFacts` 的高度错接为视口高度都不会被发现。`keep-gutter`、`content-zoom`、`preview-document-height` 退回 gap。
- **F3 混合长短窗格只检查短的一侧**（接受，补强后保留）：每个混合情形增加"长的一侧仍有滑块"的断言；"任一窗格短就隐藏两侧"的变异被杀死。
- **F4 数值盲区**（接受，补强后保留）：Source 读回容差由一整逻辑行收紧到千分之一行，"度量忽略行内偏移"的变异被杀死；Source 向上翻页改为除到达文首的一页外每页恰为
  一页像素，Preview 从文档中部双向翻页断言精确位移，"负向翻页减半"的两个变异均被杀死。
- **F5 透明度输入的 oracle**（接受，退回 gap 并待 USER）：输入框在键入时拒绝负号与第四位数字，`-1`、`1000` 无法输入也就不会 clamp；plan 09 只写"整数输入框、越界 clamp、
  非整数不提交"。这是 HEAD 已有的产品行为，是否构成与合同的冲突需 USER 判断，本批不改产品。`range-and-input` 退回 gap，测试注释写明被拒的输入只记录当前行为。
- **F6 回执 gap 文本合并失当**（接受）：`stale-frame-no-thumb` 缺 `preview_layout_is_current` 的判定断言；`stale-receipt-guard` 缺回执入口以完整 `PreviewViewport` 相等
  （而非 `same_layout`）判定、Relayout 分支提前返回且不改写 frame 与偏移的组合断言，两项分开写明。拖动 gap 文本改为"将由 G4 持有，尚无 exact 用例"。
- **F7 变异与 Linux 证据**（接受）：变异脚本与输出保存在 ignored 的 `target/review-evidence/c2b4-code/mutations/`；Linux 复跑的原始输出随下方验证保存。
- **F8 文档**（接受）：phase-05 的定向入口补上 `app::controls`、`app::title`、`app::guards`；拖动提交测试实际断言的是"松开恰一次提交请求并标脏 config"，未执行落盘，
  上文绑定表"松开恰写一次"的措辞以此为准。

结果：绑定 11 项（逐视图的栏 × 3、短文无滑块、窄槽几何、滑块几何、Preview 两端、轨道翻页、Source 映射、只差偏移的旧回执、拖动中 config 不脏）；USER 待决：透明度输入框
是否应允许键入负数与四位以上整数（D5）。
- **USER 2026-10-20 决定（D5）**：透明度输入框放开输入、提交时 clamp。核实时另发现：输入框的按键处理对每个按键以该键文本**替换**整段内容（自 Phase 8 起），且绘制的是
  预览值而非键入文本，故真实桌面上无法键入多位数、键入时也看不到内容。D5 的实现（逐键追加、聚焦后首键替换、负号只在首位、提交按数值 clamp、绘制键入文本）改变
  产品行为，单列一批另行审查与提交；本批 `range-and-input` 保持 gap，文本写明 D5 待实现。
- 验证（第 1 轮修复后）：`target/review-evidence/c2b4-code/verification-r1/` 记录 fmt、`clippy --workspace --all-targets -D warnings` 与 `all --ci --ci-shard=tests`（五项
  均 PASSED）退出 0，以及 Linux 复跑的完整输出（`linux.log`：`clippy -p stickymd-smoke -p stickymd-render` 退出 0，render 167 passed / 13 ignored、smoke 460 passed /
  3 ignored 与 19 passed 等，`test_exit=0`）。之后只改了 D5 相关的三处文本（`changes-since-r1.txt`），`verification-r2/`（范围补丁 sha256 `9114785c…a774`，运行后
  不变）在该状态上重跑 fmt、clippy 与 CI 测试分片，均退出 0。

#### C2b-4 代码审查第 2 轮（2026-10-20）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、1 SHOULD_FIX、1 NEEDS_VERIFICATION、1 NICE_TO_HAVE；F1–F6、F8 判为已解决，F7 部分解决；保留的 11 项绑定中 10 项判为
成立，6 项退回的 gap 文本判为准确；D5 延后判为合理。逐项核实后处理：

- **R2-1 Source 映射的正反计算共用未经独立验证的折行高度**（接受）：把 `scroll_line_height` 改为一律按一个视觉行计（误把折行逻辑行当一行），长文测试的跳转与读回
  使用同一错误高度，仍全部通过——已实测为 SURVIVED。现有"单行折行与真实末行几何"测试以真实布局核对该高度，实测把同一变异杀死，故加入 `SCROLL_SOURCE_MAPPING`
  的 owner 集合（plan 11：共同证明一项义务的测试须全部列为 owner），不改产品算法。
- **R2-2 变异与 Linux 证据不完整**（接受）：全部 29 个变异（原 24 个、第 1 轮补强后的 4 个、R2-1 的 1 个）用同一个保留字节的脚本重跑，每个变异单独保存补丁、
  命令、退出码与含失败断言的完整输出（`target/review-evidence/c2b4-code/mutations/runs/`），29 个均为 KILLED，运行前后 apps/crates 的 diff 摘要相同。Linux 复跑
  改为 stdout 与 stderr 分开保存（见下方验证）。
- **R2-3 计数勘误**（接受）：第 1 轮验证记录中"只改了 D5 相关的三处文本"应为四处（`controls.rs` 注释、phase-05、P14-A81、`table.rs`），均为文本。
- 过程记录：重跑前一次变异运行被中途停止，第 01 号变异留在 `app/title.rs`；已撤销并按字节核对，apps/crates 的 diff 与 `verification-r2/scoped.patch` 完全一致后
  才重跑。
- 验证（第 2 轮修复后）：磁盘低于 Rust 构建阈值，经 USER 同意删除 `target/debug` 后从头构建。`target/review-evidence/c2b4-code/verification-r3/`（范围补丁 sha256
  `07757ff0…bf6a`，运行后不变）：fmt、`clippy --workspace --all-targets -D warnings` 与 `all --ci --ci-shard=tests`（五项均 PASSED，含 owner 核验）退出 0；Linux
  复跑（归档 sha256 `1261045a…b671`）clippy 与测试退出 0，stdout / stderr 分开保存（`linux.test.stdout.log` 含 671 条逐项 `ok` 记录与全部 `test result: ok` 摘要）。

#### C2b-4 代码审查第 3 轮（2026-10-20）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、1 SHOULD_FIX、1 NEEDS_VERIFICATION；R2-1、R2-3 判为已解决，R2-2 部分解决；10 项绑定与 6 项 gap 判为成立 / 准确，
29 份变异记录与 Linux 日志判为内部一致。逐项核实后处理：

- **R3-1 独立布局依据只在 1× 检查**（接受）：把折行高度误按 DIP 计（`line_height / scale_factor`）时，长文测试全程 1×、单行折行测试的 2× 检查只看短文，两个 owner
  都会通过。单行折行测试改为在 1× 与 2× 下各做一遍折行长行的中点、末端（末行经 `caret_rect` 读真实布局须在视口内）与文首检查；该变异被 2× 的末端检查杀死
  （`runs/31`），产品算法不变。
- **R3-2 "长文 owner 已实测 SURVIVED"缺运行记录**（接受）：第 2 轮记录所称的实测当时只在交互命令中运行、未留存。现补以同一保留字节的脚本留存：`runs/30`（折行计作一行
  对长文 owner，exit 0，SURVIVED）与 `runs/32`（DIP 变异对长文 owner，SURVIVED），说明两个 owner 缺一不可；`runs/29` 以补强后的单行折行测试重跑仍为 KILLED。
- 验证（第 3 轮修复后）：`target/review-evidence/c2b4-code/verification-r4/`（范围补丁 sha256 `32605555…25fc3`，运行后不变）：fmt、`clippy --workspace --all-targets
  -D warnings` 与 `all --ci --ci-shard=tests`（五项均 PASSED，含 owner 核验）退出 0；Linux 复跑（归档 sha256 `35aee46e…8d3f`）clippy 与测试退出 0，stdout / stderr
  分开保存。

#### C2b-4 代码审查第 4 轮（2026-10-20）

gpt-6-astra（effort max）：DISAGREE，0 BLOCKER、1 SHOULD_FIX、1 NICE_TO_HAVE；R3-1、R3-2 判为已解决，32 份变异记录判为内部一致。逐项核实后处理：

- **R4-1 可用高度的单位错误仍能通过**（接受）：把 `content_height()` 误换成 DIP 时，长文测试全程 1×，2× 折行长行的末行仍落在视口内，跳转与读回共用错误边界，两个
  owner 都通过；而 2× 下本可完全容纳的短文会被算出约 0.75 的可见比例并显示滑块。单行折行测试增加 2×（320×320）下三行短文 `viewport_fraction == 1.0` 的断言；
  该变异被它杀死（`runs/33`），产品算法不变。
- **R4-2 resync 前的长文缓存前提在改写中丢失**（接受）：循环外新建的投影未先测量就 resync，缓存键不含 generation 也能通过。恢复为先测量长文（断言未全可见）再
  resync；"缓存键忽略 generation"的变异被杀死（`runs/34`）。
- 环境：第 4 轮修复前 E: 盘再次低于构建阈值，原因是 WSL Debian 的 `ext4.vhdx` 膨胀到 207 GB（盘内实用 63 GB）；USER 以 fstrim 与 diskpart 压缩到 75.3 GB 后继续。
- 验证（第 4 轮修复后）：`target/review-evidence/c2b4-code/verification-r5/`（范围补丁 sha256 `3f8c19e8…2e91`，运行后不变）：fmt、`clippy --workspace --all-targets
  -D warnings` 与 `all --ci --ci-shard=tests`（五项均 PASSED，含 owner 核验）退出 0；Linux 复跑（归档 sha256 `7681c8e4…83e5e`）clippy 与测试退出 0。

#### C2b-4 代码审查第 5 轮（2026-10-20）

gpt-6-astra（effort max）：**AGREE**，0 BLOCKER、0 SHOULD_FIX；R4-1（三行短文 2× 用例的几何与精确相等经静态计算确认，`runs/33` 的失败值 0.7526881893475851 与之
一致）与 R4-2（同一对象先测量后 resync，`resync` 不清空缓存、依靠键不匹配重算）判为已解决；11 项绑定判为成立，6 项 gap 判为准确。保留项：

- **NEEDS_VERIFICATION（变异恢复摘要）**：各次变异运行的前后 diff 摘要只记为"unchanged"。已把四次运行前记录的 `git diff -- apps crates` 摘要补入
  `mutations/README.txt`；最后一次（`e28f401b…8bff`）与审查者对当前状态的复算值相同。此后的变异运行须在汇总中同时写出前后两值。
- 运行时接线（keep-gutter、content-zoom、preview-document-height、quit-guard、cleared-after-event）、透明度输入（D5）与真实输入 / 桌面验收仍为具名缺口。
