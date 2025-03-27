<template>
    <li @click="emitGetMetadata"
        class="flex items-center p-4 transition-colors rounded-lg cursor-pointer hover:bg-base-200">
        <!-- Game icon -->
        <div class="mr-4">
            <slot name="leading">
                <img v-if="game.iconUrl" :src="game.iconUrl" :alt="game.name"
                    class="object-cover w-48 h-48 rounded-md" />
            </slot>
        </div>
        <!-- Game info -->
        <div class="flex flex-col self-start flex-1 px-5" id="game-info">
            <slot name="title">
                <div class="text-2xl font-bold">{{ game.name }}</div>
            </slot>

            <slot name="subtitle">
                <div class="text-lg opacity-70">{{ game.publisher }}</div>
            </slot>
            <div class="flex py-2 mt-2 mb-2 space-x-4" id="game-horizontal-badgelist">
                <slot>
                    <div class="px-2 text-base opacity-60">
                        {{ formattedSize }}
                    </div>
                </slot>
                <AgeRating :rating="game.rating" :age-rating="game.ageRating" class="px-2" />
            </div>

            <div id="game-intro" class="pt-2 text-lg">
                {{ game.intro }}
            </div>
        </div>

        <!-- Actions -->
        <div class="relative ml-4">
            <slot name="actions">
                <div class="relative">
                    <details class="dropdown dropdown-end" ref="dropdownMenu">
                        <summary class="btn btn-primary btn-lg">
                            <!-- Show loading spinner when importing -->
                            <span v-if="isImporting" class="loading loading-spinner loading-md"></span>
                            <span v-else>Import</span>
                        </summary>
                        <ul class="menu dropdown-content bg-base-100 rounded-box z-[1] w-52 p-2 shadow-md">
                            <li v-for="(label, key) in importers" :key="key">
                                <a @click.stop="handleImportOption(key)"
                                    :class="{ 'opacity-50 cursor-not-allowed': isImporting }" :disabled="isImporting">
                                    {{ label }}
                                </a>
                            </li>
                        </ul>
                    </details>
                </div>
            </slot>
        </div>
    </li>
    <Teleport to="body">
        <!-- Upload modal -->
        <div v-if="isUploadPopoverOpen" class="fixed inset-0 z-[9999] flex items-center justify-center bg-black/50"
            @click.self="closeUploadPopover">
            <div class="p-6 rounded-lg shadow-xl bg-base-100 w-96 max-w-[90vw]">
                <h3 class="mb-4 text-xl font-bold">Upload Game File</h3>
                <div class="pt-4">
                    <input type="file" class="w-full file-input file-input-bordered" @change="handleFileSelected" />
                </div>
                <div class="flex justify-end gap-2 mt-6">
                    <AluButton @click="closeUploadPopover" size="small">Cancel</AluButton>
                    <AluButton level="primary" :disabled="!selectedFile" @click="uploadSelectedFile" size="small">Upload
                    </AluButton>
                </div>
            </div>
        </div>

        <!-- URL download modal -->
        <div v-if="isUrlDialogOpen" class="fixed inset-0 z-[9999] flex items-center justify-center bg-black/50"
            @click.self="closeUrlDialog">
            <div class="p-6 rounded-lg shadow-xl bg-base-100 w-96 max-w-[90vw]">
                <h3 class="mb-4 text-xl font-bold">Download from URL</h3>
                <div class="pt-4 form-control">
                    <input type="text" placeholder="https://example.com/game.nsp" v-model="downloadUrl"
                        class="w-full input input-bordered" />
                </div>
                <div class="flex justify-end gap-2 mt-6">
                    <AluButton @click="closeUrlDialog" size="small">Cancel</AluButton>
                    <AluButton level="primary" :disabled="!isValidUrl" @click="submitUrlDownload" size="small">Download
                    </AluButton>
                </div>
            </div>
        </div>

        <!-- Toast container for notifications -->
        <div v-if="showToast" class="toast toast-end z-[9999] p-4 mb-4 mr-4">
            <div class="alert" :class="toastType">
                <span>{{ toastMessage }}</span>
            </div>
        </div>
    </Teleport>
</template>

<script setup>
import { computed, ref, onMounted, onUnmounted } from "vue";
import { formatFileSize } from "@/util.js";
import { importGameUltraNX } from "@/utils/import.ts";
import AgeRating from "./AgeRating.vue";
import AluButton from "../AluButton.vue";

const props = defineProps({
    game: {
        type: Object,
        required: true,
    },
});

const importers = {
    ultranx: "UltraNX",
    upload: "Upload file...",
    url: "Download from URL...",
};

const emit = defineEmits(["get-metadata", "import"]);
const isUploadPopoverOpen = ref(false);
const isUrlDialogOpen = ref(false);
const selectedFile = ref(null);
const downloadUrl = ref("");
const dropdownMenu = ref(null);

