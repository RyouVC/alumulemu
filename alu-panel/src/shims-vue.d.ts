declare module 'vue' {
    import { DefineComponent } from 'vue'

    export const createApp: any
    export const defineComponent: any
    export const ref: any
    export const reactive: any
    export const computed: any
    export const watch: any
    export const onMounted: any
    export const onUnmounted: any
    export const nextTick: any
    export type { DefineComponent }
}

declare module '*.vue';