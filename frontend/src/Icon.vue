<script setup lang="ts">
defineProps<{ name: string }>();
const paths: Record<string, string> = {
  home: 'm3 10 9-7 9 7v11h-6v-7H9v7H3V10Z',
  target: 'M21 12a9 9 0 1 1-9-9 M12 8a4 4 0 1 0 4 4 M12 12l9-9 M17 3h4v4',
  route: 'M5 3v12a4 4 0 0 0 4 4h10 M5 7h10a4 4 0 0 1 4 4v8 M16 16l3 3 3-3',
  process: 'M3 3h6v6H3V3Zm12 12h6v6h-6v-6ZM9 6h9v9M6 9v9h9',
  key: 'M14 10a5 5 0 1 1-4-5 M14 10l7 7-3 3-2-2-2 2-3-3',
  schema: 'M8 3H4v18h4 M16 3h4v18h-4 M8 8l-2 4 2 4 M16 8l2 4-2 4',
  help: 'M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3m.08 4h.01M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0',
  info: 'M12 11v6m0-10h.01M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0',
  sun: 'M12 3V1m0 22v-2M3 12H1m22 0h-2M5 5 3 3m18 18-2-2M5 19l-2 2M21 3l-2 2M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0',
  moon: 'M20.9 13A9 9 0 0 1 11 3.1 9 9 0 1 0 20.9 13Z',
};
</script>
<template>
  <svg
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path :d="paths[name] || paths.info" />
  </svg>
</template>
