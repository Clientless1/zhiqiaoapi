# 第三方登录与邮件连接修复

2026-11-12，北京时间。生产项目：`/home/zhiqiao/zhiqiaoapi`。

## 故障与行为

GitHub 授权成功后返回 `oauth_account_exists`：同邮箱的本地账户已经存在，但尚未绑定第三方身份。现在回调直接显示「连接已有账户」，使用原账户密码或登录邮箱验证码任意一种证明账户归属后，在事务中绑定并签发登录会话。绑定后可以直接使用对应第三方登录。

不能仅按第三方邮箱静默合并账户。新增迁移 `0021_oauth_pending_link.sql` 只保存经过供应商验证的身份及票据摘要，票据绑定原浏览器、10 分钟过期、单次消费，不保存供应商 access token。账户停用、强制改密、绑定冲突、错误浏览器或错误验证码用途都会拒绝。

前端立即从地址栏移除回调票据并保存在页面内存中；刷新后需要重新发起登录。路由滚动忽略 OAuth fragment，普通章节锚点通过元素 ID 定位，避免把 `#connect=...` 当成 CSS selector 引发异常。

QQ SMTP 曾出现 TLS 握手超时、连接意外关闭。现场探测发现 smtp.qq.com 的一个 465 IPv4 端点超时，另一个可用，IPv6 无出站路由；587 STARTTLS 两个 IPv4 端点都可连接。生产配置已改为 **587 + TLS**，保留原发件地址及加密凭据。

邮件共用发送路径现在对 DNS 返回地址并行建立连接并选择完成 TLS/EHLO 的可用连接。DNS、连接、认证、提交各阶段设有超时；TLS 始终校验配置的主机名。QQ 465 在认证/提交之前连接失败时可尝试 587，必须 STARTTLS。开始邮件提交后不自动重发，以免重复发送验证码或重置邮件。永久认证或发件策略拒绝仍需处理供应商配置，不能靠重试规避。

## 验证结果与范围

- 后端 603 项通过，46 项数据库测试在普通运行中忽略；另行连接隔离 PostgreSQL 运行 10 项认证数据库测试全部通过。
- 前端 87 项测试通过，类型检查、ESLint 和生产构建通过。
- 隔离 HTTP 环境完成原账户验证绑定、签发会话、读取当前用户；错误浏览器和重放请求返回 401；品牌邮件发送到隔离 SMTP 接收器。
- 公网 Edge 实际 GitHub 授权返回新的绑定页面，两种验证方式切换正常，新构建资源已加载，控制台无警告/错误。未代用户提交原账户凭据或绑定身份。
- 生产后台 SMTP 测试接口只发送一封邮件到配置的发件邮箱，2.76 秒返回成功，QQ SMTP 已接受。未读取收件箱，不能据此保证收件或长期投递稳定。
- Google 入口、账号选择/授权页及服务器连接已验证。用户此前选择跳过真实账号授权，尚未验收真实 Google 授权后完整登录，不应宣称已全部通过。

## 部署与回滚

后端镜像：`zhiqiaoapi-backend:auth-delivery-20261112`。
最终前端镜像：`zhiqiaoapi-web:auth-delivery-final-20261112`。
`latest` 已指向以上镜像。前端保留旧静态资源及预压缩，现有 Cloudflare Tunnel 与 OAuth 专用代理规则保持原配置。

现场日志及无凭据测试结果在 `/home/zhiqiao/build-auth-delivery-20261112/`。生产 SMTP 测试记录为 `smtp-production-test.json`。配置密钥和任何 SMTP/OAuth/节点凭据不得提交仓库。

```sh
cd /home/zhiqiao/zhiqiaoapi
docker compose -f docker-compose.cloud.yml ps
systemctl is-active cloudflared zhiqiao-oauth-proxy
curl -fsS http://127.0.0.1:8091/readyz
```

需要回滚应用时，使用部署前保留的镜像，保留数据库及三个固定卷。新增表与旧应用兼容，无需删除表或降级数据库：

```sh
docker tag zhiqiaoapi-backend:before-auth-delivery-20261112 zhiqiaoapi-backend:latest
docker tag zhiqiaoapi-web:before-auth-delivery-20261112 zhiqiaoapi-web:latest
docker compose -f docker-compose.cloud.yml up -d --no-deps --no-build --pull never --wait backend web
```

SMTP 587 是独立运行配置，应用回滚不恢复为故障的 465。避免无故修改订阅节点或将 SMTP、模型请求、Cloudflare Tunnel 接入 OAuth 专用代理。
