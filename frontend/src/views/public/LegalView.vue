<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import BrandLogo from '../../components/common/BrandLogo.vue'
import { useSiteBrand } from '../../composables/useSiteBrand'

const route = useRoute()
const { siteName } = useSiteBrand()
const privacy = computed(() => route.path === '/privacy')
const title = computed(() => (privacy.value ? '隐私政策' : '用户协议'))
const sections = computed(() =>
  privacy.value
    ? [
        {
          title: '我们处理哪些信息',
          text: [
            '账户信息：注册邮箱、经过加密处理的密码校验信息，以及你主动填写的账户资料。使用 GitHub 或 Google 登录时，我们会接收该平台提供的账户标识和经过验证的邮箱。',
            '服务记录：API 密钥标识、调用时间、模型、用量、计费信息、错误状态及维护服务所需的安全日志。请勿在请求中提交与你的任务无关的敏感个人信息。'
          ]
        },
        {
          title: '信息如何使用',
          text: [
            '信息用于验证身份、提供模型转发服务、统计用量和费用、处理订单、排查故障，以及识别滥用行为。我们不会因为你未选择第三方登录而要求你提供该平台的资料。',
            '你提交的 API 请求会按照所选模型发送至相应上游服务商。上游服务商对请求数据的处理受其自身政策约束；调用前请确认该服务适合你要处理的数据。'
          ]
        },
        {
          title: '共享与委托处理',
          text: [
            '为完成你的请求，必要的信息可能发送至邮件服务商、你主动选择的身份认证平台、支付服务商及模型供应商。具体范围取决于你使用的功能。我们不会出售你的个人信息。',
            '在适用法律要求、合法司法程序或保护服务及用户安全确有必要时，可能依法提供相关信息。'
          ]
        },
        {
          title: '存储与安全',
          text: [
            '我们在提供服务、履行账务义务和处理安全事件所需的期限内保留相关记录，采用访问控制、加密传输、备份和权限管理等措施。任何互联网服务均无法承诺绝对安全。',
            '浏览器本地存储用于维持登录状态、语言偏好等必要功能。退出登录会移除当前浏览器的登录凭据；请在公用设备使用后退出。'
          ]
        },
        {
          title: '你的权利与选择',
          text: [
            '你可以在账户设置中查看、修改可用资料，并通过站内提供的联系渠道向管理员提出信息查询、更正、删除或注销请求。处理请求时，我们可能需要合理核实你的身份。',
            '依法需要保留的交易、安全记录，或仍有未结算订单时，部分信息可能无法立即删除。我们会说明相关原因。'
          ]
        },
        {
          title: '政策更新',
          text: [
            '功能或处理方式发生变化时，本政策会相应更新。涉及你的重要权益的变化，将通过站内通知或其他适当方式告知。请同时阅读用户协议。'
          ]
        }
      ]
    : [
        {
          title: '适用范围',
          text: [
            `本协议适用于 ${siteName.value} 提供的网站、账户系统和 API 转发服务。注册账户或使用相关功能前，请阅读本协议及隐私政策。具体模型、价格、额度和可用功能以页面及实际订单为准。`
          ]
        },
        {
          title: '账户与访问凭据',
          text: [
            '注册时应使用你有权使用的邮箱，并提供真实、合法的信息。密码登录和邮箱验证码登录均用于访问已注册账户；第三方登录是否可用以站点配置为准。',
            '你应妥善保管密码、邮箱和 API 密钥，对通过你的账户进行的操作负责。发现凭据泄露时，请立即撤销或更换密钥，并联系管理员。不得未经授权访问他人账户或转让受限制的访问权限。'
          ]
        },
        {
          title: '使用规则',
          text: [
            '你应遵守适用法律、所调用模型供应商的使用规则及本服务公布的限制。不得利用本服务实施欺诈、侵权、攻击、绕过访问控制或其他违法行为。',
            '仅提交你有权处理的内容；对于个人信息、商业秘密或其他敏感资料，应事先确认必要的授权及保护措施。模型输出可能不准确，重要决定应由你独立核实。'
          ]
        },
        {
          title: '用量、费用与订单',
          text: [
            'API 调用按照页面展示的计费规则结算。调用费用可能涉及输入、输出、缓存或其他计量单位；请在使用前查看模型价格和账户余额。',
            '支付、充值、退款或异议处理以实际订单条款、站内公布的规则及适用法律为准。对账有疑问时，请提供订单号或请求记录，勿通过公开渠道发送完整 API 密钥。'
          ]
        },
        {
          title: '服务可用性与限制',
          text: [
            '上游供应商、网络、维护及不可抗力等因素可能影响请求成功率和响应时间。我们会合理维护服务，但不承诺所有模型持续可用，也不承诺输出适合特定目的。',
            '为保护账户、其他用户或服务安全，我们可能对异常请求进行限流、暂停或调查。涉及你的合法权益时，你可以通过站内联系渠道提出申诉。'
          ]
        },
        {
          title: '知识产权与数据',
          text: [
            '各方保留对其原有软件、商标和内容依法享有的权利。你提交的内容不会仅因使用本服务而转让其权利；输出内容的权利及使用限制同时受适用法律和上游供应商条款影响。',
            '为提供和维护服务，我们按隐私政策处理必要的账户、调用和账务记录。请在使用前阅读隐私政策。'
          ]
        },
        {
          title: '终止、变更与联系',
          text: [
            '你可以停止使用服务，并通过站内联系渠道申请注销账户。尚未结清的费用、依法需保留的记录及已产生的权利义务不因停用而自动消失。',
            '本协议随服务变化可能更新。重大变化将通过适当方式告知。发生争议时，双方应先友好协商；无法解决时，按适用法律处理。'
          ]
        }
      ]
)
</script>

