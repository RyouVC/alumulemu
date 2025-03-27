<template>
    <div class="flex items-center">
        <div class="badge badge-lg" :class="badgeClass">
            {{ displayText }}
        </div>
    </div>
</template>

<script>

function esrb_to_rating(esrb) {

    const ratings = {
        'E': 0,
        'E10+': 10,
        'T': 13,
        'M': 17,
        'AO': 18
    };
    return ratings[esrb] || null;
}

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
            // Get effective rating - either directly from props or derived from ESRB
            let effectiveRating = this.rating;

            // If no direct rating but we have an age rating string, try to convert it
            if (effectiveRating === null && this.ageRating) {
                effectiveRating = esrb_to_rating(this.ageRating);
            }

            if (effectiveRating !== null) {
                return {
                    'badge-error': effectiveRating >= 18,
                    'badge-warning': effectiveRating >= 13 && effectiveRating < 18,
                    'badge-info': effectiveRating >= 10 && effectiveRating < 13,
                    'badge-success': effectiveRating < 10,
                    'badge-neutral': effectiveRating === undefined || effectiveRating === null,
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
                const numericRating = esrb_to_rating(this.ageRating);
                return numericRating !== null ? `${numericRating}+` : this.ageRating;
            }
            // Default
            return 'N/A';
        }
    }
};
</script>
