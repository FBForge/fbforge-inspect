import { computed, onUnmounted, ref, watch } from 'vue';
type Appearance = 'system' | 'light' | 'dark';
const key = 'fbforge-inspect-appearance';
export function useTheme() {
  const media = matchMedia('(prefers-color-scheme: dark)');
  const systemDark = ref(media.matches);
  const appearance = ref<Appearance>('system');
  try {
    const saved = localStorage.getItem(key);
    if (saved === 'system' || saved === 'light' || saved === 'dark') appearance.value = saved;
  } catch {
    // Storage is optional; diagnostics and theme selection remain available.
  }
  const dark = computed(
    () => appearance.value === 'dark' || (appearance.value === 'system' && systemDark.value),
  );
  const update = (event: MediaQueryListEvent) => {
    systemDark.value = event.matches;
  };
  media.addEventListener('change', update);
  onUnmounted(() => media.removeEventListener('change', update));
  watch(
    dark,
    (value) => {
      document.documentElement.dataset.theme = value ? 'dark' : 'light';
    },
    { immediate: true },
  );
  watch(appearance, (value) => {
    try {
      localStorage.setItem(key, value);
    } catch {
      /* Optional preference. */
    }
  });
  return { appearance, systemDark };
}
