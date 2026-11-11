# CC Switch 与手动客户端配置

## 准备

在智桥「API 密钥」创建自己的 Key，给 Key 分配可用额度和模型。下面用 `https://zhiqiaoapi.fun` 演示，私有部署请替换成自己的 HTTPS 地址。

查询该 Key 实际可用的模型：

```sh
curl https://zhiqiaoapi.fun/v1/models \
  -H 'Authorization: Bearer YOUR_API_KEY'
```

将示例中的模型占位符替换为返回列表里的完整模型 ID。不要使用登录会话、上游凭证或其他用户的 Key。

## CC Switch 一键导入

1. 启动 CC Switch，在智桥打开「API 密钥」。
2. 在目标 Key 点击「导入 CC Switch」，选择 Claude Code、Codex 或 OpenCode。
3. 选择该 Key 有权限的模型，点击导入，并允许浏览器打开本机 CC Switch。
4. 在 CC Switch 确认导入并启用供应商，重启正在运行的客户端。

导入使用 CC Switch 官方 `ccswitch://v1/import` 协议，在本机传递配置；链接包含 Key，应与密码一样保管，不要转发。

## 在 CC Switch 手动添加

| 客户端 | Base URL | 协议 | 凭证 |
| --- | --- | --- | --- |
| Codex | `https://zhiqiaoapi.fun/v1` | Responses | 智桥 API Key |
| Claude Code | `https://zhiqiaoapi.fun` | Anthropic Messages | 智桥 API Key |
| OpenCode（GPT） | `https://zhiqiaoapi.fun/v1` | OpenAI Responses | 智桥 API Key |
| OpenCode（其他兼容模型） | `https://zhiqiaoapi.fun/v1` | 对应模型支持的协议 | 智桥 API Key |

Codex 的配置设 `requires_openai_auth = false`，并为当前供应商填写 `experimental_bearer_token`，避免切换时沿用其他服务的登录凭证。Claude 的 Opus、Sonnet、Haiku、推理模型均选择 Key 允许的模型。

## 直接配置 Codex

编辑 `~/.codex/config.toml`（Windows 为 `%USERPROFILE%\.codex\config.toml`），保留个人设置并合并以下供应商配置：

```toml
model_provider = "zhiqiao"
model = "YOUR_ALLOWED_GPT_MODEL"

[model_providers.zhiqiao]
name = "智桥 API"
base_url = "https://zhiqiaoapi.fun/v1"
wire_api = "responses"
requires_openai_auth = false
experimental_bearer_token = "YOUR_API_KEY"
```

对应 `~/.codex/auth.json`：

```json
{"OPENAI_API_KEY": "YOUR_API_KEY", "auth_mode": "apikey"}
```

两处 Key 保持一致，配置文件仅供本人读取。服务不会要求你用 ChatGPT 账号登录才能使用智桥 Key。

## 直接配置 Claude Code

合并到 `~/.claude/settings.json`（Windows 为 `%USERPROFILE%\.claude\settings.json`）：

```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "https://zhiqiaoapi.fun",
    "ANTHROPIC_AUTH_TOKEN": "YOUR_API_KEY",
    "ANTHROPIC_MODEL": "YOUR_ALLOWED_CLAUDE_MODEL",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "YOUR_ALLOWED_CLAUDE_MODEL",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "YOUR_ALLOWED_CLAUDE_MODEL",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "YOUR_ALLOWED_CLAUDE_MODEL",
    "ANTHROPIC_REASONING_MODEL": "YOUR_ALLOWED_CLAUDE_MODEL",
    "ANTHROPIC_CUSTOM_MODEL_OPTION": "YOUR_ALLOWED_CLAUDE_MODEL"
  }
}
```

站点同样支持 `/anthropic` 前缀，使用它时完整请求为 `/anthropic/v1/messages`。不要将 Base URL 写成以 `/v1/messages` 结尾的完整请求地址。

## API 调试台

输入自己的 API Key 后，模型选择器从 `/v1/models` 读取该 Key 的模型权限。

- 对话模式按模型选择 Responses、Chat Completions 或 Anthropic Messages；也可以手动切换模型支持的协议。
- JSON 请求模式可测试文本、嵌入、审核、图片生成、语音合成、视频创建等接口；请求体中的 `model` 使用当前选择。
- 音频上传、图片编辑等 multipart 文件请求使用 SDK 或命令行。语音合成的二进制输出可复制请求到终端保存。

## 排查

| 现象 | 检查 |
| --- | --- |
| 401 | 检查当前启用供应商、Base URL、Key 是否一致；重启客户端，检查残留的环境变量或其他供应商登录凭证。 |
| 模型无权限 | 使用 `/v1/models` 返回的完整 ID，检查 Key 模型范围和到期时间。 |
| 余额不足 | 同时检查用户账户与 Key 已分配额度。 |
| 424 / 上游认证失败 | 管理员检查上游渠道的凭证和协议；有效的智桥 Key 无法修复失效的上游凭证。 |
| 导入后仍走旧平台 | 在 CC Switch 启用新供应商，关闭并重新启动客户端，检查供应商专属 Bearer Key。 |

## 第三方登录

新用户在注册页同意用户协议和隐私政策后，使用 GitHub 或 Google 完成注册；此后可以直接登录。已有邮箱账户先用密码或邮箱验证码登录，再到「个人设置 → 登录方式」绑定。平台不会仅凭相同邮箱自动合并账户。