// Toast state
const showToast = ref(false);
const toastMessage = ref("");
const toastType = ref("alert-info");
const toastTimeout = ref(null);

// Loading state
const isImporting = ref(false);

const formattedSize = computed(() => {
    return formatFileSize(props.game.size);
});

const isValidUrl = computed(() => {
    if (!downloadUrl.value) return false;
    try {
        const url = new URL(downloadUrl.value);
        return url.protocol === "http:" || url.protocol === "https:";
    } catch (e) {
        return false;
    }
});

const emitGetMetadata = (event) => {
    // Don't trigger navigation if we're clicking inside any modal
    if (isUploadPopoverOpen.value || isUrlDialogOpen.value) {
        event.stopPropagation();
        return;
    }
    emit("get-metadata", props.game.titleId);
};

/**
 * Shows a toast notification
 * @param {string} message - The message to display
 * @param {string} type - The type of toast (alert-success, alert-error, alert-info, alert-warning)
 * @param {number} duration - Duration in milliseconds to show the toast
 */
const showToastNotification = (message, type = "alert-info", duration = 3000) => {
    // Clear any existing timeout
    if (toastTimeout.value) {
        clearTimeout(toastTimeout.value);
    }

    // Set toast properties
    toastMessage.value = message;
    toastType.value = type;
    showToast.value = true;

    // Auto-hide the toast after duration
    toastTimeout.value = setTimeout(() => {
        showToast.value = false;
    }, duration);
};

/**
 * Handles importing a game from UltraNX
 * @param {Object} gameMetadata - The game metadata object
 */
const handleUltraNXImport = async (gameMetadata) => {
    if (isImporting.value) return; // Don't allow multiple simultaneous imports

    isImporting.value = true;

    try {
        const result = await importGameUltraNX(gameMetadata);

        // Check if it's an error or success based on the status field
        if (result && result.status === "error") {
            // Show the actual error message from the API response
            showToastNotification(result.message || "Import failed", "alert-error");
        } else {
            // It's a success, show success toast with message
            showToastNotification(
                result && result.message ? result.message : "Import successful",
                "alert-success"
            );
        }

        emitImport("ultranx", result);
    } catch (error) {
        console.error("Error importing from UltraNX:", error);

        // Extract error message from response if available
        let errorMessage = "Error importing from UltraNX";

        if (error.response && error.response.data) {
            // Try to get the detailed error message from the response data
            const responseData = error.response.data;
            if (typeof responseData === 'string') {
                // If response is a string, use it directly
                errorMessage = responseData;
            } else if (responseData.message) {
                // If response has a message property
                errorMessage = responseData.message;
            } else if (responseData.error) {
                // Some APIs use an error property
                errorMessage = responseData.error;
            }
        } else if (error.message) {
            // Use the error object's message if available
            errorMessage = error.message;
        }

        showToastNotification(errorMessage, "alert-error");
    } finally {
        isImporting.value = false;
    }
};

const handleImportOption = (key) => {
    // Don't allow actions while importing
    if (isImporting.value) return;

    // Close dropdown when opening any dialog
    if (dropdownMenu.value) {
        dropdownMenu.value.removeAttribute("open");
    }

    if (key === "upload") {
        isUploadPopoverOpen.value = true;
    } else if (key === "url") {
        isUrlDialogOpen.value = true;
    } else if (key === "ultranx") {
        handleUltraNXImport(props.game);
    } else {
        emitImport(key);
    }
};

const emitImport = (key, payload = null) => {
    emit("import", key, payload);
};

const closeUploadPopover = () => {
    isUploadPopoverOpen.value = false;
    selectedFile.value = null;
};

const closeUrlDialog = () => {
    isUrlDialogOpen.value = false;
    downloadUrl.value = "";
};

const handleFileSelected = (event) => {
    selectedFile.value = event.target.files[0] || null;
};

const uploadSelectedFile = () => {
    if (selectedFile.value) {
        emitImport("upload", selectedFile.value);
        closeUploadPopover();
    }
};

const submitUrlDownload = () => {
    if (isValidUrl.value) {
        emitImport("url", downloadUrl.value);
        closeUrlDialog();
    }
};

// Close popover when clicking outside
const handleClickOutside = (event) => {
    if (
        isUploadPopoverOpen.value &&
        uploadPopover.value &&
        !uploadPopover.value.contains(event.target)
    ) {
        closeUploadPopover();
    }
};

onMounted(() => {
    document.addEventListener("click", handleClickOutside);
});

onUnmounted(() => {
    document.removeEventListener("click", handleClickOutside);
    // Clear any active toast timeout
    if (toastTimeout.value) {
        clearTimeout(toastTimeout.value);
    }
});
</script>

<style scoped>
/* No need for popover styles as we're using a fixed position modal */
</style>