<template>
  <main class="legal-page">
    <header class="legal-nav">
      <RouterLink to="/" :aria-label="siteName"><BrandLogo /></RouterLink
      ><RouterLink to="/login">返回登录 ↗</RouterLink>
    </header>
    <div class="legal-layout">
      <aside class="legal-index">
        <span>服务条款</span
        ><RouterLink to="/terms" :class="{ active: !privacy }">用户协议</RouterLink
        ><RouterLink to="/privacy" :class="{ active: privacy }">隐私政策</RouterLink>
      </aside>
      <article class="legal-document">
        <header>
          <p class="legal-kicker">{{ siteName }}</p>
          <h1>{{ title }}</h1>
          <p>更新日期：2026 年 11 月 7 日 · 版本 2026-11-07</p>
        </header>
        <section v-for="(section, index) in sections" :key="section.title">
          <h2>{{ index + 1 }}. {{ section.title }}</h2>
          <p v-for="paragraph in section.text" :key="paragraph">{{ paragraph }}</p>
        </section>
        <footer>如有与服务、账户或本政策有关的问题，请通过站内公布的联系渠道联系管理员。</footer>
      </article>
    </div>
  </main>
</template>

<style scoped>
.legal-page {
  min-height: 100dvh;
  background: #fff;
  color: #253041;
  font-family:
    Inter,
    -apple-system,
    BlinkMacSystemFont,
    'Segoe UI',
    'PingFang SC',
    sans-serif;
}
.legal-nav {
  height: 86px;
  max-width: 1120px;
  margin: auto;
  padding: 0 32px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid #edf0f3;
}
.legal-nav > a:last-child {
  color: #647184;
  font-size: 13px;
  text-decoration: none;
}
.legal-layout {
  max-width: 1120px;
  margin: auto;
  padding: 64px 32px 96px;
  display: grid;
  grid-template-columns: 180px minmax(0, 720px);
  gap: 64px;
}
.legal-index {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 20px;
  font-size: 14px;
}
.legal-index > span {
  font-size: 11px;
  color: #8b95a2;
  margin-bottom: 4px;
}
.legal-index > a {
  color: #76808d;
  text-decoration: none;
}
.legal-index > a.active {
  color: #2157ce;
  font-weight: 600;
}
.legal-document header {
  padding-bottom: 34px;
  margin-bottom: 36px;
  border-bottom: 1px solid #e8ebef;
}
.legal-kicker {
  font-size: 12px;
  color: #8b95a2;
  letter-spacing: 0.04em;
}
.legal-document h1 {
  font-size: 34px;
  letter-spacing: -0.035em;
  margin: 12px 0 16px;
  color: #172231;
  font-weight: 650;
}
.legal-document header > p:last-child {
  font-size: 12px;
  color: #8b95a2;
}
.legal-document section {
  margin-bottom: 32px;
}
.legal-document h2 {
  font-size: 17px;
  margin: 0 0 14px;
  font-weight: 600;
  color: #172231;
}
.legal-document section p {
  font-size: 14px;
  line-height: 1.95;
  color: #627083;
  margin: 10px 0;
}
.legal-document footer {
  padding-top: 26px;
  border-top: 1px solid #edf0f3;
  font-size: 12px;
  line-height: 1.8;
  color: #8b95a2;
}
@media (max-width: 700px) {
  .legal-nav {
    height: 74px;
    padding: 0 22px;
  }
  .legal-layout {
    padding: 32px 22px 60px;
    grid-template-columns: minmax(0, 1fr);
    gap: 30px;
  }
  .legal-index {
    flex-direction: row;
    gap: 24px;
  }
  .legal-index > span {
    display: none;
  }
  .legal-document h1 {
    font-size: 28px;
  }
}
</style>
