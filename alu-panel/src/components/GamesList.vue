<template>
    <div class="container mx-auto px-4">
        <br />
        <div class="flex items-center gap-4 py-8">
            <h1 class="text-white text-2xl font-semibold">Games</h1>
            <div class="relative flex-1 max-w-md">
                <input
                    type="text"
                    v-model="searchQuery"
                    placeholder="Search games..."
                    class="w-full px-4 py-2 bg-gray-700 text-white rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-400"
                />
            </div>
            <button
                @click="rescanGames"
                class="px-8 py-2 bg-gradient-to-r from-green-600 to-green-800 text-white rounded-lg font-semibold shadow-lg hover:shadow-xl focus:outline-none focus:ring-2 focus:ring-green-400 focus:ring-opacity-75 flex items-center gap-2"
                :disabled="isScanning"
            >
                <svg
                    v-if="isScanning"
                    class="animate-spin h-5 w-5 text-white"
                    xmlns="http://www.w3.org/2000/svg"
                    fill="none"
                    viewBox="0 0 24 24"
                >
                    <circle
                        class="opacity-25"
                        cx="12"
                        cy="12"
                        r="10"
                        stroke="currentColor"
                        stroke-width="4"
                    ></circle>
                    <path
                        class="opacity-75"
                        fill="currentColor"
                        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                    ></path>
                </svg>
                {{ isScanning ? "Scanning..." : "Rescan Games" }}
            </button>
        </div>
        <br />
        <div
            class="grid gap-[24px] grid-cols-1 sm:grid-cols-2 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7"
        >
            <button
                v-for="game in games"
                :key="game.titleId"
                @click="getMetadata(game.titleId)"
                class="relative rounded-xl shadow-md hover:shadow-lg transition-all w-full aspect-square overflow-hidden group"
            >
                <!-- Game Image -->
                <img
                    :src="game.iconUrl"
                    :alt="game.name"
                    class="w-full h-full object-cover"
                />

                <!-- Hover Overlay -->
                <div
                    class="absolute inset-0 bg-blue-900/80 opacity-0 group-hover:opacity-100 transition-opacity duration-300 flex flex-col items-center justify-center text-center p-4"
                >
                    <h3 class="text-xl font-bold text-white mb-2">
                        {{ game.name }}
                    </h3>
                    <p class="text-gray-300 mb-2">{{ game.publisher }}</p>
                    <p class="text-gray-300">
                        Size:
                        {{
                            game.size > 1024 * 1024 * 1024
                                ? (game.size / (1024 * 1024 * 1024)).toFixed(
                                      2,
                                  ) + " GB"
                                : (game.size / (1024 * 1024)).toFixed(2) + " MB"
                        }}
                    </p>
                </div>
            </button>
        </div>
    </div>
</template>

<script setup>
import { ref, onMounted, watch } from "vue";
const isScanning = ref(false);
const games = ref([]);
const searchQuery = ref("");

const loadingError = ref(null);

let searchTimeout = null;

const loadGames = async () => {
    try {
        loadingError.value = null;

        let url = "/api/base_games";

        if (searchQuery.value.trim()) {
            url = `/api/base_games/search?q=${encodeURIComponent(searchQuery.value.trim())}`;
        }

        console.log("Fetching from:", url);

        const response = await fetch(url);

        if (!response.ok) {
            throw new Error(
                `Failed to fetch games: ${response.status} ${response.statusText}`,
            );
        }

        const data = await response.json();
        console.log("Response data:", data);

        if (Array.isArray(data)) {
            games.value = data;
        } else if (data && Array.isArray(data.results)) {
            games.value = data.results;
        } else {
            games.value = [];
            console.warn("API didn't return an array:", data);
        }
    } catch (error) {
        console.error("Error loading games:", error);
        loadingError.value = error.message;
        games.value = [];
    }
};

watch(searchQuery, () => {
    if (searchTimeout) {
        clearTimeout(searchTimeout);
    }

    searchTimeout = setTimeout(() => {
        loadGames();
    }, 300);
});

const rescanGames = async () => {
    isScanning.value = true;
    try {
        const response = await fetch("/admin/rescan", {
            method: "POST",
            credentials: "include",
            headers: {
                "Content-Type": "application/json",
            },
        });

        if (!response.ok) {
            throw new Error("Rescan failed");
        }
        await loadGames();
    } catch (error) {
        console.log(
            "%c YOUR ADMIN PANEL SUCKS",
            `
        font-weight: bold;
        font-size: 72px;
        background: linear-gradient(90deg, red, orange, yellow, green, blue, indigo, violet);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        text-shadow:
          1px 1px 0 #ff0000,
          2px 2px 0 #ff7f00,
          3px 3px 0 #ffff00,
          4px 4px 0 #00ff00,
          5px 5px 0 #0000ff,
          6px 6px 0 #4b0082,
          7px 7px 0 #8f00ff;
      `,
        );
        console.error("Error:", error);
    } finally {
        isScanning.value = false;
    }
};

const getMetadata = async (titleId) => {
    try {
        window.location.href = `/metadata?tid=${titleId}`;
    } catch (error) {
        alert("Error finding metadata for game: " + error);
    }
};

onMounted(() => {
    loadGames();
});
</script>
