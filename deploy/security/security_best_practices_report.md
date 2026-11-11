# 公网服务安全加固记录

日期：2026-11-07（北京时间）。对象：京东云 `111.228.31.204`，项目 `/home/zhiqiao/zhiqiaoapi`。

## 结果

已实际部署 SSH 密钥认证、主机与云端防火墙、SSH 自动封禁、网站认证限流、安全响应头、敏感路径拦截及不记录查询参数的外层访问日志。首页、登录页、既有 API Key 和真实 Responses 流式调用验证通过。Ubuntu 安全更新源与每日自动更新已补齐，现有待安装安全补丁已安装并重启生效。

本次以服务器和入口防护为主，未替换业务加密密钥、用户密码、API Key 或认证协议。没有发现可据此确认已被入侵的证据；日志中的失败 SSH 尝试不等于入侵成功。本记录不是完整渗透测试。

## 发现与处置

| 编号 | 等级 | 已确认问题 | 已部署处置与证据 |
| --- | --- | --- | --- |
| SEC-01 | 高 | 公网 root 可使用密码登录，日志有持续密码尝试 | 创建本电脑专用密钥，独立验证连接后关闭密码与交互认证；保留 root 密钥认证。`sshd-hardening.conf:2-11`；新连接再次成功。 |
| SEC-02 | 高 | 安全更新周期关闭，APT 源缺少 noble-security；OpenSSH/OpenSSL 等落后 | 补充经 Ubuntu 签名校验的官方 HTTPS 安全源，开启每日安全更新并禁止自动重启。`ubuntu-security.sources:1-5`、`52-zhiqiao-security-updates:1-4`。关键补丁安装结果另记。 |
| SEC-03 | 中 | 主机防火墙未启用，云防火墙保留闲置 3389 | UFW 默认拒绝入站和转发，仅允许 TCP22/80/443（含 IPv6）；删除云端 3389 规则。外网测试 3389/5432/8080/8090/8443 均不可连接。Docker 只发布 127.0.0.1:8090，数据库和后端无公网映射。 |
| SEC-04 | 中 | 普通登录入口缺少统一入口限流 | 认证四类路径每 IP 10 次/分钟，burst10、并发10，超限429与 Retry-After60；不计入模型 API。`nginx-security-global.conf:2-7`、`nginx-security-server.conf:8-18`。16次缺字段测试为11次422、5次429。 |
| SEC-05 | 中 | 缺少浏览器防护头；SPA 可能掩盖敏感路径探测 | 添加 script-src/self、object-src/none、base-uri/self、frame-ancestors/self，nosniff、SAMEORIGIN、Referrer-Policy；隐藏 Nginx 版本。隐藏配置及 Git 路径404。`nginx-security-server.conf:1-5,21-24`。首页和登录页均无浏览器 CSP/控制台错误。 |
| SEC-06 | 中 | 默认访问日志包含查询参数，重置链接等参数可能进入日志 | 外层 Nginx 使用仅含 URI 路径的 JSON 日志；不加入查询参数、认证头、Cookie、请求体。`nginx-security-global.conf:14-19`。独特查询标记测试确认未进入该访问日志。历史日志和应用/错误日志不在此保证范围。 |
| SEC-07 | 低 | root 入口备份脚本位于普通账号可替换的项目目录 | 定时执行路径移至 `/usr/local/sbin/zhiqiao-origin-backup.py`，root:root、0700，父目录 root 所有。`../migration-jdcloud/direct/zhiqiao-origin-backup.service:9`；受限归档加入 SSH、UFW、Fail2ban、APT 源与配置。 |

SSH Fail2ban 策略：10分钟5次失败封禁1小时，递增至最多24小时，systemd 日志后端、UFW 动作。配置 `fail2ban.local:1-14`；已用保留测试地址 `192.0.2.123` 验证生成拒绝规则并立即解封；该测试计入 Total banned，不代表真实攻击封禁数量。

## 验证

