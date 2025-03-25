<template>
    <div class="container px-4 mx-auto">
        <br />
        <div class="flex items-center gap-4 py-8">
            <h1 class="text-2xl font-semibold text-white">Games</h1>
            <div class="relative flex-1 max-w-md">
                <input type="text" v-model="searchQuery" placeholder="Search games..."
                    class="w-full px-4 py-2 text-white bg-gray-700 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-400" />
            </div>
            <button @click="rescanGames" @click.shift="forceRescanGames"
                class="flex items-center gap-2 font-semibold text-white bg-green-800 rounded-lg shadow-lg btn hover:shadow-xl focus:outline-none focus:ring-2 focus:ring-green-400 focus:ring-opacity-75"
                :disabled="isScanning">
                <span v-if="isScanning" class="loading loading-spinner loading-lg"></span>
                {{ isScanning ? "Scanning..." : "Rescan Games" }}
            </button>
        </div>
        <br />
        <div
            class="grid gap-[24px] grid-cols-1 sm:grid-cols-2 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7">
            <GameTitleButton v-for="game in games" :key="game.titleId" :game="game" @get-metadata="getMetadata" />
        </div>
    </div>
</template>

<script setup>
import { ref, onMounted, watch } from "vue";
import GameTitleButton from './GameTitleButton.vue';

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

const forceRescanGames = async () => {
    isScanning.value = true;
    console.log("Force rescan games");
    try {
        const response = await fetch("/admin/rescan?rescan=true", {
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
