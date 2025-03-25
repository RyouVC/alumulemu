<template>
    <button @click="emitGetMetadata"
        class="relative w-full overflow-hidden transition-all shadow-md cursor-pointer rounded-xl hover:shadow-lg aspect-square group">
        <!-- Game Image -->
        <img :src="game.iconUrl" :alt="game.name"
            class="object-cover w-full h-full transition-all duration-300 group-hover:blur-sm group-hover:scale-105" />

        <!-- Hover Overlay -->
        <div
            class="absolute inset-0 flex flex-col items-center justify-center p-4 text-center transition-opacity duration-300 opacity-0 bg-gray-900/80 group-hover:opacity-100 backdrop-blur-sm">
            <h3 class="mb-2 text-xl font-bold text-white">
                {{ game.name }}
            </h3>
            <p class="mb-2 text-gray-300">{{ game.publisher }}</p>
            <p class="text-gray-300">
                Size: {{ formattedSize }}
            </p>
        </div>
    </button>
</template>

<script setup>
import { computed } from 'vue';


const props = defineProps({
    game: {
        type: Object,
        required: true
    }
});

const emit = defineEmits(['get-metadata']);

const formattedSize = computed(() => {
    return props.game.size > 1024 * 1024 * 1024
        ? (props.game.size / (1024 * 1024 * 1024)).toFixed(2) + " GB"
        : (props.game.size / (1024 * 1024)).toFixed(2) + " MB";
});

const emitGetMetadata = () => {
    emit('get-metadata', props.game.titleId);
};
</script>
