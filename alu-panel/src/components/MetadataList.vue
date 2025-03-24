<template>
    <br />
    <br />
    <br />
    <!-- what the hell is this -->
    <div v-if="metadata" class="container mx-auto px-4">
        <div class="flex flex-col gap-4">
            <h1 class="text-2xl font-bold text-white">{{ metadata.name }}</h1>
            <img
                :src="metadata.iconUrl"
                :alt="metadata.name"
                class="w-48 h-48 object-cover rounded-lg"
            />
            <button
                @click="downloadGame(metadata.titleId)"
                class="w-48 px-8 py-2 bg-gradient-to-r from-green-600 to-green-800 text-white rounded-lg font-semibold shadow-lg hover:shadow-xl focus:outline-none focus:ring-2 focus:ring-green-400 focus:ring-opacity-75 flex items-center justify-center gap-2"
            >
                Download
            </button>
            <div class="text-white">
                <p><strong>Title ID:</strong> {{ metadata.titleId }}</p>
                <p><strong>Publisher:</strong> {{ metadata.publisher }}</p>
                <p><strong>Release Date:</strong> {{ metadata.releaseDate }}</p>
                <p>
                    <strong>Size:</strong>
                    {{ (metadata.size / (1024 * 1024)).toFixed(2) }} MB
                </p>
                <p>
                    <strong>Categories:</strong>
                    {{ metadata.category.join(", ") }}
                </p>
                <div class="mt-4">
                    <br />
                    <h2 class="text-xl font-semibold mb-2">Description</h2>
                    <p class="whitespace-pre-line">
                        {{ metadata.description }}
                    </p>
                </div>
            </div>
        </div>
    </div>
    <div v-else class="text-white">Loading...</div>
</template>

<script>
export default {
    data() {
        return {
            titleId: null,
            metadata: null,
        };
    },
    methods: {
        async downloadGame(titleId) {
            try {
                window.location.href = `/api/get_game/${titleId}`;
            } catch (error) {
                alert("Error downloading game: " + error);
            }
        },
    },
    async created() {
        this.titleId = this.$route.query.tid;
        if (this.titleId) {
            try {
                const response = await fetch(`/api/title_meta/${this.titleId}`);
                if (!response.ok) {
                    throw new Error("Failed to fetch metadata");
                }
                this.metadata = await response.json();
            } catch (error) {
                console.error("Error fetching metadata:", error);
            }
        }
    },
};
</script>
