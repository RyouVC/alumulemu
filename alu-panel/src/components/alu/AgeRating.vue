<template>
    <div class="flex items-center">
        <div class="badge badge-lg" :class="badgeClass">
            {{ displayText }}
        </div>
    </div>
</template>

<script>
export default {
    props: {
        // Numeric age rating (e.g., 18, 13)
        rating: {
            type: Number,
            default: null
        },
        // Text-based rating (e.g., "PEGI 18", "ESRB M")
        ageRating: {
            type: String,
            default: null
        },
        // Optional size modifier
        size: {
            type: String,
            default: 'lg',
            validator: (value) => ['sm', 'md', 'lg'].includes(value)
        }
    },
    computed: {
        badgeClass() {
            if (this.rating !== null) {
                return {
                    'badge-error': this.rating >= 18,
                    'badge-warning': this.rating >= 13 && this.rating < 18,
                    'badge-info': this.rating >= 10 && this.rating < 13,
                    'badge-success': this.rating < 10,
                    'badge-neutral': this.rating === undefined || this.rating === null,
                    [`badge-${this.size}`]: this.size !== 'lg'
                };
            }
            return 'badge-neutral';
        },
        displayText() {
            // If we have a numeric rating, display as "X+"
            if (this.rating !== null && this.rating !== undefined) {
                return `${this.rating}+`;
            }
            // If we have a text-based rating, display it
            if (this.ageRating) {
                return this.ageRating;
            }
            // Default
            return 'N/A';
        }
    }
};
</script>
