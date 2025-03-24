<template>
    <!-- this is the background -->
    <div
        v-if="metadata"
        class="fixed inset-0 bg-cover bg-center bg-no-repeat blur-xl opacity-10"
        :style="{ backgroundImage: `url(${metadata.bannerUrl})` }"
    ></div>
    <div class="relative z-10">
        <br />
        <br />
        <br />
        <!-- what the hell is this -->
        <div v-if="metadata" class="container mx-auto px-4">
            <div class="flex flex-col gap-4">
                <div class="flex space-x-4">
                    <h1 class="text-2xl font-bold text-white">
                        {{ metadata.name }}
                    </h1>
                    <h1 class="text-2xl font-bold text-gray-600">&nbsp;</h1>
                    <h1 class="text-2xl font-bold text-gray-600">
                        {{ metadata.releaseDate.slice(0, 4) }}
                    </h1>
                </div>
                <div class="flex gap-8">
                    <div class="flex flex-col gap-4">
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
                    </div>
                    <div class="text-white">
                        <p><strong>Title ID:</strong> {{ metadata.titleId }}</p>
                        <p>
                            <strong>Publisher:</strong> {{ metadata.publisher }}
                        </p>
                        <p>
                            <strong>Release Date:</strong>
                            {{
                                new Date(
                                    metadata.releaseDate.replace(
                                        /(\d{4})(\d{2})(\d{2})/,
                                        "$1-$2-$3",
                                    ),
                                ).toLocaleDateString()
                            }}
                        </p>
                        <p>
                            <strong>Size:</strong>
                            {{
                                metadata.size > 1024 * 1024 * 1024
                                    ? (
                                          metadata.size /
                                          (1024 * 1024 * 1024)
                                      ).toFixed(2) + " GB"
                                    : (metadata.size / (1024 * 1024)).toFixed(
                                          2,
                                      ) + " MB"
                            }}
                        </p>
                        <p>
                            <strong>Categories:</strong>
                            {{ metadata.category.join(", ") }}
                        </p>
                        <div class="mt-4">
                            <hr class="border-gray-800 my-4" />
                            <h2 class="text-xl font-semibold mb-2">
                                Description
                            </h2>
                            <p class="whitespace-pre-line">
                                {{ metadata.description }}
                            </p>
                        </div>
                    </div>
                </div>
            </div>
        </div>
        <div v-else class="text-white">Loading...</div>
    </div>
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
