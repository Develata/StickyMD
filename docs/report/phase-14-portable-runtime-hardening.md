# Phase 14 Portable Windows Runtime Hardening

## Decision

v0.1.0 portable ZIP 的 `StickyMD.exe` 必须在未安装 Rust toolchain、Visual Studio 或独立
Visual C++ Redistributable 的原生 Windows 11 上运行。实现采用两层、相互独立的约束：

1. `.cargo/config.toml` 对固定目标 `x86_64-pc-windows-msvc` 启用静态 MSVC CRT；
2. std-only `stickymd-smoke` 解析 exact PE32+ 普通与 delay-load import table，实际 artifact
   仍导入 developer runtime DLL 时 fail closed。

构建配置不是自证；PE gate 才验证最终链接结果。Clean Windows 11 VM 仍是独立人工 gate，
在真实运行前保持 `P12-M34 = NOT TESTED`。

## Reproduction Before Hardening

旧 Release 的系统 PE 检查显示：

```text
VCRUNTIME140.dll
api-ms-win-crt-*.dll
Windows system DLLs
```

新增 Rust gate 对该旧 artifact 返回非零，并报告：

```text
portable executable imports developer runtime DLL(s): VCRUNTIME140.dll
```

因此旧 artifact 不满足“无需另装 Visual C++ runtime”的 portable 发行要求。

## Exact Artifact Evidence After Hardening

静态 CRT Release build：

```text
cargo build --workspace --release --locked
PASS
StickyMD.exe bytes = 8,476,160
```

Rust PE gate：

```text
PORTABLE_NATIVE_DEPENDENCIES=
advapi32.dll,api-ms-win-core-synch-l1-2-0.dll,bcryptprimitives.dll,combase.dll,
comctl32.dll,dwmapi.dll,gdi32.dll,imm32.dll,kernel32.dll,ntdll.dll,ole32.dll,
oleaut32.dll,shell32.dll,user32.dll,uxtheme.dll
DEVELOPER_RUNTIME_IMPORTS=none
```

Microsoft `dumpbin /dependents` 独立交叉检查同样未发现 `VCRUNTIME`、`MSVCP`、版本化
`MSVCR`、`CONCRT` 或 GNU runtime DLL。

## Verification Routing

- `phase --release` 与 `phase --package`：Release build 后、package 前执行 Rust gate；
- `.github/workflows/ci.yml` 的 Windows Release job：构建后执行 Rust gate；
- `.github/workflows/release.yml`：exact tag/workflow build 后、打包前执行 Rust gate；
- candidate receipt 创建与复核：重新验证本地 exact EXE；
- downloaded artifact receipt：解压 ZIP 后验证 exact packaged EXE。

解析器有界读取 section/RVA、DLL name、descriptor count；并以显式 Windows inbox allowlist
拒绝未知 native DLL，而不只检查 developer-runtime 黑名单。malformed PE、越界 RVA、无 NUL
terminator 或无 descriptor terminator 均返回错误。它不依赖 Visual Studio 的 `dumpbin`，因此
GitHub-hosted verification 本身不需要额外 PE 工具。

## Boundary

- product runtime dependency delta：0；
- product behavior delta：0；
- release binary linkage policy：dynamic MSVC CRT -> static MSVC CRT；
- smoke CLI 仍是开发验证面，不进入 portable ZIP；
- Windows inbox DLL、`msvcrt.dll` 与 `api-ms-win-crt-*` API set 允许；
- 自动 import gate 不冒充 Clean VM 启动、Windows reputation 或真实机器验收。
