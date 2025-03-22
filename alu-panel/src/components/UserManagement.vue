<template>
    <div class="user-management">
        <div class="create-user">
            <h2>Create User</h2>
            <form @submit.prevent="createUser">
                <input
                    v-model="newUser.username"
                    type="text"
                    placeholder="Username"
                    required
                />
                <input
                    v-model="newUser.password"
                    type="password"
                    placeholder="Password"
                    required
                />
                <button type="submit">Create User</button>
            </form>
        </div>

        <div class="users-list">
            <h2>Users</h2>
            <ul>
                <li v-for="user in users" :key="user.username">
                    {{ user.username }}
                    <button @click="deleteUser(user.username)">Delete</button>
                </li>
            </ul>
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

<style scoped>
.user-management {
    padding: 1rem;
}

.create-user form {
    display: flex;
    gap: 1rem;
    margin-bottom: 2rem;
}

.users-list li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem;
    border-bottom: 1px solid #ddd;
}
</style>
