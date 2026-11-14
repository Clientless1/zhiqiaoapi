import { computed, reactive } from 'vue'
import { getPublicSiteSetting, type SiteSetting } from '../api/settings'
import { resolveApiOrigin } from '../utils/apiOrigin'

const DEFAULT_SITE_NAME = '智桥API'
const DEFAULT_BILLING_CURRENCY = 'CNY' as const

type BillingCurrency = 'USD' | 'CNY'

const brand = reactive({
  loaded: false,
  loading: false,
  siteName: DEFAULT_SITE_NAME,
  logoUrl: '',
  apiOrigin: '',
  billingCurrency: DEFAULT_BILLING_CURRENCY as BillingCurrency
})

let pendingLoad: Promise<void> | null = null

function normalizeBillingCurrency(value?: string | null): BillingCurrency {
  if (value === 'USD') return 'USD'
  return DEFAULT_BILLING_CURRENCY
}

function applyBrand(setting: SiteSetting) {
  brand.siteName = setting.site_name?.trim() || DEFAULT_SITE_NAME
  brand.logoUrl = setting.logo_url?.trim() || ''
  brand.apiOrigin = setting.api_origin?.trim() || ''
  brand.billingCurrency = normalizeBillingCurrency(setting.billing_currency)
  brand.loaded = true
}

async function loadSiteBrand(force = false) {
  if (!force && brand.loaded) return
  if (!force && pendingLoad) return pendingLoad

  brand.loading = true
  pendingLoad = getPublicSiteSetting()
    .then(applyBrand)
    .catch(() => {
      brand.loaded = true
    })
    .finally(() => {
      brand.loading = false
      pendingLoad = null
    })

  return pendingLoad
}

export function setSiteBrand(setting: SiteSetting) {
  applyBrand(setting)
}

export function useSiteBrand() {
  void loadSiteBrand()

  return {
    siteName: computed(() => brand.siteName),
    logoUrl: computed(() => brand.logoUrl),
    apiOrigin: computed(() => resolveApiOrigin(brand.apiOrigin, window.location.origin)),
    billingCurrency: computed(() => brand.billingCurrency),
    loadSiteBrand,
    setSiteBrand
  }
}
