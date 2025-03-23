<template>
    <div class="min-h-screen flex items-center justify-center">
        <div class="max-w-4xl mx-auto p-6 space-y-6">
            <div class="bg-gray-800 rounded-lg shadow-md p-6">
                <h2 class="text-2xl font-bold mb-6 text-white">Create User</h2>
                <form
                    @submit.prevent="createUser"
                    class="flex flex-col md:flex-row gap-4"
                >
                    <input
                        v-model="newUser.username"
                        type="text"
                        placeholder="Username"
                        required
                        class="px-4 py-2 bg-gray-700 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 text-white"
                    />
                    <input
                        v-model="newUser.password"
                        type="password"
                        placeholder="Password"
                        required
                        class="px-4 py-2 bg-gray-700 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 text-white"
                    />
                    <button
                        type="submit"
                        class="bg-blue-500 text-white px-6 py-2 rounded-lg hover:bg-blue-600 transition-colors"
                    >
                        Create User
                    </button>
                </form>
            </div>
            <br />
            <div class="bg-gray-800 rounded-lg shadow-md p-6">
                <h2 class="text-2xl font-bold mb-6 text-white">Users</h2>
                <ul class="divide-y divide-gray-600">
                    <li
                        v-for="user in users"
                        :key="user.username"
                        class="py-4 px-2 flex justify-between items-center"
                    >
                        <span class="text-white">{{ user.username }}</span>
                        <button
                            @click="deleteUser(user.username)"
                            class="bg-red-500 text-white px-4 py-2 rounded-lg hover:bg-red-600 transition-colors"
                        >
                            Delete
                        </button>
                    </li>
                </ul>
            </div>
        </div>
    </div>
</template>

<script setup>
import { ref, onMounted } from "vue";

const users = ref([]);
const newUser = ref({
    username: "",
    password: "",
});

const loadUsers = async () => {
    try {
        const response = await fetch("/api/users");
        users.value = await response.json();
    } catch (error) {
        console.error("Error loading users:", error);
    }
};

const createUser = async () => {
    try {
        const response = await fetch("/api/users", {
            method: "POST",
            headers: {
                "Content-Type": "application/json",
            },
            body: JSON.stringify(newUser.value),
        });

        if (!response.ok) throw new Error("Failed to create user");

        alert("User created successfully");
        newUser.value = { username: "", password: "" };
        await loadUsers();
    } catch (error) {
        alert("Error creating user: " + error);
    }
};

const deleteUser = async (username) => {
    if (!confirm(`Are you sure you want to delete user ${username}?`)) return;

    try {
        const response = await fetch(`/api/users/${username}`, {
            method: "DELETE",
        });

        if (!response.ok) throw new Error("Failed to delete user");
        await loadUsers();
    } catch (error) {
        alert("Error deleting user: " + error);
    }
};

onMounted(() => {
    loadUsers();
});
</script>
