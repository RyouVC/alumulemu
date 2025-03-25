<template>
    <button 
        @click="$emit('click')"
        class="flex items-center justify-center w-48 h-12 gap-2 px-8 py-1 mt-3 font-semibold text-white shadow-lg btn bg-gradient-to-r from-green-600 to-green-800 hover:shadow-xl focus:outline-none focus:ring-2 focus:ring-green-400 focus:ring-opacity-75"
    >
        <slot>Button</slot>
    </button>
</template>

<script>
export default {
    name: 'AluButton',
    emits: ['click']
}
</script>
