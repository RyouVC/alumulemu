<template>
    <!-- fixed background -->
    <div
        v-if="metadata"
        class="fixed inset-0 h-screen bg-cover bg-center bg-no-repeat blur-xl opacity-70"
        :style="{ backgroundImage: `url(${metadata.bannerUrl})` }"
    ></div>

    <div class="relative z-10">
        <div v-if="metadata" class="w-full">
            <div class="h-[25vh]"></div>

            <div class="relative">
                <img
                    :src="metadata.iconUrl"
                    :alt="metadata.name"
                    class="absolute left-8 -top-24 w-48 h-48 object-cover rounded-lg shadow-xl z-20"
                />

                <!-- Title outside the box -->
                <div class="absolute left-64 -top-24 z-20 text-white">
                    <div class="flex items-center gap-4 mb-4">
                        <h1 class="text-4xl font-bold text-white">
                            {{ metadata.name }}
                        </h1>
                        <h1 class="text-4xl font-bold text-gray-300">
                            {{ metadata.releaseDate.slice(0, 4) }}
                        </h1>
                    </div>
                    <p class="text-xl">
                        {{ metadata.publisher }} |

                        {{
                            metadata.size > 1024 * 1024 * 1024
                                ? (
                                      metadata.size /
                                      (1024 * 1024 * 1024)
                                  ).toFixed(2) + " GB"
                                : (metadata.size / (1024 * 1024)).toFixed(2) +
                                  " MB"
                        }}
                    </p>
                </div>

                <div class="absolute left-64 top-6 z-20 text-white">
                    <p>
                        <strong>Title ID:</strong>
                        {{ metadata.titleId }}
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
                        <strong>Categories:</strong>
                        {{ metadata.category.join(", ") }}
                    </p>
                </div>

                <div class="bg-gray-900 min-h-[70vh] relative pt-8 rounded-lg">
                    <div
                        class="absolute right-8 -top-6 h-12 w-48 overflow-visible z-20"
                    >
                        <button
                            @click="downloadGame(metadata.titleId)"
                            class="absolute inset-0 px-8 py-1 bg-gradient-to-r from-green-600 to-green-800 text-white rounded-lg font-semibold shadow-lg hover:shadow-xl focus:outline-none focus:ring-2 focus:ring-green-400 focus:ring-opacity-75 flex items-center justify-center gap-2"
                        >
                            Download
                        </button>
                    </div>

                    <div class="px-8 pt-20">
                        <div class="text-white">
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
