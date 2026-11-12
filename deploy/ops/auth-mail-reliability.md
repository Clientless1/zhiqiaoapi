# 第三方登录与邮件连接修复

2026-11-12，北京时间。生产项目：`/home/zhiqiao/zhiqiaoapi`。

## 故障与行为

GitHub 授权成功后曾返回 `oauth_account_exists`：同邮箱的本地账户已经存在，但尚未绑定第三方身份。首轮修复增加了原账户验证表单。用户随后明确要求 GitHub / Google 授权后直接登录，现已取消该表单，行为如下：

- 已绑定的第三方 subject 直接登录对应用户；邮箱后续改变不重新分配账户。
- 首次登录以服务端从供应商取得的已验证邮箱匹配现有用户，在事务中关联并登录，不再要求本站密码或验证码。GitHub 必须为 verified primary email，Google 必须为 `email_verified=true`；不信任浏览器提交的邮箱。
- 登录页和注册页共用第三方注册/登录入口。页面显示用户协议、隐私政策及自动创建账户说明；授权后，已有账户登录，新用户在注册开放时自动创建账户、默认项目及协议记录。内部审核模式保留待审核限制。
- 关闭注册只阻止新建账户，已有账户仍可第三方登录。停用、强制改密、同供应商身份冲突均保留限制。OAuth 只签发普通用户会话，不匹配管理员表或增加管理员权限。
- 原 `0021_oauth_pending_link.sql` 迁移及表保留以便兼容和回滚，新流程不再产生待绑定票据，`confirm-link` 接口和前端表单已移除。PKCE、state、浏览器 Cookie 绑定、60 秒单次会话票据仍保留。

前端立即从地址栏移除回调票据，再自动换取会话进入工作台；过期或取消后需要重新发起登录。路由滚动忽略 OAuth fragment，普通章节锚点通过元素 ID 定位，避免把回调参数当成 CSS selector 引发异常。

QQ SMTP 曾出现 TLS 握手超时、连接意外关闭。现场探测发现 smtp.qq.com 的一个 465 IPv4 端点超时，另一个可用，IPv6 无出站路由；587 STARTTLS 两个 IPv4 端点都可连接。生产配置已改为 **587 + TLS**，保留原发件地址及加密凭据。

邮件共用发送路径现在对 DNS 返回地址并行建立连接并选择完成 TLS/EHLO 的可用连接。DNS、连接、认证、提交各阶段设有超时；TLS 始终校验配置的主机名。QQ 465 在认证/提交之前连接失败时可尝试 587，必须 STARTTLS。开始邮件提交后不自动重发，以免重复发送验证码或重置邮件。永久认证或发件策略拒绝仍需处理供应商配置，不能靠重试规避。

## 验证结果与范围

- 最终直接登录版本后端 603 项通过，45 项数据库测试在普通运行中忽略；另行连接隔离 PostgreSQL 运行 9 项认证数据库测试全部通过。涵盖两个供应商自动关联、重复登录、邮箱变化、冲突、禁用/强制改密、新用户协议/注册开关、默认项目及内部审核限制。
- 前端 87 项测试通过，类型检查、ESLint 和生产构建通过。
- 最终隔离 HTTP 环境完成单次票据换取用户会话、读取当前用户；错误浏览器和重放请求返回 401，普通用户会话无法访问管理员接口，旧确认接口返回 404。首轮修复已将品牌邮件发送到隔离 SMTP 接收器。
- 最终公网 Edge 实际 GitHub 授权后自动进入 `/home/overview`，无原账户密码/验证码表单，控制台无警告或错误。仅验证既有授权范围的登录，没有代用户接受新增 Google 授权。
- 生产后台 SMTP 测试接口只发送一封邮件到配置的发件邮箱，2.76 秒返回成功，QQ SMTP 已接受。未读取收件箱，不能据此保证收件或长期投递稳定。
- Google 入口、账号选择/授权页及服务器连接已验证。用户此前选择跳过真实账号授权，尚未验收真实 Google 授权后完整登录，不应宣称已全部通过。

## 部署与回滚

后端镜像：`zhiqiaoapi-backend:oauth-direct-20261112`。
最终前端镜像：`zhiqiaoapi-web:oauth-direct-20261112`。
`latest` 已指向以上镜像。前端保留旧静态资源及预压缩，现有 Cloudflare Tunnel 与 OAuth 专用代理规则保持原配置。

直接登录版本日志和隔离验收结果在 `/home/zhiqiao/build-oauth-direct-20261112/`，首轮邮件修复记录在 `/home/zhiqiao/build-auth-delivery-20261112/`，其中生产 SMTP 测试记录为 `smtp-production-test.json`。配置密钥和任何 SMTP/OAuth/节点凭据不得提交仓库。

```sh
cd /home/zhiqiao/zhiqiaoapi
docker compose -f docker-compose.cloud.yml ps
systemctl is-active cloudflared zhiqiao-oauth-proxy
curl -fsS http://127.0.0.1:8091/readyz
```

需要回滚应用时，使用部署前保留的镜像，保留数据库及三个固定卷。新增表与旧应用兼容，无需删除表或降级数据库：

```sh
docker tag zhiqiaoapi-backend:before-oauth-direct-20261112 zhiqiaoapi-backend:latest
docker tag zhiqiaoapi-web:before-oauth-direct-20261112 zhiqiaoapi-web:latest
docker compose -f docker-compose.cloud.yml up -d --no-deps --no-build --pull never --wait backend web
```

SMTP 587 是独立运行配置，应用回滚不恢复为故障的 465。避免无故修改订阅节点或将 SMTP、模型请求、Cloudflare Tunnel 接入 OAuth 专用代理。
