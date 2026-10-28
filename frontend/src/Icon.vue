<script setup lang="ts">
defineProps<{ name: string }>();
const paths: Record<string, string> = {
  home: 'm3 10 9-7 9 7v11h-6v-7H9v7H3z',
  target: 'M21 12a9 9 0 1 1-9-9 M12 8a4 4 0 1 0 4 4 M12 12l9-9 M17 3h4v4',
  route: 'M5 3v12a4 4 0 0 0 4 4h10 M5 7h10a4 4 0 0 1 4 4v8 M16 16l3 3 3-3',
  process: 'M3 3h7v7H3z M14 14h7v7h-7z M7 14v4h4 M14 7h4v4',
  key: 'M14 10a5 5 0 1 1-4-5 M14 10l7 7-3 3-2-2-2 2-3-3',
  schema: 'M8 3H4v18h4 M16 3h4v18h-4 M8 8l-2 4 2 4 M16 8l2 4-2 4',
  help: 'M9 8a3 3 0 1 1 4 3c-1 1-1 2-1 3 M12 17v1 M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0',
  info: 'M12 11v6 M12 7v1 M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0',
  sun: 'M12 3v2 M12 19v2 M3 12h2 M19 12h2 M5 5l2 2 M17 17l2 2 M5 19l2-2 M17 7l2-2 M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0',
  arrow: 'M5 12h14 M14 7l5 5-5 5',
};
</script>
<template>
  <svg
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.6"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path :d="paths[name] || paths.info" />
  </svg>
</template>
