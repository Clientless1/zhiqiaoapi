export function businessDays(businessDate: string, count = 30): string[] {
  const end = new Date(`${businessDate}T00:00:00Z`)
  if (!Number.isFinite(end.getTime())) return []
  return Array.from({ length: count }, (_, index) => {
    const date = new Date(end)
    date.setUTCDate(date.getUTCDate() - count + 1 + index)
    return date.toISOString().slice(0, 10)
  })
}
