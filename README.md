> [!IMPORTANT]
> Remove this line to confirm you've reviewed this PR before submitting.

# Zed Lite

基于 [Zed](https://github.com/zed-industries/zed) **稳定版**的本地编辑器，支持英文与简体中文，移除 AI、遥测、账号和实时协作。

A local-project editor based on stable Zed releases, with English and Simplified Chinese UI. AI, telemetry, accounts, collaboration and remote development are removed.

## 版本与平台

- 当前上游基线：[`v1.26.2`](https://github.com/zed-industries/zed/releases/tag/v1.26.2)，应用版本同为 **1.26.2**。
- 只跟随上游 stable Release；不引入 nightly/preview，不单独递增版本号。合并新的稳定版后，应用版本采用该上游版本。
- 首版支持 **macOS Apple Silicon（arm64）**，仅提供 DMG。Intel、Windows 和 Linux 暂未验证。
- 这是独立维护的 fork，并非 Zed Industries 的官方发行版。

## 功能范围

保留本地文件与项目、LSP 补全/跳转/诊断、搜索、Git、终端、任务、调试和普通扩展。Git 的 origin/upstream、fetch/push 不受远程开发裁剪影响。

移除 AI Agent、模型供应商、编辑预测、AI 扩展入口、遥测及崩溃上传、Zed 账号、实时协作、语音通话、SSH/WSL 远程项目和 Dev Containers，以及对应的设置、命令、快捷键和 UI。

本地项目不等于离线模式：扩展、语言服务器下载、Git 和检查更新仍可能访问网络。

## 安装与构建测试

在 [GitHub Actions](https://github.com/TianJianJun0727/zed-lite/actions/workflows/local-editor-macos.yml) 的成功运行中下载 `Zed-Lite-macOS-arm64` artifact，解压得到 `Zed-Lite-macOS-arm64.dmg`。打开 DMG，将 **Zed Lite.app** 拖入 Applications。

安装包使用 ad hoc 签名，尚未经过 Apple Developer ID 签名与公证。CI 会检查应用/CLI 类型、菜单及快捷键、本地项目回归、设置迁移和更新器，构建 DMG 后检查签名、版本输出及包内 Git。CI 通过不等于真实界面验收；是否可下载以对应运行结果为准。

同分支的构建串行执行，正在运行的任务会保留，队列只保留最新待构建版本。翻译资源、格式及本地化单元测试先在 Linux runner 执行，通过后才申请 macOS ARM runner。macOS 构建缓存同时保存固定版本的打包工具。若 GitHub 明确报告 ARM runner 容量不足且没有执行任何构建步骤，会间隔 60/120 秒自动重试失败任务，最多重试两次；代码和测试失败不会自动重试，已有新提交时也不会重试旧版本。这可以减少人工恢复，但无法保证 GitHub 的 runner 供应。

应用与原版 Zed 使用独立的 Bundle ID 和用户目录：

- Bundle ID：`io.github.tianjianjun0727.zed-lite`
- 设置：`~/.config/zed-lite/settings.json`
- 数据：`~/Library/Application Support/Zed Lite`
- 日志：`~/Library/Logs/Zed Lite`
- “安装 CLI”命令安装 `/usr/local/bin/zed-lite`。

项目内继续兼容 `.zed/settings.json`、`.zed/tasks.json` 等上游格式。

回归测试通过临时 Cargo 配置将工作区运行时代码设为 `opt-level=1`，保留 release 的条件编译；过程宏和外部依赖沿用原配置。该临时配置用于减少测试二进制的优化编译开销；安装包继续使用现有发布参数。

仅修正 Git 资源或 DMG 打包时，可在手动运行工作流时填写 `package_from_run`（成功完整构建的运行 ID）。流程会检查原生编译、翻译覆盖和全部回归结果，并拒绝应用源码、翻译资源或 Cargo 输入有变化的复用；通过后下载原安装包，只重新打包，保留原 App 的二进制及编译配置。原生源提交与打包提交记录在 `build-provenance.json`。不填写该参数时执行完整构建。

```sh
gh workflow run local-editor-macos.yml --ref main -f package_from_run="${ZED_LITE_SOURCE_RUN_ID:?Set a successful native run ID}"
```

包内备用 Git 使用 [Dugite Native v2.43.3](https://github.com/desktop/dugite-native/releases/tag/v2.43.3) 的完整发行目录，包含 HTTPS helper 及 Git 的 `COPYING`。打包检查包含 Git 版本、本地仓库操作和 HTTPS 远端读取；用户已有的 Git 配置仍由 Git 处理。

## 界面语言

在设置中选择界面语言，或编辑：

```json
{
  "ui_language": "zh-CN"
}
```

可选值为 `system`（默认）、`en`、`zh-CN`。未支持的系统语言回退英文。切换后菜单及窗口会刷新；已打开的设置页和命令面板需要重新打开。命令 ID、快捷键和 JSON 设置名保持不变；架构可继续添加语言资源。

命令面板保留英文命令 ID，并显示中文名称；命令说明也用于快捷键编辑器和 JSON 设置中的命令帮助。设置字段名、枚举值及快捷键格式保持上游格式，翻译只作用于显示文字和 schema 描述。

翻译使用稳定 key 和 `rust-i18n`。英文及简中资源需要同时更新；普通命令以 action ID 为 key。通过命令参数打开设置页时，原来的英文页面名称在中文或伪语言界面中也可继续使用。schema 描述通过英文源描述映射到已登记的稳定 key，上游说明变化会由覆盖检查提示补译。`python3 script/check-locales.py` 检查资源、占位符与调用；构建后运行以下检查，核对实际注册命令、清理残留命令翻译，并校验英/中/伪语言 schema 的结构：

```sh
target/release/zed-lite --dump-all-actions > target/localization-contracts.json
python3 script/check-locales.py --actions target/localization-contracts.json
```

排查界面漏译时，可在独立的用户数据目录中启用伪语言：

```sh
ZED_LITE_PSEUDO_TRANSLATION=1 "/Applications/Zed Lite.app/Contents/MacOS/zed-lite" --user-data-dir /tmp/zed-lite-i18n-check
```

已接入翻译的文字会显示为 `⟦English text⟧`；没有标记的应用文案需要核查是否漏接。文件内容、用户输入、扩展或语言服务器文字、系统原生对话框不会因此自动翻译。命令面板和快捷键编辑器已打开时需重新打开，才能重建缓存的命令名称。

## 自动更新

自动及手动检查更新仅查询本仓库的 [GitHub Releases](https://github.com/TianJianJun0727/zed-lite/releases)，不使用官方 Zed 更新源。

Release 应使用 `zed-lite-v1.26.2` 这样的标签，版本部分与对应 Zed 稳定版一致；`zed-lite-` 仅区分 fork 的发布标签与上游标签。macOS arm64 资源名必须为 `Zed-Lite-macOS-arm64.dmg`，卷名 `Zed Lite`，内含 `Zed Lite.app`。客户端只下载比已安装版本更新的版本，同一上游版本的重新打包不会触发版本升级。

CI artifact 不会自动成为 Release；当前工作流只构建和测试，不自动发布。没有 Release 时，检查更新显示暂无更新。

## 与上游同步

保留 `upstream` 指向官方仓库，以新的 stable tag 为合并入口：

```sh
git remote add upstream https://github.com/zed-industries/zed.git # 已存在则跳过
git fetch upstream tag vX.Y.Z
git switch -c sync/zed-X.Y.Z main
git merge vX.Y.Z
```

只选择官方标为正式发布的 stable tag，解决冲突后确认 `crates/zed/Cargo.toml` 版本与该 tag 一致，保留语言与裁剪改动，再运行 macOS workflow 并测试 DMG。不要直接合并 upstream/main 或 nightly/preview 分支。 本 fork 仅保留本地编辑器的构建、翻译预检与 runner 容量恢复工作流；上游组织通知、社区机器人、文档部署和自动递增版本的工作流已移除。同步时也需检查 `.github/workflows`，保留这一边界。构建入口见 [workflow](.github/workflows/local-editor-macos.yml)，本地开发前置条件见 [macOS 构建文档](docs/src/development/macos.md)。

## 参考与许可

- 多语言参考 [ReachForStar/zed-ZH_CN](https://github.com/ReachForStar/zed-ZH_CN)，基于提交 `09a6ba45faf90d5b2231c867b3f8ccac6952cefe`，GPL-3.0-or-later。
- 功能裁剪及简中文案、命令与 schema 覆盖检查参考 [Wu](https://github.com/dstrml/wu)。本轮翻译参考快照为 `ef99cb0db63508bba5bed32cbc92733763584d14`（GPL-3.0-or-later），保留稳定 key 架构并按本地编辑器范围调整。
- 上游编辑器由 Zed Industries 开发。本 fork 保留适用的原始许可证及第三方声明。

### Licensing

Zed source code is licensed primarily under GPL-3.0-or-later, with Apache-2.0 components where marked.

License information for third party dependencies must be correctly provided for CI to pass.

We use [`cargo-about`](https://github.com/EmbarkStudios/cargo-about) to automatically comply with open source licenses. If CI is failing, check the following:

- Is it showing a `no license specified` error for a crate you've created? If so, add `publish = false` under `[package]` in your crate's Cargo.toml.
- Is the error `failed to satisfy license requirements` for a dependency? If so, first determine what license the project has and whether this system is sufficient to comply with this license's requirements. If you're unsure, ask a lawyer. Once you've verified that this system is acceptable add the license's SPDX identifier to the `accepted` array in `script/licenses/zed-licenses.toml`.
- Is `cargo-about` unable to find the license for a dependency? If so, add a clarification field at the end of `script/licenses/zed-licenses.toml`, as specified in the [cargo-about book](https://embarkstudios.github.io/cargo-about/cli/generate/config.html#crate-configuration).
