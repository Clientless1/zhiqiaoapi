import { createRouter, createWebHistory, createWebHashHistory, loadRouteLocation } from 'vue-router'
import { getAdminServicePolicy, getSetupStatus, getUserServicePolicy } from '../api/policy'
import type { ServiceMode } from '../api/policy'
import { useAuthStore } from '../stores/auth'
import { getNavigationSetting, isUserPathVisible } from '../api/navigation'
import { scrollBehavior } from './scroll'
import {
  anthropicSubSections,
  openAiSubSections
} from '../views/public/interfaces/interfacesSections'

declare module 'vue-router' {
  interface RouteMeta {
    allowedServiceModes?: ServiceMode[]
    serviceModeRedirect?: string
  }
}

export const router = createRouter({
  history:
    import.meta.env.VITE_DESIGN_PREVIEW === 'true'
      ? createWebHashHistory(import.meta.env.BASE_URL)
      : createWebHistory(),
  scrollBehavior,
  routes: [
    { path: '/terms', name: 'terms', component: () => import('../views/public/LegalView.vue') },
    { path: '/privacy', name: 'privacy', component: () => import('../views/public/LegalView.vue') },
    {
      path: '/playground',
      component: () => import('../views/shared/PlaygroundView.vue'),
      meta: { messageKey: 'studioPlayground' }
    },
    {
      path: '/status',
      redirect: '/models'
    },
    {
      path: '/setup',
      name: 'setup',
      component: () => import('../views/setup/SetupView.vue'),
      meta: { messageKey: 'setup' }
    },
    {
      path: '/',
      name: 'home',
      component: () => import('../views/public/AuroraHomeView.vue'),
      meta: { messageKey: 'home' }
    },
    {
      path: '/models',
      name: 'modelPlaza',
      component: () => import('../views/shared/ModelPlazaView.vue'),
      meta: { messageKey: 'modelPlaza' }
    },
    {
      path: '/docs',
      name: 'docs',
      component: () => import('../views/public/DocsView.vue'),
      meta: { messageKey: 'docs' }
    },
    {
      path: '/about',
      name: 'about',
      component: () => import('../views/public/AboutView.vue'),
      meta: { messageKey: 'aboutPage' }
    },
    {
      path: '/payment/return',
      name: 'paymentReturn',
      component: () => import('../views/public/PaymentReturnView.vue'),
      meta: { messageKey: 'paymentSettings', user: true }
    },
    {
      path: '/interfaces',
      name: 'interfaces',
      redirect: '/interfaces/before-start'
    },
    {
      path: '/interfaces/before-start',
      name: 'interfacesBeforeStart',
      component: () => import('../views/public/InterfacesView.vue'),
      props: { section: 'before-start' },
      meta: { messageKey: 'interfaces' }
    },
    {
      path: `/interfaces/openai/:sub(${openAiSubSections.join('|')})?`,
      name: 'interfacesOpenAi',
      component: () => import('../views/public/InterfacesView.vue'),
      props: (route: { params: { sub?: string } }) => ({
        section: 'openai',
        sub: route.params.sub
      }),
      meta: { messageKey: 'interfaces' }
    },
    {
      path: `/interfaces/anthropic/:sub(${anthropicSubSections.join('|')})?`,
      name: 'interfacesAnthropic',
      component: () => import('../views/public/InterfacesView.vue'),
      props: (route: { params: { sub?: string } }) => ({
        section: 'anthropic',
        sub: route.params.sub
      }),
      meta: { messageKey: 'interfaces' }
    },
    {
      path: '/interfaces/errors',
      name: 'interfacesErrors',
      component: () => import('../views/public/InterfacesView.vue'),
      props: { section: 'errors' },
      meta: { messageKey: 'interfaces' }
    },
    {
      path: '/interfaces/billing',
      name: 'interfacesBilling',
      component: () => import('../views/public/InterfacesView.vue'),
      props: { section: 'billing' },
      meta: { messageKey: 'interfaces' }
    },
    {
      path: '/login',
      name: 'login',
      component: () => import('../views/auth/LoginView.vue'),
      meta: { messageKey: 'signIn' }
    },
    {
      path: '/register',
      name: 'register',
      component: () => import('../views/auth/LoginView.vue'),
      meta: { messageKey: 'registrationPage' }
    },
    {
      path: '/oauth/callback',
      name: 'oauthCallback',
      component: () => import('../views/auth/OAuthCallbackView.vue'),
      meta: { messageKey: 'signIn' }
    },
    {
      path: '/forgot-password',
      name: 'forgotPassword',
      component: () => import('../views/auth/ForgotPasswordView.vue'),
      meta: { messageKey: 'forgotPassword' }
    },
    {
      path: '/reset-password',
      name: 'resetPassword',
      component: () => import('../views/auth/ResetPasswordView.vue'),
      meta: { messageKey: 'resetPassword' }
    },
    {
      path: '/change-password',
      name: 'changePassword',
      component: () => import('../views/auth/ChangePasswordView.vue'),
      meta: { messageKey: 'changePassword', passwordChange: true }
    },
    {
      path: '/home',
      component: () => import('../layouts/UserLayout.vue'),
      meta: { user: true },
      children: [
        {
          path: 'playground',
          component: () => import('../views/shared/PlaygroundView.vue'),
          meta: { messageKey: 'studioPlayground', subtitleKey: 'studioPlaygroundHint' }
        },
        {
          path: 'status',
          redirect: '/home/models'
        },
        { path: '', redirect: '/home/overview' },
        {
          path: 'models',
          name: 'userModels',
          component: () => import('../views/shared/ModelPlazaView.vue'),
          meta: { messageKey: 'modelPlaza', subtitleKey: 'modelPlazaSubtitle' }
        },
        {
          path: 'overview',
          name: 'userOverview',
          component: () => import('../views/user/OverviewView.vue'),
          meta: { messageKey: 'userDashboard', subtitleKey: 'userDashboardSubtitle' }
        },
        {
          path: 'packages',
          name: 'userPackages',
          component: () => import('../views/user/PackagesView.vue'),
          meta: { messageKey: 'userPlans' }
        },
        {
          path: 'coupons',
          name: 'userCoupons',
          component: () => import('../views/user/CouponsView.vue'),
          meta: { messageKey: 'userCoupons' }
        },
        {
          path: 'redeem',
          name: 'userRedeem',
          component: () => import('../views/user/RedeemView.vue'),
          meta: { messageKey: 'userRedeem' }
        },
        {
          path: 'orders',
          name: 'userOrders',
          component: () => import('../views/user/OrdersView.vue'),
          meta: { messageKey: 'userOrders' }
        },
        {
          path: 'notifications',
          name: 'userNotifications',
          component: () => import('../views/user/NotificationsView.vue'),
          meta: { messageKey: 'userNotifications' }
        },
        {
          path: 'ledger',
          name: 'userLedger',
          component: () => import('../views/user/LedgerView.vue'),
          meta: { messageKey: 'userLedger' }
        },
        {
          path: 'apikeys',
          name: 'userApiKeys',
          component: () => import('../views/user/ApiKeysView.vue'),
          meta: { messageKey: 'apiKey', subtitleKey: 'apiKeySubtitle' }
        },
        {
          path: 'recharge',
          name: 'userRecharge',
          component: () => import('../views/user/RechargeView.vue'),
          meta: { messageKey: 'recharge', subtitleKey: 'rechargeSubtitle' }
        },
        {
          path: 'usage',
          name: 'userUsage',
          component: () => import('../views/user/UsageView.vue'),
          meta: { messageKey: 'usage', subtitleKey: 'usageSubtitle' }
        },
        {
          path: 'settings',
          name: 'userSettings',
          component: () => import('../views/user/SettingsView.vue'),
          meta: { messageKey: 'personalSettings', subtitleKey: 'personalSettingsSubtitle' }
        }
      ]
    },
    {
      path: '/admin',
      component: () => import('../layouts/AdminLayout.vue'),
      meta: { admin: true },
      children: [
        {
          path: 'settings/navigation',
          name: 'navigationSettings',
          component: () => import('../views/admin/NavigationSettingsView.vue'),
          meta: { messageKey: 'userNavigationSettings' }
        },
        {
          path: 'settings/auth',
          name: 'authSettings',
          component: () => import('../views/admin/AuthSettingsView.vue'),
          meta: { messageKey: 'authSettings' }
        },
        {
          path: 'playground',
          component: () => import('../views/shared/PlaygroundView.vue'),
          meta: { messageKey: 'studioPlayground', subtitleKey: 'studioPlaygroundHint' }
        },
        {
          path: 'status',
          redirect: '/admin/models'
        },
        { path: '', redirect: '/admin/overview' },
        {
          path: 'overview',
          component: () => import('../views/admin/OverviewView.vue'),
          meta: { messageKey: 'studioOverview', subtitleKey: 'studioOverviewHint' }
        },
        {
          path: 'user-operations',
          name: 'userOperations',
          component: () => import('../views/admin/UserOperationsView.vue'),
          meta: { messageKey: 'userOperations', subtitleKey: 'userOperationsSubtitle' }
        },
        {
          path: 'models',
          name: 'adminModels',
          component: () => import('../views/shared/ModelPlazaView.vue'),
          meta: { messageKey: 'modelPlaza', subtitleKey: 'modelPlazaSubtitle' }
        },
        {
          path: 'supply-chain',
          name: 'supplyChain',
          component: () => import('../views/admin/SupplyChainView.vue'),
          meta: { messageKey: 'supplyChain', subtitleKey: 'supplyChainSubtitle' }
        },
        {
          path: 'apps',
          name: 'apps',
          component: () => import('../views/admin/AppsView.vue'),
          meta: {
            messageKey: 'apps',
            subtitleKey: 'adminAppsSubtitle',
            allowedServiceModes: ['internal'],
            serviceModeRedirect: '/admin/channels'
          }
        },
        {
          path: 'credentials',
          name: 'credentials',
          component: () => import('../views/admin/CredentialsView.vue'),
          meta: {
            messageKey: 'credentialManagement',
            subtitleKey: 'adminCredentialsSubtitle',
            allowedServiceModes: ['paid'],
            serviceModeRedirect: '/admin/channels'
          }
        },
        { path: 'credentials/openai', redirect: '/admin/credentials' },
        {
          path: 'keys',
          name: 'keys',
          component: () => import('../views/admin/UsersView.vue'),
          meta: { messageKey: 'userManagement', subtitleKey: 'adminUsersSubtitle' }
        },
        {
          path: 'projects',
          name: 'projects',
          component: () => import('../views/admin/ProjectsView.vue'),
          meta: {
            messageKey: 'projectManagement',
            subtitleKey: 'adminProjectsSubtitle',
            allowedServiceModes: ['internal'],
            serviceModeRedirect: '/admin/keys'
          }
        },
        {
          path: 'channels',
          name: 'upstreamChannels',
          component: () => import('../views/admin/ChannelsView.vue'),
          meta: { messageKey: 'upstreamChannels', subtitleKey: 'adminChannelsSubtitle' }
        },
        { path: 'prices', redirect: '/admin/channels' },
        {
          path: 'usage',
          name: 'usage',
          component: () => import('../views/admin/UsageView.vue'),
          meta: { messageKey: 'usage', subtitleKey: 'adminUsageSubtitle' }
        },
        {
          path: 'statistics',
          name: 'statistics',
          component: () => import('../views/admin/StatisticsView.vue'),
          meta: {
            messageKey: 'consumptionOverview',
            subtitleKey: 'adminConsumptionOverviewSubtitle'
          }
        },
        {
          path: 'cost-attribution',
          name: 'costAttribution',
          component: () => import('../views/admin/CostAttributionView.vue'),
          meta: {
            messageKey: 'costAttribution',
            subtitleKey: 'adminCostAttributionSubtitle',
            allowedServiceModes: ['internal'],
            serviceModeRedirect: '/admin/statistics'
          }
        },
        { path: 'settings', redirect: '/admin/settings/pricing-policies' },
        {
          path: 'settings/pricing-policies',
          name: 'pricingPolicy',
          component: () => import('../views/admin/PricingSettingsView.vue'),
          meta: { messageKey: 'pricingPolicy', subtitleKey: 'adminPricingSubtitle' }
        },
        {
          path: 'settings/smtp',
          name: 'smtpSettings',
          component: () => import('../views/admin/SmtpSettingsView.vue'),
          meta: { messageKey: 'smtpSettings', subtitleKey: 'adminSmtpSubtitle' }
        },
        {
          path: 'settings/site',
          name: 'siteSettings',
          component: () => import('../views/admin/SiteSettingsView.vue'),
          meta: { messageKey: 'siteSettingsPage', subtitleKey: 'adminSiteSettingsSubtitle' }
        },
        {
          path: 'settings/payment',
          name: 'paymentSettings',
          component: () => import('../views/admin/PaymentSettingsView.vue'),
          meta: {
            messageKey: 'paymentSettings',
            subtitleKey: 'adminPaymentSubtitle',
            allowedServiceModes: ['paid'],
            serviceModeRedirect: '/admin/settings/pricing-policies'
          }
        },
        {
          path: 'settings/other',
          name: 'otherSettings',
          component: () => import('../views/admin/OtherSettingsView.vue'),
          meta: { messageKey: 'otherSettings', subtitleKey: 'adminOtherSettingsSubtitle' }
        },
        {
          path: 'settings/admin-password',
          name: 'adminPasswordSettings',
          component: () => import('../views/admin/PasswordSettingsView.vue'),
          meta: { messageKey: 'adminPasswordSettings', subtitleKey: 'adminPasswordSubtitle' }
        }
      ]
    },
    { path: '/:pathMatch(.*)*', redirect: '/' }
  ]
})

