import { useLocale } from './useLocale'
export function useStudio() {
  const { locale } = useLocale()
  return { s: (zh: string, en: string) => (locale.value === 'zh-CN' ? zh : en) }
}
