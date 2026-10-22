import { ref, watch } from 'vue'
import { useAsyncData } from './useAsyncData'
import type { PageQuery, CommercePage } from '../api/commerce'

export function useCommercePage<T, P extends CommercePage<T>>(
  loader: (query: PageQuery) => Promise<P>,
  initial: P
) {
  const page = ref(1)
  const pageSize = ref(20)
  const filter = ref('all')
  const state = useAsyncData(
    () => loader({ page: page.value, page_size: pageSize.value, type: filter.value }),
    initial
  )
  watch(page, () => void state.reload())
  watch([pageSize, filter], () => {
    if (page.value !== 1) page.value = 1
    else void state.reload()
  })
  watch(state.data, (value) => {
    const last = Math.max(1, Math.ceil(value.total / pageSize.value))
    if (page.value > last) page.value = last
  })
  async function refresh() {
    if (page.value !== 1) page.value = 1
    else await state.reload()
  }
  return { ...state, page, pageSize, filter, refresh }
}
