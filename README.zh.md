# CCometixLine

[English](README.md) | [中文](README.zh.md)

基于 Rust 的高性能 Claude Code 状态栏工具，集成 Git 信息、使用量跟踪、交互式 TUI 配置和 Claude Code 补丁工具。

![Language:Rust](https://img.shields.io/static/v1?label=Language&message=Rust&color=orange&style=flat-square)
![License:MIT](https://img.shields.io/static/v1?label=License&message=MIT&color=blue&style=flat-square)

## 截图

![CCometixLine](assets/img1.png)

状态栏显示：模型 | 目录 | Git 分支状态 | 上下文窗口信息

## 特性

### 核心功能
- **Git 集成** 显示分支、状态和跟踪信息
- **模型显示** 简化的 Claude 模型名称
- **使用量跟踪** 基于转录文件分析  
- **目录显示** 显示当前工作空间
- **简洁设计** 使用 Nerd Font 图标

### 交互式 TUI 功能
- **交互式主菜单** 无输入时直接执行显示菜单
- **TUI 配置界面** 实时预览配置效果
- **主题系统** 多种内置预设主题
- **段落自定义** 精细化控制各段落
- **配置管理** 初始化、检查、编辑配置

### Claude Code 增强
- **禁用上下文警告** 移除烦人的"Context low"消息
- **启用详细模式** 增强输出详细信息
- **稳定补丁器** 适应 Claude Code 版本更新
- **自动备份** 安全修改，支持轻松恢复

## 安装

### 快速安装（推荐）

通过 npm 安装（适用于所有平台）：

```bash
# 全局安装
npm install -g @cometix/ccline

# 或使用 yarn
yarn global add @cometix/ccline

# 或使用 pnpm
pnpm add -g @cometix/ccline
```

使用镜像源加速下载：
```bash
npm install -g @cometix/ccline --registry https://registry.npmmirror.com
```

安装后：
- ✅ 全局命令 `ccline` 可在任何地方使用
- ⚙️ 按照下方提示进行配置以集成到 Claude Code
- 🎨 运行 `ccline -c` 打开配置面板进行主题选择

### Claude Code 配置

添加到 Claude Code `settings.json`：

**跨平台通用（推荐）**
```json
{
  "statusLine": {
    "type": "command",
    "command": "~/.claude/ccline/ccline",
    "padding": 0
  }
}
```

> **Windows 用户注意：** 从 Claude Code v2.1.47+ 开始，Windows 上支持 Unix 风格路径解析。`~` 符号会自动展开为您的用户主目录。**请勿使用 `%USERPROFILE%`** — 它在 v2.1.47+ 版本中不再可靠。
> - 推荐：`~/.claude/ccline/ccline`（跨平台通用）
> - 备选：`"ccline"`（需要 npm 全局安装）

**后备方案 (npm 安装):**
```json
{
  "statusLine": {
    "type": "command",
    "command": "ccline",
    "padding": 0
  }
}
```
*如果 npm 全局安装已在 PATH 中可用，则使用此配置*

### 更新

```bash
npm update -g @cometix/ccline
```

<details>
<summary>手动安装（点击展开）</summary>

或者从 [Releases](https://github.com/Haleclipse/CCometixLine/releases) 手动下载：

#### Linux

#### 选项 1: 动态链接版本（推荐）
```bash
mkdir -p ~/.claude/ccline
wget https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-linux-x64.tar.gz
tar -xzf ccline-linux-x64.tar.gz
cp ccline ~/.claude/ccline/
chmod +x ~/.claude/ccline/ccline
```
*系统要求: Ubuntu 22.04+, CentOS 9+, Debian 11+, RHEL 9+ (glibc 2.35+)*

#### 选项 2: 静态链接版本（通用兼容）
```bash
mkdir -p ~/.claude/ccline
wget https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-linux-x64-static.tar.gz
tar -xzf ccline-linux-x64-static.tar.gz
cp ccline ~/.claude/ccline/
chmod +x ~/.claude/ccline/ccline
```
*适用于任何 Linux 发行版（静态链接，无依赖）*

#### macOS (Intel)

```bash  
mkdir -p ~/.claude/ccline
wget https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-macos-x64.tar.gz
tar -xzf ccline-macos-x64.tar.gz
cp ccline ~/.claude/ccline/
chmod +x ~/.claude/ccline/ccline
```

#### macOS (Apple Silicon)

```bash
mkdir -p ~/.claude/ccline  
wget https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-macos-arm64.tar.gz
tar -xzf ccline-macos-arm64.tar.gz
cp ccline ~/.claude/ccline/
chmod +x ~/.claude/ccline/ccline
```

#### Windows

```powershell
# 创建目录并下载
New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.claude\ccline"
Invoke-WebRequest -Uri "https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-windows-x64.zip" -OutFile "ccline-windows-x64.zip"
Expand-Archive -Path "ccline-windows-x64.zip" -DestinationPath "."
Move-Item "ccline.exe" "$env:USERPROFILE\.claude\ccline\"
```

</details>

### 从源码构建

```bash
git clone https://github.com/Haleclipse/CCometixLine.git
cd CCometixLine
cargo build --release
cp target/release/ccometixline ~/.claude/ccline/ccline
```

## 使用

### 主题覆盖

```bash
# 临时使用指定主题（覆盖配置文件设置）
ccline --theme cometix
ccline --theme minimal
ccline --theme gruvbox
ccline --theme nord
ccline --theme powerline-dark

# 或使用 ~/.claude/ccline/themes/ 目录下的自定义主题
ccline --theme my-custom-theme
```

### Claude Code 增强

```bash
# 禁用上下文警告并启用详细模式
ccline --patch /path/to/claude-code/cli.js

# 常见安装路径示例
ccline --patch ~/.local/share/fnm/node-versions/v24.4.1/installation/lib/node_modules/@anthropic-ai/claude-code/cli.js
```

## 默认段落

显示：`目录 | Git 分支状态 | 模型 | 上下文窗口`

### Git 状态指示器

- 带 Nerd Font 图标的分支名
- 状态：`✓` 清洁，`●` 有更改，`⚠` 冲突
- 远程跟踪：`↑n` 领先，`↓n` 落后

### 模型显示

显示简化的 Claude 模型名称：
- `claude-3-5-sonnet` → `Sonnet 3.5`
- `claude-4-sonnet` → `Sonnet 4`

### 上下文窗口显示

基于转录文件分析的令牌使用百分比，包含上下文限制跟踪。

## 配置

CCometixLine 支持通过 TOML 文件和交互式 TUI 进行完整配置：

- **配置文件**: `~/.claude/ccline/config.toml`
- **交互式 TUI**: `ccline --config` 实时编辑配置并预览效果
- **主题文件**: `~/.claude/ccline/themes/*.toml` 自定义主题文件
- **自动初始化**: `ccline --init` 创建默认配置

### 可用段落

所有段落都支持配置：
- 启用/禁用切换
- 自定义分隔符和图标
- 颜色自定义
- 格式选项

支持的段落：目录、Git、模型、使用量、时间、成本、输出样式

### 模型配置 (`models.toml`)

文件位置：`~/.claude/ccline/models.toml`（首次运行时自动创建）

此文件配置模型 ID 的显示名称及其上下文窗口限制。Claude 模型（Sonnet、Opus、Haiku）会自动识别并提取版本号，此文件仅用于覆盖默认行为或添加第三方模型支持。

```toml
# 模型条目：基于模型 ID 的子字符串匹配
# 优先级高于内置 Claude 模型识别
[[models]]
pattern = "glm-4.5"
display_name = "GLM-4.5"
context_limit = 128000

[[models]]
pattern = "kimi-k2"
display_name = "Kimi K2"
context_limit = 128000

# 上下文修饰符：独立匹配，可与模型条目组合使用
# 覆盖 context_limit 并将 display_suffix 追加到显示名称
# 例如：模型 "Opus 4" + 修饰符 " 1M" = "Opus 4 1M"
[[context_modifiers]]
pattern = "[1m]"
display_suffix = " 1M"
context_limit = 1000000
```


## 系统要求

- **Git**: 版本 1.5+ (推荐 Git 2.22+ 以获得更好的分支检测)
- **终端**: 必须支持 Nerd Font 图标正常显示
  - 安装 [Nerd Font](https://www.nerdfonts.com/) 字体
  - 中文用户推荐: [Maple Font](https://github.com/subframe7536/maple-font) (支持中文的 Nerd Font)
  - 在终端中配置使用该字体
- **Claude Code**: 用于状态栏集成

## 开发

```bash
# 构建开发版本
cargo build

# 运行测试
cargo test

# 构建优化版本
cargo build --release
```

## 路线图

- [x] TOML 配置文件支持
- [x] TUI 配置界面
- [x] 自定义主题
- [x] 交互式主菜单
- [x] Claude Code 增强工具

## 贡献

欢迎贡献！请随时提交 issue 或 pull request。

## 许可证

本项目采用 [MIT 许可证](LICENSE)。

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=Haleclipse/CCometixLine&type=Date)](https://star-history.com/#Haleclipse/CCometixLine&Date)