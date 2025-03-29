<template>
    <button :class="buttonClasses" :disabled="disabled" @click="handleClick" type="button">
        <slot>Button</slot>
    </button>
</template>

<script>
export default {
    name: 'AluButton',
    emits: ['click'],
    props: {
        level: {
            type: String,
            default: 'primary',
            validator: (value) => ['primary', 'secondary', 'danger', 'success'].includes(value)
        },
        variant: {
            type: String,
            default: 'solid',
            validator: (value) => ['solid', 'soft', 'outline', 'dash'].includes(value)
        },
        size: {
            type: String,
            default: 'medium',
            // validator: (value) => ['small', 'medium', 'large'].includes(value)
        },
        disabled: {
            type: Boolean,
            default: false
        },
        fullWidth: {
            type: Boolean,
            default: false
        }
    },
    methods: {
        handleClick(event) {
            // Only emit 'click' event with the original event object
            // Parent components can check event.shiftKey if needed
            this.$emit('click', event);
            console.log('Button clicked', event);
        }
    },
    computed: {
        buttonClasses() {
            const baseClasses = 'btn focus:outline-none focus:ring-2 focus:ring-opacity-75 hover:shadow-xl';

            // Level styles (previously variant)
            const levelClasses = {
                primary: 'btn-primary',
                secondary: 'btn-secondary',
                danger: 'btn-error',
                success: 'btn-success'
            };

            // Color mappings based on level
            const colorMap = {
                primary: { bg: 'blue', text: 'blue', ring: 'blue' },
                secondary: { bg: 'purple', text: 'purple', ring: 'purple' },
                danger: { bg: 'red', text: 'red', ring: 'red' },
                success: { bg: 'green', text: 'green', ring: 'green' }
            };

            // Get the color set for the current level
            const colors = colorMap[this.level];

            // Variant styles
            let variantClass = '';
            switch (this.variant) {
                case 'soft':
                    variantClass = 'btn-soft';
                    break;
                case 'outline':
                    variantClass = 'btn-outline';
                    break;
                case 'dash':
                    variantClass = 'btn-outline border-dashed';
                    break;
                default: // solid
                    variantClass = '';
                    break;
            }

            // Size styles
            const sizeClasses = {
                small: 'h-8 px-4 py-0.5 mt-2 text-sm w-36',
                medium: 'h-12 px-8 py-1 mt-3 w-48',
                large: 'h-14 px-10 py-2 mt-4 text-lg w-56'
            };

            // Text color for solid variant should be white
            const textColorClass = this.variant === 'solid' ? '' : '';

            // Full width overrides width set by size
            const widthClass = this.fullWidth ? 'w-full' : '';

            // Disabled state
            const disabledClass = this.disabled ? 'opacity-60 cursor-not-allowed' : '';

            return `${baseClasses} ${levelClasses[this.level]} ${variantClass} ${sizeClasses[this.size]} ${textColorClass} ${widthClass} ${disabledClass}`;
        }
    }
}
</script>
