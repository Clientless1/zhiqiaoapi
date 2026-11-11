# 智桥API 修复与恢复说明

日期：2026-11-06（北京时间）

## 已完成

- 将浏览器标签页图标替换为你提供的智桥 API 图片，并生成 PNG、ICO、Apple Touch Icon；页面引用加入新的版本号。
- 新图标已部署到 http://192.168.1.113:8090 。已验证 HTML 引用和图标文件均返回 HTTP 200。
- 将 PostgreSQL 数据、运行配置/密钥、上传及生成资源这三个现有 Docker 卷设为固定名称的 external 卷。更换 Compose 项目名时继续使用这些卷，常规 compose down -v 也不会删除 external 卷。
- 保留服务重启策略及开机启动，为各容器配置 60 秒停止宽限。
- 已实际重建数据库和后端容器，三个服务重新达到 healthy；用户、渠道、API Key、用量、账单、设置表的记录数量，以及运行配置/密钥文件摘要，均与重建前一致。
- 数据库 fsync、full_page_writes、synchronous_commit 均为 on，正常的已提交数据库事务使用 PostgreSQL 断电恢复机制。
- 新增每小时及开机后自动备份，涵盖 PostgreSQL、数据库角色、配置密钥、资源和部署源文件。仅完整且校验成功的备份才会更新 latest；保留最新 48 份及最近 30 天每日一份。
- 在无外部网络、无公开端口的独立临时 PostgreSQL 中完整恢复过备份，恢复成功且关键表数量一致；临时容器已清理。
- 第一份备份已经复制到本次聊天工作目录中的受限目录，并校验全部 SHA-256 摘要。

## 外网故障及修复结果

域名：https://api.zhiqiaoapi.fun

原 Cloudflare Tunnel 云端规则把域名指向 http://192.168.1.107:8090，而当前服务器是 192.168.1.113，导致回源连接被拒绝、外网返回 HTTP 502。

2026-11-06 00:52（北京时间），已登录 Cloudflare 并将已有发布应用的 Service URL 改为 http://127.0.0.1:8090 。服务器 cloudflared 已收到版本 12 的新配置，4 条隧道连接就绪。cloudflared 与应用在同一台服务器上，本机回源不依赖局域网 IP。

已实际验证：

- Edge 通过 HTTPS 域名完整加载首页。
- 公网首页、/readyz、/api/public/site、新 favicon 均返回 HTTP 200。
- /readyz 显示数据库及计费写入健康。
- /v1/models 在不提供 API Key 时返回应用的 HTTP 401 unauthorized，证明公网请求已通过隧道抵达后端鉴权。
- 从当前电脑与服务器分别进行公网 curl 验证，结果一致。
- 公网图标的 SHA-256 与部署的新图片一致。

本次验证未使用有效 API Key 发起收费模型调用。

修复截图：Cloudflare-回源修复.png；公网恢复验证.png。

## 备份位置与命令（在服务器上运行）

项目：/home/zhiqiao/zhiqiaoapi

备份目录：/home/zhiqiao/backups/zhiqiaoapi

最新有效备份：/home/zhiqiao/backups/zhiqiaoapi/latest

立即生成完整备份：

```sh
python3 /home/zhiqiao/zhiqiaoapi/deploy/ops/backup-zhiqiaoapi.py
```

查看定时任务及最后一次结果：

```sh
systemctl list-timers zhiqiaoapi-backup.timer --no-pager
systemctl show zhiqiaoapi-backup.service -p Result -p ExecMainStatus
journalctl -u zhiqiaoapi-backup.service -n 30 --no-pager
```

再次演练恢复（只操作隔离临时数据库，不覆盖正式数据库）：

```sh
python3 /home/zhiqiao/zhiqiaoapi/deploy/ops/verify-restore.py
```

部署本次图标版本：

```sh
cd /home/zhiqiao/zhiqiaoapi
docker compose -f docker-compose.cn.yml -f docker-compose.branding.yml up -d --no-build --wait
```

需要重建图标镜像时：

```sh
cd /home/zhiqiao/zhiqiaoapi
docker compose -f docker-compose.cn.yml -f docker-compose.branding.yml build web
docker compose -f docker-compose.cn.yml -f docker-compose.branding.yml up -d --no-deps web
```

## 恢复时的顺序

1. 保留最新数据库 dump、backend-config.tar.gz、backend-assets.tar.gz、deployment.tar.gz 和 database-globals.sql；用 manifest.json 检查完整性。
2. 先在隔离临时数据库中演练恢复并验证数据。
3. 正式恢复应先暂停业务写入，另做当前现场备份，再恢复到新数据库/卷并核对。数据库及 backend-config 中的加密密钥必须配套保存；不要只恢复数据库后随机生成新密钥。
4. 确认新库、配置、资源和站点健康后再切换正式服务；保留旧卷以便回退。

## 已知限制

- 尚未找到能证明过去“全部数据丢失”的历史日志；不能将之前的数据丢失原因写成已经确诊。当前数据库卷自 10 月 22 日创建，现存数据可正常读取，本次重建验证没有丢失数据。
- 内存中尚未提交的请求记录不能通过数据库 fsync 保证断电不丢。已有 usage 队列默认每 200ms 写入；本次没有改动业务结算代码。
- 定时备份目前保存在同一台服务器磁盘；这次另复制了一份到你的电脑。服务器整盘损坏时，服务器内备份也会受影响。持续异机/对象存储备份尚未配置。
- 现有前端源码完整构建出现了原有类型/导出错误。本次图标镜像保留当前正常运行的编译产物，仅替换静态图标和 HTML 图标引用；原始源码的构建问题仍待单独处理。
- 未切断服务器电源或重启整台虚拟机；验证的是正常 Docker 容器重建以及完整备份恢复。

## 标签页图标缓存补充修复

已确认原后台标签页仍保留旧文档中的 /favicon.svg?v=zhiqiao-bridge 引用。将新图标移到独立文件名 /icons/zhiqiao-favicon-f745f88f.png 和 /icons/zhiqiao-favicon-c3e5aa56.ico，并为 SPA 文档及旧图标路径添加 no-store/no-cache 缓存控制。新镜像已构建部署且健康，已刷新原后台并确认新地址，在原生 Edge 新后台标签中实际看到用户提供的图片。截图：标签页图标-实际验证.png、标签页图标-局部验证.png。
