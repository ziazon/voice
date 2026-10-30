<script setup lang="ts">
import type { ConvenePanelRequest } from '../../types/generated/ConvenePanelRequest'
defineProps<{ id: string | null, start: ConvenePanelRequest | null }>()
defineEmits<{ running: [boolean], back: [] }>()
</script>

<template><div /></template>