- `sshd -t`、`nginx -t`、`fail2ban-client -t` 均通过；配置加载后服务正常。
- 本电脑独立新建 SSH 密钥连接成功；原复用连接不是唯一验证依据。
- 外网 HTTPS `/readyz` 200，`ssl_verify_result=0`；HTTP ACME 检查200，保留证书续期通道。
- 未登录 `/api/me`、`/api/user/overview`、管理设置和 `/v1/models` 均401/403；`/.env`、`/.git/config`、编码点路径404。
- `/api/setup/status` 确认 setup_completed=true，初始化 providers 接口409；审阅 policy.rs 确认安装写入操作有已初始化保护。未调用初始化写入接口。
- 登录限流在服务器回环地址上测试，避免消耗用户公网 IP 的登录额度；限流后健康检查和模型认证仍按原规则工作。
- 浏览器加载首页、登录页成功，控制台0错误；截图及请求记录在 `../../output/playwright/security-*`。
- 两个既有 Key 模型列表均200，分别4和1个模型；真实 GPT Responses SSE 请求200且收到 response.completed。
- Fail2ban 封禁/解封与 UFW 联动通过；当前规则仅22/80/443。
- 验证脚本 `verify-ingress.py` 不使用真实账号尝试错误密码，不创建账号，不更改业务配置。

## 更新记录

先升级10个关键组件，再按已安装软件的 Ubuntu noble-security 来源筛选并升级199个软件包、安装7个依赖包（包含新内核），无软件包移除。APT 执行返回0，dpkg --audit 无异常；最终安全源候选更新为0（基于检查时已刷新索引，不等于不存在未知漏洞，也不代表安装全部普通功能更新）。

| 组件 | 最终版本 |
| --- | --- |
| OpenSSH Server | 1:9.6p1-3ubuntu13.20 |
| OpenSSL | 3.0.13-0ubuntu3.16 |
| curl | 8.5.0-2ubuntu10.16 |
| sudo | 1.9.15p5-3ubuntu5.24.04.4 |
| 运行内核 | 6.8.0-147-generic |

已执行一次系统重启，独立密钥连接成功，三个业务容器均 healthy；nginx、Docker、SSH、UFW、Fail2ban 均 active，备份与 IP 证书续期定时器均 enabled；不再有 reboot-required 标记。重启后外网 HTTPS readiness200、证书校验0、ACME HTTP200，安全规则保留。升级日志仅存服务器 root 受限目录 `/root/zhiqiao-security-before-20261107/`。

## 剩余限制

- 前端仍将 Bearer 会话令牌保存于 localStorage（`../github-upload/frontend/src/stores/auth.ts:11,100`）。本次 CSP 降低脚本注入风险，但未将认证协议迁移为 HttpOnly Cookie；未发现可据此证明存在 XSS 的证据。
- 主机防火墙和应用限流不能提供上游带宽级 DDoS 清洗能力。后续增设 Docker 端口映射时，必须重新检查公网暴露；Docker 的转发规则可能绕过普通 UFW INPUT 规则。
- 本次浏览器验证覆盖未登录页面；受保护接口的未认证拒绝与既有 Key 调用已验证，没有用管理员密码执行完整后台操作回归。
- 业务和入口定时备份位于同一云主机；本电脑另存经过 SHA256 校验的受限入口快照。没有配置持续异地备份，不能把同盘备份称为异地灾备。
- 域名接入问题按用户要求暂缓；当前公网入口是可信 IP HTTPS。

## 运维

在本电脑连接：`ssh zhiqiao-jdcloud`。专用密钥 `/Users/congwenjie/.ssh/zhiqiao-jdcloud_ed25519` 只留在本电脑，请妥善保管。SSH 密码入口已关闭；云控制台恢复入口保留。

```sh
ufw status verbose
fail2ban-client status sshd
fail2ban-client set sshd unbanip <需要解封的IP>
sshd -t && systemctl reload ssh
nginx -t && systemctl reload nginx
systemctl list-timers apt-daily.timer apt-daily-upgrade.timer zhiqiao-origin-backup.timer zhiqiaoapi-backup.timer zhiqiao-ip-cert-renew.timer --no-pager
systemctl start zhiqiao-origin-backup.service
systemctl show zhiqiao-origin-backup.service -p Result -p ExecMainStatus
```

加固前受限快照 `/root/zhiqiao-security-before-20261107/host-config-before.tar.gz`，最新入口与安全配置归档 `/root/zhiqiao-origin-backups/latest`。恢复涉及私钥与登录控制，应通过控制台逐项恢复、验证 SSH 与 HTTPS 后再离开；不要盲目覆盖整个 /etc 或关闭防火墙。临时自动回退定时器在新连接和公网验证通过后已取消。

备份及 SSH 私钥不可提交 GitHub 或放入站点可访问目录。
