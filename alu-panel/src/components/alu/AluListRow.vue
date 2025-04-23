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
                                <a @click.stop="handleImportOption(key)" :class="{
                                    'opacity-50 cursor-not-allowed': isImporting || disabledOptions[key],
                                    'line-through text-base-300': disabledOptions[key]
                                }" :disabled="isImporting || disabledOptions[key]">
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
                <div class="flex justify-end gap-2 mt-8">
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
                <div class="flex justify-end gap-2 mt-4 pt-4 border-t border-base-300">
                    <AluButton @click="closeUrlDialog" size="small">Cancel</AluButton>
                    <AluButton level="primary" :disabled="!isValidUrl" @click="submitUrlDownload" size="small">Download
                    </AluButton>
                </div>
            </div>
        </div>

        <!-- Toast container for stacked notifications -->
        <div class="toast toast-end z-[9999] p-4 mb-4 mr-4">
            <div v-for="(toast, index) in toasts" :key="index" class="alert my-2" :class="toast.type">
                <span>{{ toast.message }}</span>
            </div>
        </div>
    </Teleport>
</template>

<script lang="ts" setup>
import { computed, ref, onMounted, onUnmounted } from "vue";
import { formatFileSize } from "@/util.js";
import { importGameUltraNX, importGameURL } from "@/utils/import";
import type { TitleMetadata } from "@/utils/title";
import AgeRating from "./AgeRating.vue";
import AluButton from "../AluButton.vue";

// Use the existing TitleMetadata type
const props = defineProps<{
    game: TitleMetadata;
}>();

interface Toast {
    id: number;
    message: string;
    type: string;
    timeoutId?: number;
}

const importers = {
    ultranx_fullpkg: "UltraNX (Full)",
    ultranx_base: "UltraNX (Base)",
    ultranx_update: "UltraNX (Update)",
    upload: "Upload file...",
    url: "Download from URL...",
};

// Add a computed property to identify disabled options
const disabledOptions = computed(() => ({
    ultranx_fullpkg: false,
    ultranx_base: false,
    ultranx_update: false,
    upload: true, // Mark upload as disabled
    url: false
}));

const emit = defineEmits<{
    (e: 'get-metadata', titleId: string): void;
    (e: 'import', key: string, payload?: any): void;
}>();

const isUploadPopoverOpen = ref(false);
const isUrlDialogOpen = ref(false);
const selectedFile = ref<File | null>(null);
const downloadUrl = ref("");
const dropdownMenu = ref<HTMLElement | null>(null);

// Toast state - updated to support multiple toasts
const toasts = ref<Toast[]>([]);
let nextToastId = 0;

// Loading state
const isImporting = ref(false);

const formattedSize = computed(() => {
    return formatFileSize(props.game.size || 0);
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

const emitGetMetadata = (event: MouseEvent) => {
    // Don't trigger navigation if we're clicking inside any modal
    if (isUploadPopoverOpen.value || isUrlDialogOpen.value) {
        event.stopPropagation();
        return;
    }
    emit("get-metadata", props.game.titleId);
};

/**
 * Shows a toast notification that will be added to the stack
 * @param message - The message to display
 * @param type - The type of toast (alert-success, alert-error, alert-info, alert-warning)
 * @param duration - Duration in milliseconds to show the toast
 */
const showToastNotification = (message: string, type = "alert-info", duration = 3000) => {
    // Create a unique ID for this toast
    const id = nextToastId++;

    // Add the toast to our list
    const toast: Toast = {
        id,
        message,
        type,
    };

    toasts.value.push(toast);

    // Auto-hide the toast after duration
    const timeoutId = window.setTimeout(() => {
        // Remove this toast when the timeout expires
        toasts.value = toasts.value.filter(t => t.id !== id);
    }, duration);

    // Store the timeout ID so we can clear it if needed
    toast.timeoutId = timeoutId;
};

/**
 * Handles importing a game from UltraNX
 * @param gameMetadata - The game metadata object
 * @param type - The type of import (e.g., "fullpkg", "base", "update")
 */
const handleUltraNXImport = async (gameMetadata: TitleMetadata, type: string) => {
    if (isImporting.value) return; // Don't allow multiple simultaneous imports
    isImporting.value = true;
    try {
        // No need for conversion since we're already using TitleMetadata
        const result = await importGameUltraNX(gameMetadata, type);
        showToastNotification(`Import (${type}) started successfully`, "alert-success");
        emitImport("ultranx", result); // Keep the event name generic
    } catch (error) {
        console.error(`Error importing ${type} from UltraNX:`, error);
        const errorMsg = error instanceof Error ? error.message : `Error importing ${type} from UltraNX`;
        showToastNotification(errorMsg, "alert-error");
    } finally {
        isImporting.value = false;
    }
};

/**
 * Handles importing a game from a URL
 * @param url - The URL to import from
 */
const handleUrlImport = async (url: string) => {
    if (isImporting.value) return; // Don't allow multiple simultaneous imports
    isImporting.value = true;
    try {
        const result = await importGameURL(url);
        showToastNotification("Import started successfully", "alert-success");
        emitImport("url", result);
    } catch (error) {
        console.error("Error importing from URL:", error);
        const errorMsg = error instanceof Error ? error.message : "Error importing from URL";
        showToastNotification(errorMsg, "alert-error");
    } finally {
        isImporting.value = false;
    }
};

const handleImportOption = (key: string) => {
    // Don't allow actions while importing or if the option is disabled
    if (isImporting.value || disabledOptions.value[key as keyof typeof disabledOptions.value]) return;

    // Close dropdown when opening any dialog
    if (dropdownMenu.value) {
        dropdownMenu.value.removeAttribute("open");
    }

    if (key === "url") {
        isUrlDialogOpen.value = true;
    } else if (key.startsWith("ultranx_")) {
        // Extract the type ('fullpkg', 'base', 'update') from the key
        const importType = key.substring("ultranx_".length);
        handleUltraNXImport(props.game, importType);
    } else {
        // Handle other potential import types like 'upload' if enabled in the future
        emitImport(key); // Emit for other types if needed
    }
};

const emitImport = (key: string, payload: any = null) => {
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

const handleFileSelected = (event: Event) => {
    const input = event.target as HTMLInputElement;
    selectedFile.value = input.files ? input.files[0] : null;
};

const uploadSelectedFile = () => {
    if (selectedFile.value) {
        emitImport("upload", selectedFile.value);
        closeUploadPopover();
    }
};

const submitUrlDownload = () => {
    if (isValidUrl.value) {
        handleUrlImport(downloadUrl.value);
        closeUrlDialog();
    }
};

// Close popover when clicking outside
const handleClickOutside = (event: MouseEvent) => {
    // We're not using uploadPopover ref, so simplify this check
    if (isUploadPopoverOpen.value) {
        // Check if the clicked element is not inside the modal
        const modal = document.querySelector('.p-6.rounded-lg.shadow-xl');
        if (modal && !modal.contains(event.target as Node)) {
            closeUploadPopover();
        }
    }
};

onMounted(() => {
    document.addEventListener("click", handleClickOutside);
});

onUnmounted(() => {
    document.removeEventListener("click", handleClickOutside);
    // Clear all active toast timeouts
    toasts.value.forEach(toast => {
        if (toast.timeoutId) {
            clearTimeout(toast.timeoutId);
        }
    });
});
</script>

<style scoped>
/* No need for popover styles as we're using a fixed position modal */
</style>