let setupCompleted = false

// Legacy subsection anchors (/interfaces/openai#openai-text) now live on their
// own pages; redirect them before any other navigation guard runs.
router.beforeEach((to) => {
  for (const prefix of ['openai', 'anthropic'] as const) {
    const base = `/interfaces/${prefix}`
    if (to.path === base && to.hash.startsWith(`#${prefix}-`)) {
      return { path: `${base}/${to.hash.slice(prefix.length + 2)}`, replace: true }
    }
  }
})

router.beforeEach(async (to) => {
  const auth = useAuthStore()
  // Start loading this page's code while setup/session checks are in flight.
  // This only imports code: the guard below still decides whether to enter it.
  void loadRouteLocation(to).catch(() => undefined)
  const setup =
    setupCompleted && to.name !== 'setup' ? null : await getSetupStatus().catch(() => null)
  if (setup?.setup_completed) setupCompleted = true

  if (setup && !setup.setup_completed && to.name !== 'setup') {
    return {
      name: 'setup',
      query: { redirect: to.fullPath }
    }
  }

  if (setup?.setup_completed && to.name === 'setup') {
    return {
      name: 'login'
    }
  }

  const requiresAuth = to.meta.admin === true || to.meta.user === true
  if (requiresAuth && !auth.isAuthed) {
    return {
      name: 'login',
      query: { redirect: to.fullPath }
    }
  }

  if (requiresAuth && !(await auth.verifySession())) {
    return {
      name: 'login',
      query: { redirect: to.fullPath }
    }
  }

  if (
    auth.isUser &&
    auth.requiresPasswordChange &&
    to.name !== 'changePassword' &&
    to.name !== 'login'
  ) {
    return {
      name: 'changePassword',
      query: { redirect: to.fullPath }
    }
  }

  if (
    to.name === 'changePassword' &&
    (!auth.isAuthed || !(await auth.verifySession()) || !auth.isUser)
  ) {
    return {
      name: 'login',
      query: { redirect: to.fullPath }
    }
  }

  if (to.name === 'changePassword' && auth.isUser && !auth.requiresPasswordChange) {
    return '/home/overview'
  }

  if (to.meta.admin === true && !auth.isAdmin) {
    return '/home'
  }

  if (to.meta.user === true && auth.isAdmin) {
    return '/admin'
  }

  if (to.meta.allowedServiceModes?.length && to.meta.serviceModeRedirect) {
    const servicePolicy = await getAdminServicePolicy().catch(() => null)
    if (servicePolicy && !to.meta.allowedServiceModes.includes(servicePolicy.service_mode)) {
      return to.meta.serviceModeRedirect
    }
  }

  if (to.meta.user === true && auth.isUser) {
    const navigation = await getNavigationSetting().catch(() => null)
    if (navigation && !isUserPathVisible(to.path, navigation)) return '/home/overview'
  }

  if (to.name === 'userRecharge') {
    const servicePolicy = await getUserServicePolicy().catch(() => null)
    if (servicePolicy && !servicePolicy.recharge_enabled) {
      return '/home/overview'
    }
  }

  if (
    (to.name === 'login' || to.name === 'register') &&
    auth.isAuthed &&
    (await auth.verifySession())
  ) {
    if (auth.isUser && auth.requiresPasswordChange) return '/change-password'
    return auth.isAdmin ? '/admin' : '/home/overview'
  }
})
